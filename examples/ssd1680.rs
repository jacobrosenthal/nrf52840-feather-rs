use embassy_nrf::gpio::{AnyPin, Output};
use embedded_hal::digital::v2::OutputPin;

/// Maximum display height this driver supports
pub const HEIGHT: u8 = 250;

/// Maximum display width this driver supports
pub const WIDTH: u8 = 122;

pub fn display<S>(spim: &mut S, buffer: &mut [u8], cs: &mut Output<AnyPin>, dc: &mut Output<AnyPin>)
where
    S: embassy::traits::spi::Write<u8>,
{
    power_up(spim, cs, dc);

    // Set X & Y ram counters
    set_ram_address(spim, cs, dc);

    write_ramframebuffer_to_epd(spim, buffer, cs, dc);
}

pub fn set_ram_address<S>(spim: &mut S, cs: &mut Output<AnyPin>, dc: &mut Output<AnyPin>)
where
    S: embassy::traits::spi::Write<u8>,
{
    epd_command(spim, SSD1680_SET_RAMXCOUNT, cs, dc, false);
    epd_data(spim, &[0x01], cs, dc);

    epd_command(spim, SSD1680_SET_RAMYCOUNT, cs, dc, false);
    epd_data(spim, &[0x00, 0x00], cs, dc);
}

pub fn write_ramframebuffer_to_epd<S>(
    spim: &mut S,
    buffer: &mut [u8],
    cs: &mut Output<AnyPin>,
    dc: &mut Output<AnyPin>,
) where
    S: embassy::traits::spi::Write<u8>,
{
    epd_command(spim, SSD1680_WRITE_RAM1, cs, dc, false);
    epd_data(spim, buffer, cs, dc);
}

pub fn power_up<S>(spim: &mut S, cs: &mut Output<AnyPin>, dc: &mut Output<AnyPin>)
where
    S: embassy::traits::spi::Write<u8>,
{
    epd_command(spim, SSD1680_DATA_MODE, cs, dc, false); // Ram data entry mode
    epd_data(spim, &[0x03], cs, dc); // Ram data entry mode

    epd_command(spim, SSD1680_WRITE_BORDER, cs, dc, false); // border color
    epd_data(spim, &[0x05], cs, dc); // border color

    epd_command(spim, SSD1680_WRITE_VCOM, cs, dc, false); // Vcom Voltage
    epd_data(spim, &[0x36], cs, dc); // Vcom Voltage

    epd_command(spim, SSD1680_GATE_VOLTAGE, cs, dc, false); // Set gate voltage
    epd_data(spim, &[0x17], cs, dc); // Set gate voltage

    epd_command(spim, SSD1680_SOURCE_VOLTAGE, cs, dc, false); // Set source voltage
    epd_data(spim, &[0x41, 0x00, 0x32], cs, dc); // Set source voltage

    epd_command(spim, SSD1680_SET_RAMXCOUNT, cs, dc, false);
    epd_data(spim, &[1], cs, dc);

    epd_command(spim, SSD1680_SET_RAMYCOUNT, cs, dc, false);
    epd_data(spim, &[0, 0], cs, dc);

    let mut height = HEIGHT;
    if (height % 8) != 0 {
        height += 8 - (height % 8);
    }

    // Set ram X start/end postion
    epd_command(spim, SSD1680_SET_RAMXPOS, cs, dc, false);
    epd_data(spim, &[0x01, height / 8], cs, dc);

    // Set ram Y start/end postion
    epd_command(spim, SSD1680_SET_RAMYPOS, cs, dc, false);
    epd_data(spim, &[0x00, 0x00, (WIDTH - 1), 0x00], cs, dc);

    // Set display size and driver output control
    epd_command(spim, SSD1680_DRIVER_CONTROL, cs, dc, false);
    epd_data(spim, &[(WIDTH - 1), 0x00, 0x00], cs, dc);
}

fn epd_command<S>(
    spim: &mut S,
    command: u8,
    cs: &mut Output<AnyPin>,
    dc: &mut Output<AnyPin>,
    end: bool,
) where
    S: embassy::traits::spi::Write<u8>,
{
    let _ = cs.set_high();
    let _ = dc.set_low();
    let _ = cs.set_low();

    spim.write(&[command]);

    if end {
        let _ = cs.set_high();
    }
}

fn epd_data<S>(spim: &mut S, buffer: &[u8], cs: &mut Output<AnyPin>, dc: &mut Output<AnyPin>)
where
    S: embassy::traits::spi::Write<u8>,
{
    let _ = dc.set_high();
    spim.write(buffer);
    let _ = cs.set_high();
}

const SSD1680_DRIVER_CONTROL: u8 = 0x01;
const SSD1680_GATE_VOLTAGE: u8 = 0x03;
const SSD1680_SOURCE_VOLTAGE: u8 = 0x04;
const SSD1680_DATA_MODE: u8 = 0x11;
const SSD1680_WRITE_RAM1: u8 = 0x24;
const SSD1680_WRITE_VCOM: u8 = 0x2C;
const SSD1680_WRITE_BORDER: u8 = 0x3C;
const SSD1680_SET_RAMXPOS: u8 = 0x44;
const SSD1680_SET_RAMYPOS: u8 = 0x45;
const SSD1680_SET_RAMXCOUNT: u8 = 0x4E;
const SSD1680_SET_RAMYCOUNT: u8 = 0x4F;
