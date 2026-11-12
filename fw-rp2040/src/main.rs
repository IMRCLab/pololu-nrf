#![no_std]
#![no_main] 

use embassy_rp::gpio::{Level, Output};
use embassy_time::Timer;

use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::task] 
async fn led_blink_task(mut led: Output<'static>) {
    loop {
        led.toggle();
        Timer::after(embassy_time::Duration::from_millis(500)).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let p = embassy_rp::init(Default::default());
    let led = Output::new(p.PIN_25, Level::Low);

    spawner.spawn(led_blink_task(led).unwrap());
}
