use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

use cortex_m::interrupt;
use cortex_m::peripheral::{DWT, NVIC};
use heapless::BinaryHeap;
use heapless::binary_heap::Min;

use crate::Timestamp;
use crate::critical_section::CsGuard;
use crate::dispatchers::{DISPATCHERS, NUM_DISPATCHERS, dispatcher};
use crate::task::{RunningTask, ScheduledTask, Task};

/// Depth 1 queue that serves to pass messages from the scheduler to the
/// dispatchers
struct TaskMessageQueue<const N: usize>(UnsafeCell<[Option<RunningTask>; N]>);

unsafe impl<const N: usize> Sync for TaskMessageQueue<N> {}

impl<const N: usize> TaskMessageQueue<N> {
    fn store(&self, _cs: &CsGuard, task: RunningTask, dispatcher_idx: usize) {
        unsafe {
            (&mut *self.0.get())
                .get_mut(dispatcher_idx)
                .expect("BUG: dispatcher idx doesn't exist")
                .replace(task);
        }
    }

    fn take(&self, _cs: &CsGuard, dispatcher_idx: usize) -> Option<RunningTask> {
        unsafe {
            (&mut *self.0.get())
                .get_mut(dispatcher_idx)
                .expect("BUG: dispatcher idx doesn't exist")
                .take()
        }
    }

    fn is_empty(&self, _cs: &CsGuard) -> bool {
        unsafe { (&*self.0.get()).iter().all(|f| f.is_none()) }
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
struct TaskQueue(UnsafeCell<BinaryHeap<ScheduledTask, Min, 16>>);

unsafe impl Sync for TaskQueue {}

impl TaskQueue {
    fn get_mut(&self, _cs: &CsGuard) -> *mut BinaryHeap<ScheduledTask, Min, 16> {
        self.0.get()
    }
}

static RUNNING_QUEUE: TaskMessageQueue<NUM_DISPATCHERS> =
    TaskMessageQueue(UnsafeCell::new([const { None }; NUM_DISPATCHERS]));
static MIN_DEADLINE: MinDeadline = MinDeadline(UnsafeCell::new(u32::MAX));
static PARKED_QUEUE: TaskQueue = TaskQueue(UnsafeCell::new(BinaryHeap::new()));

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

    pub fn init(&self, nvic: &mut NVIC) {
        interrupt::disable();

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

    pub fn schedule(&self, task: Task) {
        self.check_init();

        let cs = CsGuard::new();
        let now = now();
        #[cfg(feature = "defmt")]
        let rel_dl = task.rel_deadline();
        let task = task.into_queued(now);
        let min_dl = unsafe { *MIN_DEADLINE.get_mut(&cs) };

        #[cfg(feature = "defmt")]
        defmt::debug!(
            "[SCHEDULE] now: {}, rel dl: {}, abs dl: {}, min dl: {}",
            now,
            rel_dl,
            task.abs_deadline(),
            min_dl
        );

        if task.abs_deadline() < min_dl || RUNNING_QUEUE.is_empty(&cs) {
            #[cfg(feature = "defmt")]
            defmt::debug!("[PREEMPT]");
            Self::execute(cs, task, now);
        } else {
            {
                let queue = unsafe { &mut *PARKED_QUEUE.get_mut(&cs) };
                #[cfg(feature = "defmt")]
                defmt::debug!("[ENQUEUE] queue length: {}", queue.len());

                queue.push(task).expect("Task queue is full");
            }
        }
    }

    fn execute(cs: CsGuard, task: ScheduledTask, now: Timestamp) {
        let min_dl = unsafe { &mut *MIN_DEADLINE.get_mut(&cs) };
        let prev_dl = *min_dl;
        *min_dl = task.abs_deadline();

        let dispatcher_prio = task.dispatcher_prio();

        RUNNING_QUEUE.store(
            &cs,
            RunningTask::from_scheduled(task, prev_dl),
            dispatcher_prio as usize,
        );
        // let max_prio = stack.len() as u8;

        #[cfg(feature = "defmt")]
        defmt::debug!(
            // "[EXEC] max prio: {}, dispatcher prio: {}, now: {}, new dl: {}, prev dl: {}",
            "[EXEC] dispatcher prio: {}, now: {}, new dl: {}, prev dl: {}",
            // max_prio,
            dispatcher_prio,
            now,
            &*min_dl,
            prev_dl
        );

        NVIC::pend(dispatcher(dispatcher_prio));
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
#[inline(always)]
pub(super) fn run_task<const P: usize>() {
    let (callback, prev_deadline) = {
        let cs = CsGuard::new();

        let task = RUNNING_QUEUE
            .take(&cs, P)
            .expect("BUG: a task is supposed to be enqueued here");

        (task.callback(), task.prev_deadline())
    };

    // Finally call the actual task
    callback();

    // And cleanup after ourselves
    let cs = CsGuard::new();
    let min_deadline = unsafe { &mut *MIN_DEADLINE.get_mut(&cs) };

    // TODO: should the task be dequeued here instead?
    // stack
    //     .pop()
    //     .expect("BUG: dispatcher stack should contain at least one task");
    // Restore previous deadline
    *min_deadline = prev_deadline;

    #[cfg(feature = "defmt")]
    defmt::debug!(
        // "[COMPLETE TASK] new dl: {}, stack depth: {}",
        "[COMPLETE TASK] new dl: {}",
        prev_deadline,
        // stack.len(),
    );

    // It's possible that a task showed up in the queue as the previous task was
    // running. So we need to check if it would preempt the next task in line to
    // run, which would start as soon as the critical section exits.
    let queue = unsafe { &mut *PARKED_QUEUE.get_mut(&cs) };
    if let Some(task) = queue.peek()
        && (task.abs_deadline() < *min_deadline || RUNNING_QUEUE.is_empty(&cs))
    {
        let task = unsafe { queue.pop_unchecked() };
        #[cfg(feature = "defmt")]
        defmt::debug!("[RESCHEDULE TASK]");
        Scheduler::execute(cs, task, now());
    }
}

fn now() -> u32 {
    DWT::cycle_count()
}
