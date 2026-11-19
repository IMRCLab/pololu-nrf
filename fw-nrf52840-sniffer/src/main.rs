#![no_std]
#![no_main]

use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_time::Timer;

use panic_halt as _;

#[embassy_executor::task]
async fn led_blink_task(mut led: Output<'static>) {
    loop {
        led.toggle();
        Timer::after(embassy_time::Duration::from_millis(500)).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let p = embassy_nrf::init(Default::default());
    let led = Output::new(p.P0_06, Level::High, OutputDrive::Standard);

    spawner.spawn(led_blink_task(led).unwrap());
}
