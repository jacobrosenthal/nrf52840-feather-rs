//! Gain = (1/6) REFERENCE = (0.6 V) RESOLUTION = 12bits VDIV = 1/2
//! Max Input = (0.6 V)/(1/6) = 3.6 V
//! batmin 3401, works down to 3300 but its dropping so fast at that point
//! ble and stuff works under that, but screen doesnt refresh
//! VBAT_MV_PER_LSB = Max Input/ 2^RESOLUTION
//! VBAT_MV_PER_LSB = 3600mV/4096
//! mv = raw * (1/VDIV) * VBAT_MV_PER_LSB
//! mv = raw * (7200/4096)

use embassy_nrf::interrupt::SAADC;
use embassy_nrf::saadc;

pub const MIN: u32 = 3400;
pub const MAX: u32 = 4200;

pub async fn battery_mv(irq: &mut SAADC) -> u32 {
    let mut dp = unsafe { <embassy_nrf::Peripherals as embassy::util::Steal>::steal() };

    let config = saadc::Config::default();
    let channel_config = saadc::ChannelConfig::single_ended(&mut dp.P0_29);
    let mut saadc = saadc::Saadc::new(&mut dp.SAADC, irq, config, [channel_config]);

    let mut battery = [0; 1];
    saadc.sample(&mut battery).await;

    let mv = battery[0] as u32 * 7200 / 4096;
    mv
}

pub fn percent_from_mv<const MIN: u32, const MAX: u32>(mv: u32) -> u8 {
    let mv = mv.min(MAX);
    let mv = mv.max(MIN);
    let percent = (100 * ((mv + 1) - MIN)) / (MAX - MIN);

    // SAFETY: has to be between 0 and 99
    percent as u8
}
