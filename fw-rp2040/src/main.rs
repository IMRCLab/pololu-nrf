#![no_std]
#![no_main]

mod display;

use embassy_rp::bind_interrupts;
use embassy_rp::dma;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::DMA_CH0;
use embassy_time::{Duration, Timer};
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};

use panic_halt as _;

use display::Display;

bind_interrupts!(struct Irqs {
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>;
});

#[embassy_executor::task]
async fn led_blink_task(mut led: Output<'static>) {
    loop {
        led.toggle();
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let p = embassy_rp::init(Default::default());
    // Yellow LED, active low.
    let led = Output::new(p.PIN_25, Level::High);
    spawner.spawn(led_blink_task(led).unwrap());

    let mut display = Display::new(p.SPI0, p.PIN_2, p.PIN_3, p.PIN_0, p.PIN_1, p.DMA_CH0, Irqs)
        .await
        .unwrap();

    let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
    Text::with_baseline("Hello World!", Point::new(0, 0), style, Baseline::Top)
        .draw(&mut *display)
        .unwrap();
    Text::with_baseline("Pololu 3pi+ 2040", Point::new(0, 16), style, Baseline::Top)
        .draw(&mut *display)
        .unwrap();
    display.flush().await.unwrap();

    // Never return: dropping `display` would release its pins and blank the screen.
    core::future::pending::<()>().await;
}
