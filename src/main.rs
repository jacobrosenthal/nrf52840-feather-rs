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
//! DEFMT_LOG=trace cargo run --release
#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use defmt::{info, unwrap};
use nrf_softdevice_defmt_rtt as _; // global logger
use panic_probe as _; // print out panic messages
mod bluetooth;
mod display;
mod saadc;

use bluetooth::{bluetooth_task, softdevice_config, softdevice_task};
use display::display_task;
use embassy::blocking_mutex::raw::ThreadModeRawMutex;
use embassy::channel::mpmc::Channel;
use embassy::executor::Spawner;
use embassy::time::{Duration, Timer};
use embassy::util::Forever;
use embassy_nrf::config::{Config, HfclkSource, LfclkSource};
use embassy_nrf::{interrupt, Peripherals};
use nrf_softdevice::Softdevice;
use nrf_softdevice_s140::{
    sd_power_dcdc_mode_set, sd_power_system_off, NRF_POWER_DCDC_MODES_NRF_POWER_DCDC_ENABLE,
};
use saadc::{battery_mv, battery_task, percent_from_mv, MAX, MIN};

static CHANNEL: Forever<Channel<ThreadModeRawMutex, bool, 1>> = Forever::new();

#[embassy::main(config = "embassy_config()")]
async fn main(spawner: Spawner, _dp: Peripherals) {
    // well use these logging macros instead of println to tunnel our logs via the debug chip
    info!("Hello World!");

    // some bluetooth under the covers stuff we need to start up
    let config = softdevice_config();
    let sd = Softdevice::enable(&config);

    let mut saadc_irq = interrupt::take!(SAADC);
    let mv = battery_mv(&mut saadc_irq).await;
    let percent = percent_from_mv::<MIN, MAX>(mv);
    if percent < 1 {
        unsafe { sd_power_system_off() };
    }

    // save battery
    unsafe {
        sd_power_dcdc_mode_set(NRF_POWER_DCDC_MODES_NRF_POWER_DCDC_ENABLE as u8);
    }

    let c = CHANNEL.put(Channel::new());

    unwrap!(spawner.spawn(softdevice_task(sd)));
    unwrap!(spawner.spawn(bluetooth_task(sd, c.sender(), percent)));
    // wait till SERVER Mutex is ready
    Timer::after(Duration::from_secs(1)).await;
    unwrap!(spawner.spawn(battery_task(saadc_irq)));
    unwrap!(spawner.spawn(display_task(c.receiver())));
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
