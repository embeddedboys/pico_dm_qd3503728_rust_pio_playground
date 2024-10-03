#![no_std]
#![no_main]

use defmt::info;
use embedded_hal::digital::OutputPin;
use cortex_m::delay::Delay;
use rp2040_hal::pio::{Tx, ValidStateMachine};

pub struct Pio16BitBus<SM: ValidStateMachine, DC> {
    tx: Tx<SM>,
    dc: DC,
}

pub trait WriteOnlyDataCommand {
    fn send_commands(&mut self, cmd: u16);
    fn send_data(&mut self, buf: u16);
}

impl<TX, DC> Pio16BitBus<TX, DC>
where
    TX: ValidStateMachine,
    DC: OutputPin,
{
    pub fn new(tx: Tx<TX>, dc: DC) -> Self {
        Self { tx, dc }
    }

    pub fn send_val(&mut self, val: u16) {
        self.tx.write(val as u32);
    }
}

impl<TX, DC> WriteOnlyDataCommand for Pio16BitBus<TX, DC>
where
    TX: ValidStateMachine,
    DC: OutputPin,
{
    fn send_commands(&mut self, cmd: u16) {
        self.dc.set_low().unwrap();
        self.send_val(cmd);
    }
    fn send_data(&mut self, buf: u16) {
        self.dc.set_high().unwrap();
        self.send_val(buf);
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

    pub fn init_test(&mut self) {
        self.write_command(0x55);
    }

    pub fn init(&mut self, mut delay_source: Delay ) {
        self.hard_reset(&mut delay_source);

        if let Some(bl) = self.bl.as_mut() {
            // bl.set_low().unwrap();
            // delay_source.delay_ms(10);
            bl.set_high().unwrap();
        }

        self.write_command(0xf7);
        self.write_data(0xa9);
        self.write_data(0x51);
        self.write_data(0x2c);
        self.write_data(0x82);

        self.write_command(0xc0);
        self.write_data(0x11);
        self.write_data(0x09);

        self.write_command(0xc1);
        self.write_data(0x41);

        self.write_command(0xc5);
        self.write_data(0x00);
        self.write_data(0x28);
        self.write_data(0x80);

        self.write_command(0xb1);
        self.write_data(0xb0);
        self.write_data(0x11);

        self.write_command(0xb4);
        self.write_data(0x02);

        self.write_command(0xb6);
        self.write_data(0x02);
        self.write_data(0x22);

        self.write_command(0xb7);
        self.write_data(0xc6);

        self.write_command(0xbe);
        self.write_data(0x00);
        self.write_data(0x04);

        self.write_command(0xe9);
        self.write_data(0x00);

        self.write_command(0x36);
        self.write_data(0x8 | (1 << 5) | (1 << 6));
        // self.write_data(1 << 3);

        self.write_command(0x3a);
        self.write_data(0x55);

        self.write_command(0xe0);
        self.write_data(0x00);
        self.write_data(0x07);
        self.write_data(0x10);
        self.write_data(0x09);
        self.write_data(0x17);
        self.write_data(0x0b);
        self.write_data(0x41);
        self.write_data(0x89);
        self.write_data(0x4b);
        self.write_data(0x0a);
        self.write_data(0x0c);
        self.write_data(0x0e);
        self.write_data(0x18);
        self.write_data(0x1b);
        self.write_data(0x0f);

        self.write_command(0xe1);
        self.write_data(0x00);
        self.write_data(0x17);
        self.write_data(0x1a);
        self.write_data(0x04);
        self.write_data(0x0e);
        self.write_data(0x06);
        self.write_data(0x2f);
        self.write_data(0x45);
        self.write_data(0x43);
        self.write_data(0x02);
        self.write_data(0x0a);
        self.write_data(0x09);
        self.write_data(0x32);
        self.write_data(0x36);
        self.write_data(0x0f);

        self.write_command(0x11);
        delay_source.delay_ms(60);
        self.write_command(0x29);
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

    pub fn set_addr_win(&mut self, xs: u16, ys: u16, xe: u16, ye: u16) {
        self.write_command(0x2A);
        self.write_data(xs >> 8);
        self.write_data(xs);
        self.write_data(xe >> 8);
        self.write_data(xe);

        self.write_command(0x2B);
        self.write_data(ys >> 8);
        self.write_data(ys);
        self.write_data(ye >> 8);
        self.write_data(ye);

        self.write_command(0x2C);
    }

    pub fn clear(&mut self, color: u16) {
        self.set_addr_win(0, 0, self.size_x, self.size_y);

        // info!("Clearing screen ({}x{})", self.size_x, self.size_y);
        for _ in 0..((self.size_x as u32) * (self.size_y as u32)) {
            self.write_data(color);
        }
    }

    pub fn write_command(&mut self, cmd: u16) {
        self.di.send_commands(cmd);
    }

    pub fn write_data(&mut self, buf: u16) {
        self.di.send_data(buf);
    }
}