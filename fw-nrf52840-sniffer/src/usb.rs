//! USB serial (CDC-ACM) port

use embassy_executor::Spawner;
use embassy_nrf::usb::Driver;
use embassy_nrf::usb::vbus_detect::HardwareVbusDetect;
use embassy_nrf::{Peri, bind_interrupts, peripherals};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::driver::EndpointError;
use embassy_usb::{Builder, UsbDevice};
use static_cell::StaticCell;
use embassy_time::Timer;

bind_interrupts!(struct Irqs {
    USBD => embassy_nrf::usb::InterruptHandler<peripherals::USBD>;
    CLOCK_POWER => embassy_nrf::usb::vbus_detect::InterruptHandler;
});

type UsbDriver = Driver<'static, HardwareVbusDetect>;

/// Sets up the USB device and spawns its tasks.
pub fn init(spawner: Spawner, usbd: Peri<'static, peripherals::USBD>) {
    let driver = Driver::new(usbd, Irqs, HardwareVbusDetect::new(Irqs));

    // 0x1209:0x0001 is the pid.codes test ID: fine for development, not for release.
    let mut config = embassy_usb::Config::new(0x1209, 0x0001);
    config.manufacturer = Some("IMRCLab");
    config.product = Some("nRF52840 sniffer");
    config.serial_number = Some("0001");
    config.max_power = 100; // current draw in mA

    static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
    static CDC_STATE: StaticCell<State> = StaticCell::new();

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESC.init([0; 256]),
        BOS_DESC.init([0; 256]),
        &mut [], // no Microsoft OS descriptors
        CONTROL_BUF.init([0; 64]),
    );

    // One CDC-ACM (USB serial) port, shows up as /dev/ttyACM* on Linux
    let class = CdcAcmClass::new(&mut builder, CDC_STATE.init(State::new()), 64);

    spawner.spawn(usb_task(builder.build()).unwrap());
    spawner.spawn(echo_task(class).unwrap());
}

/// Runs the USB stack (enumeration, control requests). Must run forever.
#[embassy_executor::task]
async fn usb_task(mut device: UsbDevice<'static, UsbDriver>) -> ! {
    device.run().await
}

#[embassy_executor::task]
async fn echo_task(mut class: CdcAcmClass<'static, UsbDriver>) {
    loop {
        class.wait_connection().await;
        let _ = hello(&mut class).await; // returns when the host disconnects
    }
}

async fn echo(class: &mut CdcAcmClass<'static, UsbDriver>) -> Result<(), EndpointError> {
    let mut buf = [0; 64];
    loop {
        let n = class.read_packet(&mut buf).await?;
        class.write_packet(&buf[..n]).await?;
    }
}

async fn hello(class: &mut CdcAcmClass<'static, UsbDriver>) -> Result<(), EndpointError> {
    loop {
        class.write_packet(b"Hello from blue!\r\n").await?;
        Timer::after_millis(1000).await;
    }
}
