use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

use atsamd_hal::pac::{DWT, NVIC, SCB};
use cortex_m::interrupt;
use heapless::Vec;

use crate::Timestamp;
use crate::critical_section::CsGuard;
use crate::dispatchers::{DISPATCHERS, NUM_DISPATCHERS, dispatcher, dispatcher_irq};
use crate::task::{RunningTask, ScheduledTask, Task};
use crate::vector_table::set_handler;

struct TaskStack(UnsafeCell<Vec<RunningTask, NUM_DISPATCHERS, u8>>);

unsafe impl Sync for TaskStack {}

impl TaskStack {
    fn get_mut(&self, _cs: &CsGuard) -> *mut Vec<RunningTask, NUM_DISPATCHERS, u8> {
        self.0.get()
    }
}

struct MinDeadline(UnsafeCell<Timestamp>);

unsafe impl Sync for MinDeadline {}

impl MinDeadline {
    fn get_mut(&self, _cs: &CsGuard) -> *mut Timestamp {
        self.0.get()
    }
}

// TODO get rid of this magic number
struct TaskQueue(UnsafeCell<Vec<ScheduledTask, 64, u8>>);

unsafe impl Sync for TaskQueue {}

impl TaskQueue {
    fn get_mut(&self, _cs: &CsGuard) -> *mut Vec<ScheduledTask, 64, u8> {
        self.0.get()
    }

    unsafe fn get_most_urgent_task(&self, cs: &CsGuard) -> Option<(usize, &ScheduledTask)> {
        unsafe {
            (&*self.get_mut(cs))
                .iter()
                .enumerate()
                .min_by_key(|(_, t)| t.abs_deadline())
        }
    }

    unsafe fn remove(&self, cs: &CsGuard, index: usize) -> ScheduledTask {
        unsafe { (&mut *self.get_mut(cs)).swap_remove_unchecked(index) }
    }
}

static RUNNING_STACK: TaskStack = TaskStack(UnsafeCell::new(Vec::new()));
static MIN_DEADLINE: MinDeadline = MinDeadline(UnsafeCell::new(u32::MAX));
static PARKED_QUEUE: TaskQueue = TaskQueue(UnsafeCell::new(Vec::new()));

pub struct Scheduler {
    ready: AtomicBool,
}

unsafe impl Sync for Scheduler {}

impl core::default::Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    pub const fn new() -> Self {
        Self {
            ready: AtomicBool::new(false),
        }
    }

    pub fn check_init(&self) {
        if !self.ready.load(Ordering::SeqCst) {
            panic!("Scheduler not initialized");
        }
    }

    pub fn init(&self, nvic: &mut NVIC, scb: &mut SCB) {
        interrupt::disable();

        // Before we can start messing with the vector table, we must first copy it over
        // to RAM
        crate::vector_table::copy_vector_table(scb);

        for (level, interrupt) in DISPATCHERS.iter().enumerate() {
            // TODO remove this "8" magic number somehow, which is the number of priorities
            // available on the ATSAMD51J
            let nvic_prio = (8 - (level as u8 + 1)) << 4;

            unsafe {
                NVIC::unpend(*interrupt);
                NVIC::unmask(*interrupt);
                nvic.set_priority(*interrupt, nvic_prio);
            }
        }

        self.ready.swap(true, Ordering::SeqCst);
        // interrupt::enable();
    }

    pub fn enqueue(&self, task: Task) {
        let cs = CsGuard::new();
        let now = now();
        let (queue, min_dl) = unsafe {
            (
                &mut *PARKED_QUEUE.get_mut(&cs),
                &mut *MIN_DEADLINE.get_mut(&cs),
            )
        };
        let task = task.into_queued(now);
        if task.abs_deadline() < *min_dl {
            *min_dl = task.abs_deadline();
        }
        queue.push(task).unwrap();
    }

    pub fn schedule(&self, task: Task) {
        self.check_init();
        // let prev_count = now();

        let cs = CsGuard::new();
        let now_ts = now();
        let task = task.into_queued(now_ts);
        let (stack, min_dl) =
            unsafe { (&mut *RUNNING_STACK.get_mut(&cs), *MIN_DEADLINE.get_mut(&cs)) };

        if task.abs_deadline() < min_dl || stack.is_empty() {
            Self::execute(cs, task);
            // defmt::warn!("Schedule cycle count (preempt): {}", now() -
            // prev_count);
        } else {
            {
                let queue = unsafe { &mut *PARKED_QUEUE.get_mut(&cs) };
                queue.push(task);
                // defmt::warn!(
                //     "Schedule cycle count (enqueue): {}, queue len: {}",
                //     now() - prev_count,
                //     queue.len() - 1
                // );
            }
        }
    }

    fn execute(cs: CsGuard, task: ScheduledTask) {
        // let prev_count = now();
        let min_dl = unsafe { &mut *MIN_DEADLINE.get_mut(&cs) };
        let prev_dl = *min_dl;
        *min_dl = task.abs_deadline();

        let stack = unsafe { &mut *RUNNING_STACK.get_mut(&cs) };

        stack.push(RunningTask::from_scheduled(task, prev_dl));
        let max_prio = stack.len() as u8;

        let irq = dispatcher_irq(max_prio);
        unsafe { set_handler(irq, run_task) };
        NVIC::pend(dispatcher(max_prio));
        // defmt::warn!("Execute cycle count: {}", now() - prev_count);
    }

    pub fn idle(&self) -> ! {
        unsafe { cortex_m::interrupt::enable() };
        loop {
            cortex_m::asm::wfi();
        }
    }
}

/// Trampoline that takes care of launching the task, and restoring the
/// scheduler state after its execution completes.
extern "C" fn run_task() {
    // let prev_count = now();
    let (callback, prev_deadline) = unsafe {
        let cs = CsGuard::new();
        let task = (&*RUNNING_STACK.get_mut(&cs)).last().unwrap();
        (task.callback(), task.prev_deadline())
    };

    // defmt::warn!("Task setup cycle count: {}", now() - prev_count);
    // Finally call the actual task
    callback();

    // And cleanup after ourselves
    let prev_count = now();
    let cs = CsGuard::new();
    let (stack, min_deadline) = unsafe {
        (
            &mut *RUNNING_STACK.get_mut(&cs),
            &mut *MIN_DEADLINE.get_mut(&cs),
        )
    };

    let warn_unchecked_option = ();
    stack.pop();
    // Restore previous deadline
    *min_deadline = prev_deadline;

    // It's possible that a task showed up in the queue as the previous task was
    // running. So we need to check if it would preempt the next task in line to
    // run, which would start as soon as the critical section exits.
    unsafe {
        if let Some((idx, task)) = PARKED_QUEUE.get_most_urgent_task(&cs)
            && (task.abs_deadline() < *min_deadline || stack.is_empty())
        {
            let task = PARKED_QUEUE.remove(&cs, idx);
            Scheduler::execute(cs, task);

            defmt::warn!(
                "Task cleanup (reschedule) cycle count: {}",
                now() - prev_count
            );
        } else {
            defmt::warn!(
                "Task cleanup (fall through) cycle count: {}",
                now() - prev_count
            );
        }
    }
}

fn now() -> u32 {
    DWT::cycle_count()
}
