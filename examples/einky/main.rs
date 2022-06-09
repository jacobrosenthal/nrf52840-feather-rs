//! Print battery percentage on screen every 3 hours
//!
//! Feather nrf52840 express
//! https://www.adafruit.com/product/4062
//! https://learn.adafruit.com/introducing-the-adafruit-nrf52840-feather?view=all
//! https://learn.adafruit.com/assets/68545/
//!
//! Adafruit 2.13" Monochrome eInk / ePaper Display FeatherWing
//! https://www.adafruit.com/product/4195
//! https://learn.adafruit.com/adafruit-2-13-eink-display-breakouts-and-featherwings?view=all
//! As of April 27, 2020 we're selling a version with SSD1680 chipset, instead of the SSD1675 chipset
//! Busy and Rst pin not connected
//!
//! P1_02 button
//! P0_16 neopixel
//! P1_10 led blue
//! P1_15 led red
//! P0_29 battery divided by 2
//!
//! thinkink
//! P0_14 sck
//! P0_13 mosi
//! P0_15 miso
//! skip 3
//! P0_30 rst MUST SOLDER
//!
//! P0_06 11 busy MUST SOLDER
//! P0_27 10 dc
//! P0_26 9 cs
//! P0_07 6 srcs
//! P1_08 5 sd cs
//!
//! DEFMT_LOG=trace cargo run --release --example einky
#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use defmt::info;
use defmt_rtt as _;
use panic_probe as _;

use embassy::interrupt::InterruptExt;
use embassy::time::{Delay, Duration, Timer};
use embassy::util::Forever;
use embassy_nrf::gpio::{self, AnyPin, Pin};
use embassy_nrf::saadc::{self, Saadc};
use embassy_nrf::{interrupt, spim};
use embedded_graphics::{
    mono_font::MonoTextStyleBuilder,
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text, TextStyleBuilder},
};
use embedded_hal_async::spi::ExclusiveDevice;
use heapless::String;
use ssd1680::{DisplayRotation, Ssd1680};

// we make a lazily created static
static EXECUTOR: Forever<embassy::executor::Executor> = Forever::new();

#[cortex_m_rt::entry]
fn main() -> ! {
    // well use these logging macros instead of println to tunnel our logs via the debug chip
    info!("Hello World!");

    // once we hit runtime we create and fill that executor finally
    let executor = EXECUTOR.put(embassy::executor::Executor::new());

    // provides the peripherals from the async first pac if you selected it
    // let dp = embassy_nrf::init(embassy_config());

    // spawn tasks
    executor.run(|spawner| {
        let _ = spawner.spawn(display_task());
    })
}

#[embassy::task]
async fn blinky_task(mut led: gpio::Output<'static, AnyPin>) {
    loop {
        led.set_high();
        Timer::after(Duration::from_millis(300)).await;
        led.set_low();
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
    let spim = spim::Spim::new_txonly(
        &mut dp.SPI3,
        &mut spim_irq,
        &mut dp.P0_14,
        &mut dp.P0_13,
        spim_config,
    );

    let cs = gpio::Output::new(
        dp.P0_26.degrade(),
        gpio::Level::Low,
        gpio::OutputDrive::Standard,
    );
    let spi_dev = ExclusiveDevice::new(spim, cs);
    let dc = gpio::Output::new(
        dp.P0_27.degrade(),
        gpio::Level::Low,
        gpio::OutputDrive::Standard,
    );
    let reset = gpio::Output::new(
        dp.P0_30.degrade(),
        gpio::Level::High,
        gpio::OutputDrive::Standard,
    );
    let busy = gpio::Input::new(dp.P0_06.degrade(), gpio::Pull::Up);

    let mut ssd1680 = Ssd1680::new(spi_dev, dc, reset, busy, DisplayRotation::Rotate0);
    let style = MonoTextStyleBuilder::new()
        .font(&embedded_graphics::mono_font::ascii::FONT_10X20)
        .text_color(BinaryColor::On)
        .background_color(BinaryColor::Off)
        .build();
    let text_style = TextStyleBuilder::new().baseline(Baseline::Top).build();

    let config = saadc::Config::default();
    let irq = interrupt::take!(SAADC);
    let channel_config = saadc::ChannelConfig::single_ended(&mut dp.P0_29);
    let mut saadc = Saadc::new(dp.SAADC, irq, config, [channel_config]);

    loop {
        let mut s: String<3> = String::new();
        get_battery_percentage(&mut s, &mut saadc).await;

        Text::with_text_style(&s, Point::new(0, 0), style, text_style)
            .draw(&mut ssd1680)
            .unwrap();
        ssd1680.flush(&mut Delay).await.unwrap();

        Timer::after(Duration::from_secs(3600 * 3)).await;
    }
}

// Gain = (1/6) REFERENCE = (0.6 V) Vdiv=1/2 RESOLUTION = 12bits
// Max Input = (0.6 V)/(1/6) = 3.6 V
// VBAT_MV_PER_LSB = Max Input/ 2^RESOLUTION
// VBAT_MV_PER_LSB = 3600mV/4096
// V(p) = raw * 1/Vdiv * VBAT_MV_PER_LSB
// V(p) = raw * 2 * 3600/4096
// V(p) = raw * (7200/4096)
// Percentage = V(p) * 100 / 4200
// Percentage = raw * 100 * (7200/4096) / 4200
// Percentage = raw * (720000/4096) / 4200
// Percentage = raw * 720000/17203200
const BATTERY_TRANSLATION: u32 = 720000 / 17203200;
async fn get_battery_percentage(s: &mut String<3>, saadc: &mut Saadc<'_, 1>) {
    let mut battery = [0; 1];
    saadc.sample(&mut battery).await;
    let percentage = (battery[0] as u32 * BATTERY_TRANSLATION) as u8;
    core::fmt::write(s, format_args!("{}%", percentage)).unwrap();
}

// WARNING may overflow and wrap-around in long lived apps
defmt::timestamp! {"{=usize}", {
    use core::sync::atomic::{AtomicUsize, Ordering};

    static COUNT: AtomicUsize = AtomicUsize::new(0);
    // NOTE(no-CAS) `timestamps` runs with interrupts disabled
    let n = COUNT.load(Ordering::Relaxed);
    COUNT.store(n + 1, Ordering::Relaxed);
    n
}}

// 0 is Highest. Lower prio number can preempt higher prio number
// Softdevice has reserved priorities 0, 1 and 3
pub fn embassy_config() -> embassy_nrf::config::Config {
    let mut config = embassy_nrf::config::Config::default();
    config.hfclk_source = embassy_nrf::config::HfclkSource::ExternalXtal;
    config.lfclk_source = embassy_nrf::config::LfclkSource::ExternalXtal;
    config.time_interrupt_priority = interrupt::Priority::P2;
    // if we see button misses lower this
    config.gpiote_interrupt_priority = interrupt::Priority::P7;
    config
}
