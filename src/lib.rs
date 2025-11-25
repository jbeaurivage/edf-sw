#![no_std]

pub use fugit;

pub mod dispatchers;
pub mod scheduler;
pub mod task;

mod critical_section;

type Timestamp = u32;
pub type Deadline = u32;

pub mod benchmark {
    pub fn reset_cyccnt() {
        let mut dwt = unsafe { cortex_m::peripheral::Peripherals::steal() }.DWT;
        dwt.set_cycle_count(0);
        dwt.enable_cycle_counter();
    }
}
