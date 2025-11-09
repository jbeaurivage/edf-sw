use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicBool, Ordering};

use atsamd_hal::pac::{DWT, NVIC, SCB};
use cortex_m::interrupt;
use defmt::Format;
use heapless::Vec;
use heapless::sorted_linked_list::{Min, SortedLinkedList};
use rtic_monotonics::Monotonic;

use crate::critical_section::CsGuard;
use crate::dispatchers::{DISPATCHERS, NUM_DISPATCHERS, dispatcher, dispatcher_irq};
use crate::task::{RunningTask, ScheduledTask, Task};
use crate::vector_table::set_handler;
use crate::{IntoUnchecked, Timestamp};

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
struct TaskQueue(UnsafeCell<SortedLinkedList<ScheduledTask, Min, 16, usize>>);

unsafe impl Sync for TaskQueue {}

impl TaskQueue {
    fn get_mut(&self, _cs: &CsGuard) -> *mut SortedLinkedList<ScheduledTask, Min, 16, usize> {
        self.0.get()
    }
}

static RUNNING_STACK: TaskStack = TaskStack(UnsafeCell::new(Vec::new()));
static MIN_DEADLINE: MinDeadline = MinDeadline(UnsafeCell::new(Timestamp::from_ticks(u32::MAX)));
static PARKED_QUEUE: TaskQueue = TaskQueue(UnsafeCell::new(SortedLinkedList::new_usize()));

pub struct Scheduler<M>
where
    M: Monotonic<Instant: IntoUnchecked<Timestamp>>,
{
    ready: AtomicBool,
    mono: PhantomData<M>,
}

unsafe impl<M: Monotonic<Instant: IntoUnchecked<Timestamp>>> Sync for Scheduler<M> {}

impl<M: Monotonic<Instant: IntoUnchecked<Timestamp> + Format>> core::default::Default
    for Scheduler<M>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<M> Scheduler<M>
where
    M: Monotonic<Instant: IntoUnchecked<Timestamp> + Format>,
{
    pub const fn new() -> Self {
        Self {
            ready: AtomicBool::new(false),
            mono: PhantomData,
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

    pub fn schedule(&self, task: Task) {
        self.check_init();

        let cs = CsGuard::new();
        let now = M::now().into_unchecked();
        let task = task.into_queued(now);
        let (stack, min_dl) =
            unsafe { (&mut *RUNNING_STACK.get_mut(&cs), *MIN_DEADLINE.get_mut(&cs)) };

        if task.abs_deadline() < min_dl || stack.is_empty() {
            Self::execute(cs, task);
            let cyccnt = DWT::cycle_count();
            defmt::warn!("Schedule cycle count (preempt): {}", cyccnt);
        } else {
            {
                let queue = unsafe { &mut *PARKED_QUEUE.get_mut(&cs) };
                queue.push(task).unwrap();
                let cyccnt = DWT::cycle_count();
                let queue_len = queue.iter().count() - 1;
                defmt::warn!(
                    "Schedule cycle count (enqueue): {}, queue len: {}",
                    cyccnt,
                    queue_len
                );
            }
        }
    }

    fn execute(cs: CsGuard, task: ScheduledTask) {
        let min_dl = unsafe { &mut *MIN_DEADLINE.get_mut(&cs) };
        let prev_dl = *min_dl;
        *min_dl = task.abs_deadline();

        let stack = unsafe { &mut *RUNNING_STACK.get_mut(&cs) };

        stack
            .push(RunningTask::from_scheduled(task, prev_dl))
            .unwrap();
        let max_prio = stack.len() as u8;

        let irq = dispatcher_irq(max_prio);
        unsafe { set_handler(irq, run_task::<M>) };
        NVIC::pend(dispatcher(max_prio));
    }

    pub fn idle(&self) -> ! {
        self.check_init();

        unsafe {
            interrupt::enable();
        }

        loop {
            let cs = CsGuard::new();
            let queue = unsafe { &mut *PARKED_QUEUE.get_mut(&cs) };
            let task = queue.pop();

            if let Some(t) = task {
                Self::execute(cs, t);
            }
        }
    }
}

/// Trampoline that takes care of launching the task, and restoring the
/// scheduler state after its execution completes.
extern "C" fn run_task<M: Monotonic<Instant: IntoUnchecked<Timestamp> + Format>>() {
    let (callback, prev_deadline) = unsafe {
        let cs = CsGuard::new();
        let task = (&*RUNNING_STACK.get_mut(&cs)).last().unwrap();
        (task.callback(), task.prev_deadline())
    };

    // Finally call the actual task
    callback();

    // And cleanup after ourselves
    let cs = CsGuard::new();
    let (stack, min_deadline, queue) = unsafe {
        (
            &mut *RUNNING_STACK.get_mut(&cs),
            &mut *MIN_DEADLINE.get_mut(&cs),
            &mut *PARKED_QUEUE.get_mut(&cs),
        )
    };

    stack.pop().unwrap();
    // Restore previous deadline
    *min_deadline = prev_deadline;

    // It's possible that a task showed up in the queue as the previous task was
    // running. So we need to check if it would preempt the next task in line to
    // run, which would start as soon as the critical section exits.
    if let Some(q_t) = queue.peek()
        && q_t.abs_deadline() < *min_deadline
    {
        let task = queue.pop().unwrap();
        Scheduler::<M>::execute(cs, task);
    }
}
