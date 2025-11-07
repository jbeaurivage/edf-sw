use core::{
    cell::RefCell,
    marker::PhantomData,
    sync::atomic::{AtomicBool, Ordering},
};

use atsamd_hal::pac::{NVIC, SCB};
use cortex_m::interrupt::{self, Mutex};
use heapless::{
    Vec,
    sorted_linked_list::{Min, SortedLinkedList},
};
use rtic_monotonics::Monotonic;

use crate::{
    Timestamp,
    critical_section::RestoreCs,
    dispatchers::{DISPATCHERS, dispatcher, dispatcher_irq},
    task::{RunningTask, ScheduledTask, Task},
    vector_table::set_handler,
};

pub(crate) static TASK_STACK: Mutex<RefCell<Vec<RunningTask, 16, u8>>> =
    Mutex::new(RefCell::new(Vec::new()));

pub(crate) static MIN_DEADLINE: Mutex<RefCell<Timestamp>> =
    Mutex::new(RefCell::new(Timestamp::from_ticks(u64::MAX)));

pub struct Scheduler<M>
where
    M: Monotonic<Instant = Timestamp>,
{
    ready: AtomicBool,
    task_queue: Mutex<RefCell<SortedLinkedList<ScheduledTask, Min, 16, usize>>>,
    mono: PhantomData<M>,
}

impl<M: Monotonic<Instant = Timestamp>> core::default::Default for Scheduler<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> Scheduler<M>
where
    M: Monotonic<Instant = Timestamp>,
{
    pub const fn new() -> Self {
        Self {
            ready: AtomicBool::new(false),
            task_queue: Mutex::new(RefCell::new(SortedLinkedList::new_usize())),
            mono: PhantomData,
        }
    }

    pub fn init(&self, nvic: &mut NVIC, scb: &mut SCB) {
        // Before we can start messing with the vector table, we must first copy it over
        // to RAM
        crate::vector_table::copy_vector_table(scb);

        for (level, interrupt) in DISPATCHERS.iter().enumerate() {
            unsafe {
                NVIC::unpend(*interrupt);
                NVIC::unmask(*interrupt);
                nvic.set_priority(*interrupt, 16 - (level as u8 + 1));
                interrupt::enable();
            }
        }

        self.ready.swap(true, Ordering::SeqCst);
    }

    pub fn schedule(&self, task: Task) {
        if !self.ready.load(Ordering::SeqCst) {
            panic!("Scheduler not initialized");
        }

        let cs = RestoreCs::new();
        let now = M::now();
        let rel_dl = task.rel_deadline();
        let task = task.into_queued(now);
        let min_dl = *MIN_DEADLINE.borrow(&cs).borrow_mut();

        defmt::trace!(
            "[SCHEDULE] now: {}, rel dl: {}, abs dl: {}, min dl: {}",
            now,
            rel_dl,
            task.abs_deadline(),
            min_dl
        );

        if task.abs_deadline() < min_dl || self.task_queue.borrow(&cs).borrow().is_empty() {
            defmt::trace!("preempt");
            self.execute(cs, task, now);
        } else {
            {
                defmt::trace!("enqueue");
                let mut queue = self.task_queue.borrow(&cs).borrow_mut();
                queue.push(task).unwrap();
            }
        }
    }

    fn execute(&self, cs: RestoreCs, task: ScheduledTask, now: Timestamp) {
        if !self.ready.load(Ordering::SeqCst) {
            panic!("Scheduler not initialized");
        }

        let mut min_dl = MIN_DEADLINE.borrow(&cs).borrow_mut();
        let prev_dl = *min_dl;
        *min_dl = task.abs_deadline();

        let mut stack = TASK_STACK.borrow(&cs).borrow_mut();

        stack
            .push(RunningTask::from_scheduled(task, prev_dl))
            .unwrap();
        let max_prio = stack.len() as u8;

        defmt::trace!(
            "[EXEC] now: {}, prio: {}, new dl: {}, prev dl: {}\n",
            now,
            max_prio,
            &*min_dl,
            prev_dl
        );

        let irq = dispatcher_irq(max_prio);
        unsafe { set_handler(irq, trampoline) };
        NVIC::pend(dispatcher(max_prio));
    }

    pub fn idle(&self) -> ! {
        loop {
            let task = {
                let cs = RestoreCs::new();
                let mut queue = self.task_queue.borrow(&cs).borrow_mut();
                queue.pop()
            };

            if let Some(t) = task {
                defmt::trace!("dequeue");
                let cs = RestoreCs::new();
                self.execute(cs, t, M::now());
            }
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn trampoline() {
    // TODO: I feel like somehow this should execute without giving the interrupts a
    // chance to run after it has been pended..
    let (callback, prev_deadline) = {
        let cs = RestoreCs::new();
        let stack = TASK_STACK.borrow(&cs).borrow_mut();
        let task = stack.last().unwrap();
        (task.callback(), task.prev_deadline())
    };

    // Finally call the actual task
    callback();

    // And cleanup after ourselves
    let cs = RestoreCs::new();
    TASK_STACK.borrow(&cs).borrow_mut().pop().unwrap();
    // Restore previous deadline
    *MIN_DEADLINE.borrow(&cs).borrow_mut() = prev_deadline;

    defmt::trace!("[COMPLETE TASK] new dl: {}", prev_deadline);
}
