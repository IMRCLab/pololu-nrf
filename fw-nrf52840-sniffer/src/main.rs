#![no_std]
#![no_main]

#[cfg(not(any(feature = "rx", feature = "tx")))]
compile_error!("choose a role: build with `--features rx` or `--features tx`");
#[cfg(all(feature = "rx", feature = "tx"))]
compile_error!("features `rx` and `tx` are mutually exclusive");

mod frame;
mod radio;
mod usb;

use embassy_executor::Spawner;
use embassy_nrf::config::HfclkSource;
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_time::Timer;

use panic_halt as _;

#[embassy_executor::task]
async fn led_blink_task(mut led: Output<'static>) {
    loop {
        led.toggle();
        Timer::after_millis(500).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = embassy_nrf::config::Config::default();

    // USB and the radio need the external 32 MHz crystal
    config.hfclk_source = HfclkSource::ExternalXtal;

    let p = embassy_nrf::init(config);

    #[cfg(feature = "tx")]
    let led = Output::new(p.P0_12, Level::High, OutputDrive::Standard); // blue led

    #[cfg(feature = "rx")]
    let led = Output::new(p.P0_06, Level::High, OutputDrive::Standard); // green led
    spawner.spawn(led_blink_task(led).unwrap());

    usb::init(spawner, p.USBD);
    radio::init(spawner, p.RADIO);
}
