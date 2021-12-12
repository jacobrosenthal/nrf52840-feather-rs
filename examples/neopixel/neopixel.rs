use embassy::time::{Duration, Timer};
use embassy_nrf::gpio::NoPin;
use embassy_nrf::pwm::{Prescaler, SequenceConfig, SequenceMode, SequencePwm};
use smart_leds::{colors, RGB8};

#[embassy::task]
pub async fn neopixel_task() {
    // Safety: Too lazy to pass all the pins and peripherals we need.
    // Safe but fragile As long as pins and peripherals arent used anywhere else
    let dp = unsafe { <embassy_nrf::Peripherals as embassy::util::Steal>::steal() };

    let mut neopixel = dp.P0_16;
    let mut pwm_peripheral = dp.PWM0;

    let pattern = [colors::GREEN, colors::RED, colors::BLUE, RGB8::default()];
    loop {
        for color in pattern {
            // need 24 bytes for our 1 led
            // and another 24 bytes of 0x8000 is the latch, we dont want to overwrite that
            let mut seq_values = [0x8000; 48];

            // fill up the first 24 bytes (our single neopixel)
            fill_buf(&color, &mut seq_values[0..24]).unwrap();

            let mut config = SequenceConfig::default();
            config.prescaler = Prescaler::Div1;
            config.max_duty = 20;

            let pwm = SequencePwm::new(
                &mut pwm_peripheral,
                &mut neopixel,
                NoPin,
                NoPin,
                NoPin,
                config,
                &seq_values,
            )
            .unwrap();
            let _ = pwm.start(SequenceMode::Times(1));

            Timer::after(Duration::from_millis(1000)).await;
        }
    }
}

pub fn fill_buf(color: &RGB8, buf: &mut [u16]) -> Result<(), ()> {
    if buf.len() < 24 {
        return Err(());
    }

    let red = color.r.reverse_bits();
    let green = color.g.reverse_bits();
    let blue = color.b.reverse_bits();

    for (g, item) in buf.iter_mut().enumerate().take(8) {
        if ((green >> g) & 0b1) == 0b1 {
            *item = 0x8000 | 13;
        } else {
            *item = 0x8000 | 5;
        }
    }

    for (r, item) in buf.iter_mut().enumerate().skip(8).take(8) {
        if ((red >> r) & 0b1) == 0b1 {
            *item = 0x8000 | 13;
        } else {
            *item = 0x8000 | 5;
        }
    }

    for (b, item) in buf.iter_mut().enumerate().skip(16).take(8) {
        if ((blue >> b) & 0b1) == 0b1 {
            *item = 0x8000 | 13;
        } else {
            *item = 0x8000 | 5;
        }
    }

    Ok(())
}
