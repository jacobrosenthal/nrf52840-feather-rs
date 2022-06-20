//! Adafruit 2.13" Monochrome eInk / ePaper Display FeatherWing
//! https://www.adafruit.com/product/4195
//! https://learn.adafruit.com/adafruit-2-13-eink-display-breakouts-and-featherwings?view=all
//! As of April 27, 2020 we're selling a version with SSD1680 chipset, instead of the SSD1675 chipset
//! Busy and Rst pin not connected
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

use crate::saadc::BATTERY;
use defmt::{info, unwrap};
use embassy::time::{Delay, Duration, Instant, Timer};
use embassy::util::{select, Either};
use embassy_nrf::gpio::{self};
use embassy_nrf::interrupt::{self, InterruptExt, SPIM3};
use embassy_nrf::spim;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Baseline, Text, TextStyleBuilder};
use embedded_hal_async::spi::ExclusiveDevice;
use heapless::String;
use ssd1680::{DisplayRotation, Ssd1680};

#[embassy::task]
pub async fn display_task() {
    let mut dp = unsafe { <embassy_nrf::Peripherals as embassy::util::Steal>::steal() };
    let mut spim_irq = interrupt::take!(SPIM3);
    spim_irq.set_priority(interrupt::Priority::P4);

    let mut input = gpio::Input::new(&mut dp.P1_02, gpio::Pull::Up);

    let mut minutes = 0;

    'start: loop {
        info!("waiting");
        display(&mut spim_irq, minutes).await;

        // scope to drop pin, check for long press
        {
            input.wait_for_low().await;
            let start = Instant::now();
            input.wait_for_high().await;
            let duration = start.elapsed();

            // check for long press and clear display
            if duration.as_secs() > 2 {
                info!("reseting");
                minutes = 0;
                continue 'start;
            }
        }

        info!("timing");

        'timing: loop {
            // count minutes until button press to stop timing
            match select(Timer::after(Duration::from_secs(60)), input.wait_for_low()).await {
                // timer return just continue
                Either::First(_) => {
                    minutes += 1;
                    display(&mut spim_irq, minutes).await;
                    continue 'timing;
                }
                Either::Second(_val) => {
                    input.wait_for_high().await;
                    continue 'start;
                }
            }
        }
    }
}

async fn display(irq: &mut SPIM3, minutes: u32) {
    let mut dp = unsafe { <embassy_nrf::Peripherals as embassy::util::Steal>::steal() };

    let mut spim_config = spim::Config::default();
    spim_config.frequency = spim::Frequency::M32;
    let spim = spim::Spim::new_txonly(&mut dp.SPI3, irq, &mut dp.P0_14, &mut dp.P0_13, spim_config);

    let cs = gpio::Output::new(&mut dp.P0_26, gpio::Level::Low, gpio::OutputDrive::Standard);
    let spi_dev = ExclusiveDevice::new(spim, cs);
    let dc = gpio::Output::new(&mut dp.P0_27, gpio::Level::Low, gpio::OutputDrive::Standard);
    let busy = gpio::Input::new(&mut dp.P0_06, gpio::Pull::Up);
    let reset = gpio::Output::new(
        &mut dp.P0_30,
        gpio::Level::High,
        gpio::OutputDrive::Standard,
    );

    let mut ssd1680 = Ssd1680::new(spi_dev, dc, reset, busy, DisplayRotation::Rotate0);
    let style = MonoTextStyleBuilder::new()
        .font(&embedded_graphics::mono_font::ascii::FONT_10X20)
        .text_color(BinaryColor::On)
        .background_color(BinaryColor::Off)
        .build();
    let text_style = TextStyleBuilder::new().baseline(Baseline::Top).build();

    let hours = minutes / 60;
    let hours = hours.min(99);
    let minutes = minutes % 60;
    let mut clock_string: String<5> = String::new(); //99:99 is 5 chars
    core::fmt::write(
        &mut clock_string,
        format_args!("{:02}:{:02}", hours, minutes),
    )
    .ok();
    unwrap!(
        Text::with_text_style(&clock_string, Point::new(0, 0), style, text_style)
            .draw(&mut ssd1680)
    );

    if let Some(percent) = BATTERY.borrow().borrow().as_ref() {
        let mut percent_string: String<3> = String::new(); //99% is 3 chars
        core::fmt::write(&mut percent_string, format_args!("{:02}%", percent)).ok();
        unwrap!(
            Text::with_text_style(&percent_string, Point::new(220, 0), style, text_style)
                .draw(&mut ssd1680)
        );
    }

    ssd1680.flush(&mut Delay).await.ok();
}
