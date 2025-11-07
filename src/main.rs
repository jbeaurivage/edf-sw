#![no_std]
#![no_main]

use atsamd_hal::{self as hal};
use defmt_rtt as _;
use hal::rtc::rtic::rtc_clock;
use panic_probe as _;

use edf_sw::{Deadline, scheduler::Scheduler, task::Task};

hal::rtc_monotonic!(Mono, rtc_clock::Clock32k);

#[unsafe(no_mangle)]
static RTIC_ASYNC_MAX_LOGICAL_PRIO: u8 = 4;

static SCHEDULER: Scheduler<Mono> = Scheduler::new();

#[cortex_m_rt::entry]
fn main() -> ! {
    let peripherals = atsamd_hal::pac::Peripherals::take().unwrap();
    let mut core = atsamd_hal::pac::CorePeripherals::take().unwrap();
    Mono::start(peripherals.rtc);

    SCHEDULER.init(&mut core.NVIC, &mut core.SCB);

    core.SYST.set_reload(32_000_000 - 1);
    core.SYST.clear_current();
    core.SYST.enable_interrupt();
    core.SYST.enable_counter();

    SCHEDULER.schedule(Task::new(Deadline::millis(1000), task_1));
    SCHEDULER.schedule(Task::new(Deadline::millis(500), task_1));
    SCHEDULER.idle();
}

// Just testing that the systick interrupt works after relocating the vtable
#[cortex_m_rt::exception]
fn SysTick() {
    SCHEDULER.schedule(Task::new(Deadline::millis(2), task_2));
}

fn task_1() {
    defmt::info!("Here is a task that has been succesfully scheduled!");
}

fn task_2() {
    defmt::debug!("tick");
}
