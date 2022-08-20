//! Print battery percentage on screen every 3 hours
//!
//! Feather nrf52840 express
//! https://www.adafruit.com/product/4062
//! https://learn.adafruit.com/introducing-the-adafruit-nrf52840-feather?view=all
//! https://learn.adafruit.com/assets/68545/
//!
//! P1_02 button
//! P0_16 neopixel
//! P1_10 led blue
//! P1_15 led red
//! P0_29 battery divided by 2
//!
//! DEFMT_LOG=trace cargo run --release
#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use defmt::{info, unwrap};
use panic_probe as _; // print out panic messages
mod display;
use defmt_rtt as _;
mod saadc;

use display::display_task;
use embassy_executor::Spawner;
use embassy_nrf::config::{Config, HfclkSource, LfclkSource};
use embassy_nrf::interrupt;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let _dp = embassy_nrf::init(embassy_config());

    // well use these logging macros instead of println to tunnel our logs via the debug chip
    info!("Hello World!");

    unwrap!(spawner.spawn(display_task()));
}

// 0 is Highest. Lower prio number can preempt higher prio number
// Softdevice has reserved priorities 0, 1 and 3
pub fn embassy_config() -> Config {
    let mut config = Config::default();
    config.hfclk_source = HfclkSource::ExternalXtal;
    config.lfclk_source = LfclkSource::ExternalXtal;
    config.time_interrupt_priority = interrupt::Priority::P2;
    // if we see button misses lower this
    config.gpiote_interrupt_priority = interrupt::Priority::P7;
    config
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
