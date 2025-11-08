use core::cmp::Ordering;

use crate::{Deadline, Timestamp};

#[derive(Debug)]
pub struct Task {
    // TODO For debugging purposes
    id: u8,
    rel_deadline: Deadline,
    callback: fn(),
}

impl Task {
    pub fn new(id: u8, rel_deadline: Deadline, callback: fn()) -> Self {
        Self {
            id,
            rel_deadline,
            callback,
        }
    }

    pub fn id(&self) -> u8 {
        self.id
    }

    pub fn rel_deadline(&self) -> Deadline {
        self.rel_deadline
    }

    pub fn set_deadline(&mut self, deadline: Deadline) {
        self.rel_deadline = deadline;
    }

    pub(crate) fn into_queued(self, now: Timestamp) -> ScheduledTask {
        ScheduledTask {
            id: self.id,
            deadline: now + self.rel_deadline,
            callback: self.callback,
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
#[allow(unpredictable_function_pointer_comparisons)]
pub(crate) struct ScheduledTask {
    id: u8,
    deadline: Timestamp,
    callback: fn(),
}

impl ScheduledTask {
    pub fn id(&self) -> u8 {
        self.id
    }

    pub fn abs_deadline(&self) -> Timestamp {
        self.deadline
    }
}

impl PartialOrd for ScheduledTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.cmp(other).into()
    }
}

impl Ord for ScheduledTask {
    fn cmp(&self, other: &Self) -> Ordering {
        self.deadline.cmp(&other.deadline)
    }
}

#[derive(Debug)]
pub(crate) struct RunningTask {
    id: u8,
    prev_deadline: Timestamp,
    callback: fn(),
}

impl RunningTask {
    pub fn from_scheduled(task: ScheduledTask, prev_deadline: Timestamp) -> Self {
        Self {
            id: task.id,
            prev_deadline,
            callback: task.callback,
        }
    }

    pub fn id(&self) -> u8 {
        self.id
    }

    pub fn prev_deadline(&self) -> Timestamp {
        self.prev_deadline
    }

    pub fn callback(&self) -> fn() {
        self.callback
    }
}
