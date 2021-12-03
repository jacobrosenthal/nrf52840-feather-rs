//! The rust-toolchain will pull in the correct nightly and target so all you
//! need to run is
//!
//! Feather xenon
//! https://docs.particle.io/datasheets/discontinued/xenon-datasheet/
//! https://docs.particle.io/assets/images/xenon/xenon-pinout-v1.0.pdf
//! https://docs.particle.io/assets/images/xenon/xenon-block-diagram.png
//!
//! antenna selection
//! p025 = 0, p0.24 = 1 pcb antenna
//! p025 = 1, p0.24 = 0 external u.fl
//!
//! p0.13 red rgb
//! p0.14 green rgb
//! p0.15 blue rgb
//! p0.11 button
//! p1.12 blue led
//!
//! p0.27 scl
//! p0.26 sda
//!
//! thinkink
//! p1.01 sd_cs
//! p0.31 ss
//! p1.10 sram_cs d5 6  
//!
//! xenon is not really a feather is it
//! ssd1680
//! p1.08   cs   xenon d4 featherd9
//! p1.02   dc   xenon d3 featherd10
//! p1.15   sck
//! p1.13   mosi
//!
//! DEFMT_LOG=trace cargo run --release --example bmp
//!

#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use defmt::info;
use defmt_rtt as _;
mod ssd1680;

use core::future::pending;
use embassy::interrupt::InterruptExt;
use embassy::time::{Delay, Duration, Timer};
use embassy::util::Forever;
use embassy_nrf::gpio::{self, AnyPin, NoPin, Pin};
use embassy_nrf::{interrupt, spim};
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Circle, Line, PrimitiveStyle};
use embedded_graphics::text::{Baseline, Text, TextStyleBuilder};
use embedded_hal::digital::v2::OutputPin;
use ssd1680::{display, power_up, HEIGHT, WIDTH};

// we make a lazily created static
static EXECUTOR: Forever<embassy::executor::Executor> = Forever::new();

#[cortex_m_rt::entry]
fn main() -> ! {
    // well use these logging macros instead of println to tunnel our logs via the debug chip
    info!("Hello World!");

    // once we hit runtime we create and fill that executor finally
    let executor = EXECUTOR.put(embassy::executor::Executor::new());

    // provides the peripherals from the async first pac if you selected it
    let dp = embassy_nrf::init(Default::default());

    let blue = gpio::Output::new(
        // degrade just a typesystem hack to forget which pin it is so we can
        // call it Anypin and make our function calls more generic
        dp.P1_12.degrade(),
        gpio::Level::High,
        gpio::OutputDrive::Standard,
    );

    // spawn tasks
    executor.run(|spawner| {
        let _ = spawner.spawn(blinky_task(blue));
        let _ = spawner.spawn(display_task());
    })
}

#[embassy::task]
async fn blinky_task(mut led: gpio::Output<'static, AnyPin>) {
    loop {
        led.set_high().unwrap();
        Timer::after(Duration::from_millis(300)).await;
        led.set_low().unwrap();
        Timer::after(Duration::from_millis(1000)).await;
    }
}

#[embassy::task]
pub async fn display_task() {
    // Too lazy to pass all the pins and peripherals we need.
    // Safety: Fragile but safe as long as pins and peripherals arent used
    // anywhere else
    let mut dp = unsafe { <embassy_nrf::Peripherals as embassy::util::Steal>::steal() };

    let mut spim_irq = interrupt::take!(SPIM3);
    spim_irq.set_priority(interrupt::Priority::P4);

    let mut spim_config = spim::Config::default();
    spim_config.frequency = spim::Frequency::M4;
    let mut spim = spim::Spim::new(
        &mut dp.SPI3,
        &mut spim_irq,
        &mut dp.P1_15,
        NoPin,
        &mut dp.P1_13,
        spim_config,
    );

    //  busy pin isnt accessible.. just a sacrificial pin dp.P0_16 pulled down?
    let mut cs = gpio::Output::new(
        dp.P1_08.degrade(),
        gpio::Level::High,
        gpio::OutputDrive::Standard,
    );
    let mut dc = gpio::Output::new(
        dp.P1_02.degrade(),
        gpio::Level::Low,
        gpio::OutputDrive::Standard,
    );

    let mut buffer = [0u8; WIDTH as usize * HEIGHT as usize / 8];

    Timer::after(Duration::from_millis(500)).await;
    info!("going!");

    power_up(&mut spim, &mut cs, &mut dc);
    Timer::after(Duration::from_millis(500)).await;
    display(&mut spim, &mut buffer, &mut cs, &mut dc);
    Timer::after(Duration::from_millis(500)).await;

    info!("sleeep!");

    pending::<()>().await;
}

#[panic_handler] // panicking behavior
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        cortex_m::asm::bkpt();
    }
}

// WARNING may overflow and wrap-around in long lived apps
defmt::timestamp! {"{=usize}", {
    use core::sync::atomic::{AtomicUsize, Ordering};

    static COUNT: AtomicUsize = AtomicUsize::new(0);
    // NOTE(no-CAS) `timestamps` runs with interrupts disabled
    let n = COUNT.load(Ordering::Relaxed);
    COUNT.store(n + 1, Ordering::Relaxed);
    n
}
}
