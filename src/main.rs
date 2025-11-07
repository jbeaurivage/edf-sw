#![no_std]
#![no_main]

use atsamd_hal::clock::GenericClockController;
use atsamd_hal::pac::{Interrupt, NVIC, Peripherals, interrupt};
use atsamd_hal::prelude::InterruptDrivenTimer;
use atsamd_hal::timer::TimerCounter;
use atsamd_hal::{self as hal};
use cortex_m::peripheral::scb::SystemHandler;
use fugit::ExtU32;
use hal::rtc::rtic::rtc_clock;
use {defmt_rtt as _, panic_probe as _};

use edf_sw::Deadline;
use edf_sw::scheduler::Scheduler;
use edf_sw::task::Task;

hal::rtc_monotonic!(Mono, rtc_clock::Clock32k);

#[unsafe(no_mangle)]
static RTIC_ASYNC_MAX_LOGICAL_PRIO: u8 = 4;

static SCHEDULER: Scheduler<Mono> = Scheduler::new();

#[cortex_m_rt::entry]
fn main() -> ! {
    let mut peripherals = atsamd_hal::pac::Peripherals::take().unwrap();
    let mut core = atsamd_hal::pac::CorePeripherals::take().unwrap();
    Mono::start(peripherals.rtc);

    let mut clocks = GenericClockController::with_external_32kosc(
        peripherals.gclk,
        &mut peripherals.mclk,
        &mut peripherals.osc32kctrl,
        &mut peripherals.oscctrl,
        &mut peripherals.nvmctrl,
    );

    unsafe {
        core.SCB.set_priority(SystemHandler::SysTick, 4);
        NVIC::unpend(Interrupt::TC4);
        NVIC::unmask(Interrupt::TC4);
        core.NVIC.set_priority(Interrupt::TC4, 4);
    };

    // configure a clock for the TC4 and TC5 peripherals
    let timer_clock = clocks.gclk0();
    let tc45 = &clocks.tc4_tc5(&timer_clock).unwrap();

    // instantiate a timer object for the TC4 peripheral
    let mut timer = TimerCounter::tc4_(tc45, peripherals.tc4, &mut peripherals.mclk);
    timer.start(500.millis());
    timer.enable_interrupt();

    SCHEDULER.init(&mut core.NVIC, &mut core.SCB);

    core.SYST.set_reload(32_000_000 - 1);
    core.SYST.clear_current();
    core.SYST.enable_interrupt();
    core.SYST.enable_counter();

    // SCHEDULER.schedule(Task::new(Deadline::millis(500), manual_task));

    defmt::debug!("REACHED IDLE");
    SCHEDULER.idle();
}

// Just testing that the systick interrupt works after relocating the vtable
#[cortex_m_rt::exception]
fn SysTick() {
    SCHEDULER.schedule(Task::new(Deadline::millis(2), systick_task));
}

#[interrupt]
fn TC4() {
    let tc4 = unsafe { Peripherals::steal().tc4 };
    tc4.count16().intflag().write(|w| w.ovf().set_bit());
    SCHEDULER.schedule(Task::new(Deadline::millis(20), timer_task));
}

fn manual_task() {
    // Roughly 2s delay with CPU running at 48 MHz
    cortex_m::asm::delay(96_000_000);
    defmt::info!("Here is a task that has been succesfully scheduled!");
    SCHEDULER.schedule(Task::new(Deadline::millis(1000), manual_task));
}

fn systick_task() {
    defmt::info!("tick");
}

fn timer_task() {
    defmt::info!("Timer task");
}
