#![no_std]
#![no_main]

use atsamd_hal::clock::GenericClockController;
use atsamd_hal::pac::{Interrupt, NVIC, Peripherals, interrupt};
use atsamd_hal::prelude::InterruptDrivenTimer;
use atsamd_hal::timer::TimerCounter;
use atsamd_hal::{self as hal};

use cortex_m::asm;
use cortex_m::peripheral::DWT;
use cortex_m::peripheral::scb::SystemHandler;

use fugit::ExtU32;
use hal::rtc::rtic::rtc_clock;

use edf_sw::Deadline;
use edf_sw::scheduler::Scheduler;
use edf_sw::task::Task;

use {defmt_rtt as _, panic_probe as _};

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
        core.SCB.set_priority(SystemHandler::SysTick, 0);
        NVIC::unpend(Interrupt::TC4);
        NVIC::unmask(Interrupt::TC4);
        core.NVIC.set_priority(Interrupt::TC4, 0);
    };

    let timer_clock = clocks.gclk0();
    let tc45 = &clocks.tc4_tc5(&timer_clock).unwrap();

    // Enable cycle counter for measurements
    DWT::unlock();
    unsafe {
        core.DCB.demcr.modify(|r| r | (1 << 24));
    }

    SCHEDULER.init(&mut core.NVIC, &mut core.SCB);

    // Instantiate a timer object for the TC4 timer/counter
    let mut timer = TimerCounter::tc4_(tc45, peripherals.tc4, &mut peripherals.mclk);
    timer.start(100.millis());
    // timer.enable_interrupt();

    core.SYST.set_reload(8_000_000 - 1);
    core.SYST.clear_current();
    core.SYST.enable_interrupt();
    // core.SYST.enable_counter();

    for i in 0..=16 {
        let deadline = Deadline::millis(i + 1);
        reset_dwt();
        SCHEDULER.schedule(Task::new(deadline, software_task));
    }

    reset_dwt();
    SCHEDULER.schedule(Task::new(Deadline::micros(60), software_task));

    defmt::debug!("[IDLE START]");
    SCHEDULER.idle();
}

#[cortex_m_rt::exception]
fn SysTick() {
    reset_dwt();
    SCHEDULER.schedule(Task::new(Deadline::millis(2), systick_task));
}

#[interrupt]
fn TC4() {
    let tc4 = unsafe { Peripherals::steal().tc4 };
    tc4.count16().intflag().write(|w| w.ovf().set_bit());
    SCHEDULER.schedule(Task::new(Deadline::millis(20), timer_task));
}

fn software_task() {
    // Simulate blocking roughly for 2s with CPU running at 48 MHz
    asm::delay(96_000_000);
    defmt::info!("[TASK 0] Software task complete");
    // SCHEDULER.schedule(Task::new(Deadline::millis(1000), software_task));
}

fn systick_task() {
    asm::delay(4_000);
    defmt::info!("[TASK 2] Systick task complete");
}

fn timer_task() {
    asm::delay(8_000);
    defmt::info!("[TASK 1] Timer task complete");
}

fn reset_dwt() {
    let mut dwt = unsafe { cortex_m::peripheral::Peripherals::steal() }.DWT;
    dwt.set_cycle_count(0);
    dwt.enable_cycle_counter();
}
