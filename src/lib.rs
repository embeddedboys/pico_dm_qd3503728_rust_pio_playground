#![no_std]
#![no_main]

use cortex_m::delay::Delay;
use defmt::info;
use display_interface::{DataFormat, DisplayError, WriteOnlyDataCommand};
use embedded_graphics::{pixelcolor::Rgb565, prelude::IntoStorage};
use embedded_hal::digital::OutputPin;
use rp2040_hal::pio::{Tx, ValidStateMachine};

mod graphics;
pub mod overclock;

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
            while !self.tx.is_empty() {}
        }
        Ok(())
    }

    fn write_iter16(&mut self, iter: impl Iterator<Item = u16>) -> Result {
        for value in iter {
            self.tx.write(value as u32);
            while !self.tx.is_empty() {}
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

        self.write_reg(&[0xf7, 0xa9, 0x51, 0x2c, 0x82])?;

        self.write_reg(&[0xc0, 0x11, 0x09])?;

        self.write_reg(&[0xc1, 0x41])?;

        self.write_reg(&[0xc5, 0x00, 0x28, 0x80])?;

        self.write_reg(&[0xb1, 0xb0, 0x11])?;

        self.write_reg(&[0xb4, 0x02])?;

        self.write_reg(&[0xb6, 0x02, 0x22])?;

        self.write_reg(&[0xb7, 0xc6])?;

        self.write_reg(&[0xbe, 0x00, 0x04])?;

        self.write_reg(&[0xe9, 0x00])?;

        self.write_reg(&[0x36, 0x8 | (1 << 5) | (1 << 6)])?;

        self.write_reg(&[0x3a, 0x55])?;

        self.write_reg(&[
            0xe0, 0x00, 0x07, 0x10, 0x09, 0x17, 0x0b, 0x41, 0x89, 0x4b, 0x0a, 0x0c, 0x0e, 0x18,
            0x1b, 0x0f,
        ])?;

        self.write_reg(&[
            0xe1, 0x00, 0x17, 0x1a, 0x04, 0x0e, 0x06, 0x2f, 0x45, 0x43, 0x02, 0x0a, 0x09, 0x32,
            0x36, 0x0f,
        ])?;

        self.write_reg(&[0x11])?;
        delay_source.delay_ms(60);
        self.write_reg(&[0x29])?;

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
        self.write_reg(&[
            0x2A,
            (xs >> 8) as u8,
            (xs & 0xFF) as u8,
            (xe >> 8) as u8,
            (xe & 0xFF) as u8,
        ])?;

        self.write_reg(&[
            0x2B,
            (ys >> 8) as u8,
            (ys & 0xFF) as u8,
            (ye >> 8) as u8,
            (ye & 0xFF) as u8,
        ])?;

        self.write_reg(&[0x2C])?;
        Ok(())
    }

    pub fn clear(&mut self, color: Rgb565) -> Result {
        let mut buf = [color.into_storage()];
        let slice = buf.as_mut();
        let size = (self.size_x as u32) * (self.size_y as u32);
        self.set_addr_win(0, 0, self.size_x - 1, self.size_y - 1)?;

        for _ in 0..size {
            self.write_data16(slice)?;
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

    pub fn write_reg(&mut self, seq: &[u8]) -> Result {
        self.write_command(seq[0])?;
        for val in &seq[1..] {
            self.write_data(*val)?;
        }
        Ok(())
    }

    pub fn write_data16(&mut self, buf: &mut [u16]) -> Result {
        self.di.send_data(DataFormat::U16LE(buf))?;
        Ok(())
    }
}
