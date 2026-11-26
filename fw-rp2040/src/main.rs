#![no_std]
#![no_main]

use embassy_rp::flash::Flash;
use embassy_rp::gpio::{Level, Output};
use embassy_time::Timer;

use panic_halt as _;

mod flash_disk;
mod usb_msc;

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

    let mut disk = flash_disk::FlashDisk::new(Flash::new_blocking(p.FLASH));
    // Without a filesystem the drive still shows up, the host just offers to format it
    let _ = flash_disk::format_if_unformatted(&mut disk).await;
    usb_msc::init(spawner, p.USB, disk);
}
