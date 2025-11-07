use atsamd_hal::pac::{Interrupt, interrupt};

pub const NUM_DISPATCHERS: usize = 3;

pub(crate) const DISPATCHERS: [Interrupt; NUM_DISPATCHERS] = [
    Interrupt::SERCOM0_0,
    Interrupt::SERCOM0_1,
    Interrupt::SERCOM0_2,
];

const fn interrupt_to_irq(interrupt: Interrupt) -> usize {
    interrupt as usize + 16
}

pub const fn dispatcher_irq(level: u8) -> usize {
    interrupt_to_irq(DISPATCHERS[level as usize])
}

pub const fn dispatcher(level: u8) -> Interrupt {
    DISPATCHERS[level as usize]
}

/// Level 1 dispatcher placeholder
#[interrupt]
fn SERCOM0_0() {}

/// Level 2 dispatcher placeholder
#[interrupt]
fn SERCOM0_1() {}

/// Level 3 dispatcher placeholder
#[interrupt]
fn SERCOM0_2() {}
