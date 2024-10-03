#![no_std]
#![no_main]

use cortex_m::delay::Delay;
use defmt::info;
use display_interface::{DataFormat, DisplayError, WriteOnlyDataCommand};
use embedded_graphics::{pixelcolor::Rgb565, prelude::IntoStorage};
use embedded_hal::digital::OutputPin;
use rp2040_hal::pio::{Tx, ValidStateMachine};

mod graphics;

type Result<T = ()> = core::result::Result<T, DisplayError>;

pub struct Pio16BitBus<SM: ValidStateMachine, DC> {
    tx: Tx<SM>,
    dc: DC,
}

// pub trait WriteOnlyDataCommand {
//     fn send_commands(&mut self, cmd: u16);
//     fn send_data(&mut self, buf: u16);
// }

impl<TX, DC> Pio16BitBus<TX, DC>
where
    TX: ValidStateMachine,
    DC: OutputPin,
{
    pub fn new(tx: Tx<TX>, dc: DC) -> Self {
        Self { tx, dc }
    }

    fn write_iter(&mut self, iter: impl Iterator<Item = u8>) -> Result {
        for value in iter {
            self.tx.write(value as u32);
        }
        Ok(())
    }

    fn write_iter16(&mut self, iter: impl Iterator<Item = u16>) -> Result {
        for value in iter {
            self.tx.write(value as u32);
        }
        Ok(())
    }

    pub fn write_data(&mut self, data: DataFormat<'_>) -> Result {
        match data {
            DataFormat::U8(slice) => self.write_iter(slice.iter().copied()),
            // DataFormat::U16(slice) => self.write_iter16(slice.iter().copied()),
            // DataFormat::U16BE(slice) => self.write_iter16(slice.iter().copied().map(u16::to_be)),
            DataFormat::U16LE(slice) => self.write_iter16(slice.iter().copied().map(u16::to_le)),
            DataFormat::U16LEIter(iter) => self.write_iter16(iter),
            _ => Err(DisplayError::DataFormatNotImplemented),
        }
    }
}

impl<TX, DC> WriteOnlyDataCommand for Pio16BitBus<TX, DC>
where
    TX: ValidStateMachine,
    DC: OutputPin,
{
    fn send_commands(&mut self, cmd: DataFormat<'_>) -> Result {
        self.dc.set_low().map_err(|_| DisplayError::DCError)?;
        self.write_data(cmd)?;
        Ok(())
    }
    fn send_data(&mut self, buf: DataFormat<'_>) -> Result {
        self.dc.set_high().map_err(|_| DisplayError::DCError)?;
        self.write_data(buf)?;
        Ok(())
    }
}

pub struct ILI9488<DI, RST, BL>
where
    DI: WriteOnlyDataCommand,
    RST: OutputPin,
    BL: OutputPin,
{
    di: DI,
    rst: Option<RST>,
    bl: Option<BL>,

    size_x: u16,
    size_y: u16,
}

