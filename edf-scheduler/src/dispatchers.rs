use atsamd_hal::pac::{Interrupt, interrupt};

pub const NUM_DISPATCHERS: usize = 3;

pub(crate) const DISPATCHERS: [Interrupt; NUM_DISPATCHERS] = [
    Interrupt::SERCOM0_0,
    Interrupt::SERCOM0_1,
    Interrupt::SERCOM0_2,
];

pub const fn dispatcher(level: u8) -> Interrupt {
    DISPATCHERS[level as usize]
}

/// Level 1 dispatcher placeholder
#[interrupt]
#[allow(non_snake_case)]
fn SERCOM0_0() {
    crate::scheduler::run_task::<0>();
}

/// Level 2 dispatcher placeholder
#[interrupt]
#[allow(non_snake_case)]
fn SERCOM0_1() {
    crate::scheduler::run_task::<1>();
}

/// Level 3 dispatcher placeholder
#[interrupt]
#[allow(non_snake_case)]
fn SERCOM0_2() {
    crate::scheduler::run_task::<2>();
}
