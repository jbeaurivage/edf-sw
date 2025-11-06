#![no_std]
#![no_main]

use atsamd_hal as hal;
use cortex_m::asm::nop;
use defmt_rtt as _;
use hal::rtc::rtic::rtc_clock;
use panic_probe as _;

use edf_sw::vector_table;

hal::rtc_monotonic!(Mono, rtc_clock::Clock32k);

#[unsafe(no_mangle)]
static RTIC_ASYNC_MAX_LOGICAL_PRIO: u8 = 4;

#[cortex_m_rt::entry]
fn main() -> ! {
    let peripherals = atsamd_hal::pac::Peripherals::take().unwrap();
    let mut core = atsamd_hal::pac::CorePeripherals::take().unwrap();
    Mono::start(peripherals.rtc);

    vector_table::copy(&mut core.SCB);
    vector_table::print_dbg();

    core.SYST.set_reload(16_000_000 - 1);
    core.SYST.clear_current();
    core.SYST.enable_interrupt();
    core.SYST.enable_counter();

    unsafe { cortex_m::interrupt::enable() };

    loop {
        nop();
    }
}

// Just testing that the systick interrupt works after relocating the vtable
#[cortex_m_rt::exception]
fn SysTick() {
    defmt::info!("tick!");
}
