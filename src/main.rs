#![no_std]
#![no_main]

use atsamd_hal::{self as hal};
use defmt_rtt as _;
use hal::rtc::rtic::rtc_clock;
use panic_probe as _;

use edf_sw::{Deadline, scheduler::Scheduler, task::Task, vector_table};

hal::rtc_monotonic!(Mono, rtc_clock::Clock32k);

#[unsafe(no_mangle)]
static RTIC_ASYNC_MAX_LOGICAL_PRIO: u8 = 4;

static SCHEDULER: Scheduler = Scheduler::new();

#[cortex_m_rt::entry]
fn main() -> ! {
    let peripherals = atsamd_hal::pac::Peripherals::take().unwrap();
    let mut core = atsamd_hal::pac::CorePeripherals::take().unwrap();
    Mono::start(peripherals.rtc);

    vector_table::copy_vector_table(&mut core.SCB);

    core.SYST.set_reload(16_000_000 - 1);
    core.SYST.clear_current();
    core.SYST.enable_interrupt();
    core.SYST.enable_counter();

    SCHEDULER.init(&mut core.NVIC);

    defmt::debug!("scheduled");
    SCHEDULER.schedule::<Mono>(Task::new(Deadline::secs(1), task_1));
    SCHEDULER.idle();
}

// Just testing that the systick interrupt works after relocating the vtable
#[cortex_m_rt::exception]
fn SysTick() {
    defmt::trace!("tick!");
}

fn task_1() {
    defmt::info!("Here is a task");
}
