#![no_std]

use atsamd_hal::fugit::{TimerDurationU64, TimerInstantU64};

pub mod dispatchers;
pub mod scheduler;
pub mod task;
pub mod vector_table;

mod critical_section;

type Timestamp = TimerInstantU64<32768>;
pub type Deadline = TimerDurationU64<32768>;
