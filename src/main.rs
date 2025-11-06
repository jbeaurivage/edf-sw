#![no_std]
#![no_main]

use atsamd_hal as hal;
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
    let core = atsamd_hal::pac::CorePeripherals::take().unwrap();
    Mono::start(peripherals.rtc);

    vector_table::copy();
    vector_table::print_dbg();

    loop {
        cortex_m::asm::nop();
    }
}