impl<DI, RST, BL> ILI9488<DI, RST, BL>
where
    DI: WriteOnlyDataCommand,
    RST: OutputPin,
    BL: OutputPin,
{
    pub fn new(di: DI, rst: Option<RST>, bl: Option<BL>, size_x: u16, size_y: u16) -> Self {
        Self {
            di,
            rst,
            bl,
            size_x,
            size_y,
        }
    }

    pub fn init_test(&mut self) -> Result {
        self.write_command(0x55)?;
        Ok(())
    }

    pub fn init(&mut self, delay_source: &mut Delay) -> Result {
        self.hard_reset(delay_source);

        if let Some(bl) = self.bl.as_mut() {
            bl.set_high().unwrap();
        }

        self.write_command(0xf7)?;
        self.write_data(0xa9)?;
        self.write_data(0x51)?;
        self.write_data(0x2c)?;
        self.write_data(0x82)?;

        self.write_command(0xc0)?;
        self.write_data(0x11)?;
        self.write_data(0x09)?;

        self.write_command(0xc1)?;
        self.write_data(0x41)?;

        self.write_command(0xc5)?;
        self.write_data(0x00)?;
        self.write_data(0x28)?;
        self.write_data(0x80)?;

        self.write_command(0xb1)?;
        self.write_data(0xb0)?;
        self.write_data(0x11)?;

        self.write_command(0xb4)?;
        self.write_data(0x02)?;

        self.write_command(0xb6)?;
        self.write_data(0x02)?;
        self.write_data(0x22)?;

        self.write_command(0xb7)?;
        self.write_data(0xc6)?;

        self.write_command(0xbe)?;
        self.write_data(0x00)?;
        self.write_data(0x04)?;

        self.write_command(0xe9)?;
        self.write_data(0x00)?;

        self.write_command(0x36)?;
        self.write_data(0x8 | (1 << 5) | (1 << 6))?;

        self.write_command(0x3a)?;
        self.write_data(0x55)?;

        self.write_command(0xe0)?;
        self.write_data(0x00)?;
        self.write_data(0x07)?;
        self.write_data(0x10)?;
        self.write_data(0x09)?;
        self.write_data(0x17)?;
        self.write_data(0x0b)?;
        self.write_data(0x41)?;
        self.write_data(0x89)?;
        self.write_data(0x4b)?;
        self.write_data(0x0a)?;
        self.write_data(0x0c)?;
        self.write_data(0x0e)?;
        self.write_data(0x18)?;
        self.write_data(0x1b)?;
        self.write_data(0x0f)?;

        self.write_command(0xe1)?;
        self.write_data(0x00)?;
        self.write_data(0x17)?;
        self.write_data(0x1a)?;
        self.write_data(0x04)?;
        self.write_data(0x0e)?;
        self.write_data(0x06)?;
        self.write_data(0x2f)?;
        self.write_data(0x45)?;
        self.write_data(0x43)?;
        self.write_data(0x02)?;
        self.write_data(0x0a)?;
        self.write_data(0x09)?;
        self.write_data(0x32)?;
        self.write_data(0x36)?;
        self.write_data(0x0f)?;

        self.write_command(0x11)?;
        delay_source.delay_ms(60);
        self.write_command(0x29)?;

        Ok(())
    }

    pub fn hard_reset(&mut self, delay_source: &mut Delay) {
        if let Some(rst) = self.rst.as_mut() {
            rst.set_high().unwrap();
            delay_source.delay_ms(10);
            rst.set_low().unwrap();
            delay_source.delay_ms(10);
            rst.set_high().unwrap();
            delay_source.delay_ms(10);
        }
    }

    pub fn set_addr_win(&mut self, xs: u16, ys: u16, xe: u16, ye: u16) -> Result {
        self.write_command(0x2A)?;
        self.write_data((xs >> 8) as u8)?;
        self.write_data((xs & 0xFF) as u8)?;
        self.write_data((xe >> 8) as u8)?;
        self.write_data((xe & 0xFF) as u8)?;

        self.write_command(0x2B)?;
        self.write_data((ys >> 8) as u8)?;
        self.write_data((ys & 0xFF) as u8)?;
        self.write_data((ye >> 8) as u8)?;
        self.write_data((ye & 0xFF) as u8)?;

        // self.write_command(0x2C)?;
        Ok(())
    }

    pub fn clear(&mut self, color: Rgb565) -> Result {
        let mut buf = [color.into_storage()];
        let slice = buf.as_mut();
        let size = (self.size_x as u32) * (self.size_y as u32);
        self.set_addr_win(0, 0, self.size_x, self.size_y)?;

        self.write_command(0x2C)?;
        for _ in 0..size {
            self.write_data16(slice)?;
            // self.write_data16(slice)?;
        }
        Ok(())
    }
    // pub fn clear<I>(&mut self, color: I) -> Result
    // where
    //     I: IntoIterator<Item = Rgb565>,
    // {
    //     self.set_addr_win(0, 0, self.size_x - 1, self.size_y - 1)?;
    //     let size = (self.size_x as u32) * (self.size_y as u32);

    //     let colors = core::iter::repeat(color).take(size.try_into().unwrap());
    //     let mut iter = colors.into_iter().map(|c| c.into_storage());
    //     let buf = DataFormat::U16LEIter(&mut iter);
    //     self.di.send_data(buf)?;
    //     Ok(())
    // }

    pub fn write_pixels<I>(&mut self, colors: I) -> Result
    where
        I: IntoIterator<Item = Rgb565>,
    {
        let mut iter = colors.into_iter().map(|c| c.into_storage());
        let buf = DataFormat::U16LEIter(&mut iter);
        self.di.send_data(buf)?;
        Ok(())
    }

    pub fn write_command(&mut self, cmd: u8) -> Result {
        self.di.send_commands(DataFormat::U8(&[cmd]))?;
        Ok(())
    }

    pub fn write_data(&mut self, buf: u8) -> Result {
        self.di.send_data(DataFormat::U8(&[buf]))?;
        Ok(())
    }

    pub fn write_data16(&mut self, buf: &mut [u16]) -> Result {
        self.di.send_data(DataFormat::U16LE(buf))?;
        Ok(())
    }
}
