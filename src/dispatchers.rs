use atsamd_hal::pac::{Interrupt, interrupt};

/// Level 1 dispatcher
#[interrupt]
fn SERCOM0_0() {
    const IRQ: u32 = Interrupt::SERCOM0_0 as u32 + 16;
}

/// Level 2 dispatcher
#[interrupt]
fn SERCOM0_1() {
    const IRQ: u32 = Interrupt::SERCOM0_1 as u32 + 16;
}

/// Level 3 dispatcher
#[interrupt]
fn SERCOM0_2() {
    const IRQ: u32 = Interrupt::SERCOM0_2 as u32 + 16;
}

/// A task ISR which signals a pending task
#[interrupt]
fn SERCOM1_0() {}
