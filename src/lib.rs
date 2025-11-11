#![no_std]

use fugit::{TimerDurationU32, TimerInstantU32};

pub mod dispatchers;
pub mod scheduler;
pub mod task;

mod critical_section;
mod vector_table;

type Timestamp = TimerInstantU32<32768>;
pub type Deadline = TimerDurationU32<32768>;

// TODO this is not great at all and should NOT be used for production!!
pub unsafe trait IntoUnchecked<T> {
    fn into_unchecked(self) -> T;
}

unsafe impl IntoUnchecked<Timestamp> for TimerInstantU32<32768> {
    fn into_unchecked(self) -> Timestamp {
        Timestamp::from_ticks(self.ticks() as u32)
    }
}

unsafe impl IntoUnchecked<Deadline> for TimerDurationU32<32768> {
    fn into_unchecked(self) -> Deadline {
        Deadline::from_ticks(self.ticks() as u32)
    }
}

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
