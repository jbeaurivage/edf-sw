use core::cell::RefCell;

use atsamd_hal::pac::NVIC;
use cortex_m::interrupt::{self, Mutex};
use heapless::{
    Vec,
    sorted_linked_list::{Max, SortedLinkedList},
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

pub struct Scheduler {
    task_queue: Mutex<RefCell<SortedLinkedList<ScheduledTask, Max, 16, usize>>>,
}

impl core::default::Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    pub const fn new() -> Self {
        Self {
            task_queue: Mutex::new(RefCell::new(SortedLinkedList::new_usize())),
        }
    }

    pub fn init(&self, nvic: &mut NVIC) {
        for (level, interrupt) in DISPATCHERS.iter().enumerate() {
            unsafe {
                NVIC::unpend(*interrupt);
                NVIC::unmask(*interrupt);
                nvic.set_priority(*interrupt, 16 - (level as u8 + 1));
                interrupt::enable();
            }
        }
    }

    pub fn schedule<M>(&self, task: Task)
    where
        M: Monotonic<Instant = Timestamp>,
    {
        let cs = RestoreCs::new();
        let task = task.into_queued(M::now());

        if task.deadline() < *MIN_DEADLINE.borrow(&cs).borrow_mut() {
            defmt::debug!("execute");
            self.execute(cs, task);
        } else {
            {
                defmt::debug!("enqueue");
                let mut queue = self.task_queue.borrow(&cs).borrow_mut();
                queue.push(task).unwrap();
            }
        }
    }

    fn execute(&self, cs: RestoreCs, task: ScheduledTask) {
        {
            let mut min_dl = MIN_DEADLINE.borrow(&cs).borrow_mut();
            let prev_dl = *min_dl;
            *min_dl = task.deadline();

            let mut stack = TASK_STACK.borrow(&cs).borrow_mut();

            stack
                .push(RunningTask::from_scheduled(task, prev_dl))
                .unwrap();
            let max_prio = stack.len() as u8;

            let irq = dispatcher_irq(max_prio);
            unsafe { set_handler(irq, trampoline) };
            NVIC::pend(dispatcher(max_prio));
        }

        // Can also just drop crit section
        cs.restore();
    }

    pub fn idle(&self) -> ! {
        loop {
            let task = {
                let cs = RestoreCs::new();
                let mut queue = self.task_queue.borrow(&cs).borrow_mut();
                queue.pop()
            };

            if let Some(t) = task {
                let cs = RestoreCs::new();
                self.execute(cs, t);
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
}
