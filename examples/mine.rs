//! The rust-toolchain will pull in the correct nightly and target so all you
//! need to run is
//!
//! Feather nrf52840 express
//! https://www.adafruit.com/product/4062
//! https://learn.adafruit.com/introducing-the-adafruit-nrf52840-feather?view=all
//! https://learn.adafruit.com/assets/68545/
//!
//! P1.02 button
//! P0.16 nopixl
//!
//! thinkink
//! p0_14 sck
//! p0_13 mosi
//! p0_15 miso
//! skip 3
//!
//! P0_27 10 dc
//! P0_26 9 cs
//! P0_07 6 srcs
//! P1_08 5 sd cs
//! skip 2
//!
//! p1_14 busy not connected, just us as sacrificial
//! p1_13 rst not connected, just us as sacrificial
//!
//! DEFMT_LOG=trace cargo run --release --example bmp
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
        &mut dp.P0_14,
        NoPin,
        &mut dp.P0_13,
        spim_config,
    );

    let mut cs = gpio::Output::new(
        dp.P0_26.degrade(),
        gpio::Level::High,
        gpio::OutputDrive::Standard,
    );
    let mut dc = gpio::Output::new(
        dp.P0_27.degrade(),
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
