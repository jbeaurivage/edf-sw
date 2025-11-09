#![no_std]

use atsamd_hal::fugit::{TimerDurationU64, TimerInstantU64};

pub mod dispatchers;
pub mod scheduler;
pub mod task;

mod critical_section;
mod vector_table;

type Timestamp = TimerInstantU64<32768>;
pub type Deadline = TimerDurationU64<32768>;

/// Print the vector table
pub fn print_vtable() {
    for (addr, item) in unsafe { vector_table::VECTOR_TABLE.get() }
        .iter()
        .enumerate()
    {
        defmt::debug!(
            "ADDR: {:#x}, item: {:#x}",
            vector_table::VECTOR_TABLE.addr() as usize + addr * 4,
            item
        );
    }
}

pub mod benchmark {
    pub fn reset_cyccnt() {
        let mut dwt = unsafe { cortex_m::peripheral::Peripherals::steal() }.DWT;
        dwt.set_cycle_count(0);
        dwt.enable_cycle_counter();
    }
}
