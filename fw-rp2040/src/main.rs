#![no_std]
#![no_main]

use embassy_rp::flash::Flash;
use embassy_rp::gpio::{Level, Output};
use embassy_time::{Duration, Timer};

use panic_halt as _;

use flash_disk::SharedDisk;

mod flash_disk;
mod usb_msc;

/// How long to wait at boot for a USB host. A host configures the device well
/// within this (typically a few hundred ms); without one, the robot starts
/// after this delay.
const HOST_TIMEOUT: Duration = Duration::from_secs(1);

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut led = Output::new(p.PIN_25, Level::Low);

    let mut disk = flash_disk::FlashDisk::new(Flash::new_blocking(p.FLASH));
    // Without a filesystem the drive still shows up, the host just offers to format it
    let _ = flash_disk::format_if_unformatted(&mut disk).await;
    let disk = usb_msc::init(spawner, p.USB, disk);

    // The mode is chosen once at boot. Changing it takes a reset.
    if usb_msc::host_connected(HOST_TIMEOUT).await {
        // Drive mode: the host owns the drive, the robot does nothing else
        usb_msc::enable_drive();
        led.set_high();
        // Keep `led` (dropping it releases the pin, turning the LED off)
        core::future::pending::<()>().await;
    } else {
        robot_mode(led, disk).await;
    }
}

/// Robot mode: the firmware owns the drive (configuration, logs). The host
/// sees the drive without a medium if the cable is plugged in later.
async fn robot_mode(mut led: Output<'static>, _disk: &'static SharedDisk) -> ! {
    loop {
        led.toggle();
        Timer::after(Duration::from_millis(500)).await;
    }
}
