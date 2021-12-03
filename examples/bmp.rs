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
use defmt_rtt as _; // global logger

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
use epd_waveshare::color::*;
use epd_waveshare::epd2in13_v2::{Display2in13, Epd2in13};
use epd_waveshare::graphics::DisplayRotation;
use epd_waveshare::prelude::*;

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

    //  not connected, just us as sacrificial
    let busy = gpio::Input::new(dp.P1_14, gpio::Pull::Down);
    // not connected, just us as sacrificial
    let reset = gpio::Output::new(dp.P1_13, gpio::Level::High, gpio::OutputDrive::Standard);
    let cs = gpio::Output::new(dp.P0_26, gpio::Level::High, gpio::OutputDrive::Standard);
    let dc = gpio::Output::new(dp.P0_27, gpio::Level::Low, gpio::OutputDrive::Standard);

    let mut epd =
        Epd2in13::new(&mut spim, cs, busy, dc, reset, &mut Delay).expect("eink initalize error");

    // // Clear the full screen
    // let _ = epd.clear_frame(&mut spim, &mut Delay);
    // let _ = epd.display_frame(&mut spim, &mut Delay);

    // // Speeddemo
    // let _ = epd.set_lut(&mut spim, Some(RefreshLut::Quick));
    // let small_buffer = [Color::Black.get_byte_value(); 32]; //16x16
    // let number_of_runs = 1;
    // for i in 0..number_of_runs {
    //     let offset = i * 8 % 150;
    //     let _ = epd.update_partial_frame(
    //         &mut spim,
    //         &small_buffer,
    //         25 + offset,
    //         25 + offset,
    //         16,
    //         16,
    //         &mut Delay,
    //     );
    //     let _ = epd.display_frame(&mut spim, &mut Delay);
    // }

    // Clear the full screen
    let _ = epd.clear_frame(&mut spim, &mut Delay);
    let _ = epd.display_frame(&mut spim, &mut Delay);

    // // Draw some squares
    // // let small_buffer = [Color::Black.get_byte_value(); 3200]; //160x160
    // // let _ = epd.update_partial_frame(&mut spim, &small_buffer, 20, 20, 160, 160, &mut Delay);

    let small_buffer = [Color::White.get_byte_value(); 800]; //80x80
    let _ = epd.update_partial_frame(&mut spim, &small_buffer, 60, 60, 80, 80, &mut Delay);

    let small_buffer = [Color::Black.get_byte_value(); 8]; //8x8
    let _ = epd.update_partial_frame(&mut spim, &small_buffer, 96, 96, 8, 8, &mut Delay);

    // info!("writing!");

    // Display updated frame
    let _ = epd.display_frame(&mut spim, &mut Delay);
    Timer::after(Duration::from_millis(5000)).await;

    // Set the EPD to sleep
    let _ = epd.sleep(&mut spim, &mut Delay);

    info!("sleepy now");

    // info!("Test all the rotations");
    // let mut display = Display2in13::default();

    // display.set_rotation(DisplayRotation::Rotate0);
    // draw_text(&mut display, "Rotate 0!", 5, 50);

    // display.set_rotation(DisplayRotation::Rotate90);
    // draw_text(&mut display, "Rotate 90!", 5, 50);

    // display.set_rotation(DisplayRotation::Rotate180);
    // draw_text(&mut display, "Rotate 180!", 5, 50);

    // display.set_rotation(DisplayRotation::Rotate270);
    // draw_text(&mut display, "Rotate 270!", 5, 50);

    // let _ = epd.update_frame(&mut spim, display.buffer(), &mut Delay);
    // epd
    //     .display_frame(&mut spim, &mut Delay)
    //     .expect("display frame new graphics");
    // Timer::after(Duration::from_millis(5000)).await;

    // info!("Now test new graphics with default rotation and some special stuff:");
    // display.clear_buffer(Color::White);

    // // draw a analog clock
    // let _ = Circle::with_center(Point::new(64, 64), 80)
    //     .into_styled(PrimitiveStyle::with_stroke(Black, 1))
    //     .draw(&mut display);
    // let _ = Line::new(Point::new(64, 64), Point::new(30, 40))
    //     .into_styled(PrimitiveStyle::with_stroke(Black, 4))
    //     .draw(&mut display);
    // let _ = Line::new(Point::new(64, 64), Point::new(80, 40))
    //     .into_styled(PrimitiveStyle::with_stroke(Black, 1))
    //     .draw(&mut display);

    // // draw white on black background
    // let style = MonoTextStyleBuilder::new()
    //     .font(&embedded_graphics::mono_font::ascii::FONT_6X10)
    //     .text_color(White)
    //     .background_color(Black)
    //     .build();
    // let text_style = TextStyleBuilder::new().baseline(Baseline::Top).build();

    // let _ = Text::with_text_style("It's working-WoB!", Point::new(90, 10), style, text_style)
    //     .draw(&mut display);

    // // use bigger/different font
    // let style = MonoTextStyleBuilder::new()
    //     .font(&embedded_graphics::mono_font::ascii::FONT_10X20)
    //     .text_color(White)
    //     .background_color(Black)
    //     .build();

    // let _ = Text::with_text_style("It's working\nWoB!", Point::new(90, 40), style, text_style)
    //     .draw(&mut display);

    // // Demonstrating how to use the partial refresh feature of the screen.
    // // Real animations can be used.
    // epd
    //     .set_refresh(&mut spim, &mut Delay, RefreshLut::Quick)
    //     .unwrap();
    // epd.clear_frame(&mut spim, &mut Delay).unwrap();

    // // a moving `Hello World!`
    // let limit = 10;
    // for i in 0..limit {
    //     draw_text(&mut display, "  Hello World! ", 5 + i * 12, 50);

    //     epd
    //         .update_and_display_frame(&mut spim, display.buffer(), &mut Delay)
    //         .expect("display frame new graphics");
    //     Timer::after(Duration::from_millis(1000)).await;
    // }

    // // Show a spinning bar without any delay between frames. Shows how «fast»
    // // the screen can refresh for this kind of change (small single character)
    // display.clear_buffer(Color::White);
    // epd
    //     .update_and_display_frame(&mut spim, display.buffer(), &mut Delay)
    //     .unwrap();

    // let spinner = ["|", "/", "-", "\\"];
    // for i in 0..10 {
    //     display.clear_buffer(Color::White);
    //     draw_text(&mut display, spinner[i % spinner.len()], 10, 100);
    //     epd
    //         .update_and_display_frame(&mut spim, display.buffer(), &mut Delay)
    //         .unwrap();
    // }

    // info!("Finished tests - going to sleep");
    // let _ = epd.sleep(&mut spim, &mut Delay);

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
