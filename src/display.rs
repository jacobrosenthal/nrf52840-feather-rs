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

use crate::bluetooth::SERVER;
use defmt::{info, unwrap};
use embassy::blocking_mutex::raw::ThreadModeRawMutex;
use embassy::channel::mpmc::Receiver;
use embassy::time::{Delay, Duration, Timer};
use embassy::util::{select, Either};
use embassy_nrf::interrupt::{self, InterruptExt};
use embassy_nrf::{gpio, spim};
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Baseline, Text, TextStyleBuilder};
use embedded_hal_async::spi::ExclusiveDevice;
use ssd1680::{DisplayRotation, Ssd1680};

#[embassy::task]
pub async fn display_task(channel: Receiver<'static, ThreadModeRawMutex, bool, 1>) {
    let mut dp = unsafe { <embassy_nrf::Peripherals as embassy::util::Steal>::steal() };
    let mut spim_irq = interrupt::take!(SPIM3);
    spim_irq.set_priority(interrupt::Priority::P4);

    loop {
        info!("Display Update");
        if let Some(server) = SERVER.borrow().borrow().as_ref() {
            let mut spim_config = spim::Config::default();
            spim_config.frequency = spim::Frequency::M32;
            let spim = spim::Spim::new_txonly(
                &mut dp.SPI3,
                &mut spim_irq,
                &mut dp.P0_14,
                &mut dp.P0_13,
                spim_config,
            );

            let cs =
                gpio::Output::new(&mut dp.P0_26, gpio::Level::Low, gpio::OutputDrive::Standard);
            let spi_dev = ExclusiveDevice::new(spim, cs);
            let dc =
                gpio::Output::new(&mut dp.P0_27, gpio::Level::Low, gpio::OutputDrive::Standard);
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

            let message = unwrap!(server.text.message_get());
            info!("message: {=str}", message);

            unwrap!(
                Text::with_text_style(&message, Point::new(0, 0), style, text_style)
                    .draw(&mut ssd1680)
            );

            ssd1680.flush(&mut Delay).await.ok();
        }

        // wait for timer, or a new message from bluetooth to update screen
        match select(
            Timer::after(Duration::from_secs(60 * 60 * 24)),
            channel.recv(),
        )
        .await
        {
            // timer return just continue
            Either::First(_) => {}
            Either::Second(_val) => {
                // todo anything better than a bool in a channel just to trigger?
            }
        }
    }
}
