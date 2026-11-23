//! USB serial (CDC-ACM) port

use embassy_executor::Spawner;
use embassy_nrf::usb::Driver;
use embassy_nrf::usb::vbus_detect::HardwareVbusDetect;
use embassy_nrf::{Peri, bind_interrupts, peripherals};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::driver::EndpointError;
use embassy_usb::{Builder, UsbDevice};
use static_cell::StaticCell;

use core::fmt::Write;
use embassy_nrf::radio::ieee802154::Packet;

#[cfg(feature = "rx")]
use crate::frame;
use crate::frame::Header;
use crate::radio::FRAMES;

bind_interrupts!(struct Irqs {
    USBD => embassy_nrf::usb::InterruptHandler<peripherals::USBD>;
    CLOCK_POWER => embassy_nrf::usb::vbus_detect::InterruptHandler;
});

type UsbDriver = Driver<'static, HardwareVbusDetect>;

/// Largest USB full-speed bulk packet
const MAX_PACKET: u16 = 64;

/// Sets up the USB device and spawns its tasks.
pub fn init(spawner: Spawner, usbd: Peri<'static, peripherals::USBD>) {
    let driver = Driver::new(usbd, Irqs, HardwareVbusDetect::new(Irqs));

    // 0x1209:0x0001 is the pid.codes test ID
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
    let class = CdcAcmClass::new(&mut builder, CDC_STATE.init(State::new()), MAX_PACKET);

    spawner.spawn(usb_task(builder.build()).unwrap());
    spawner.spawn(forward_task(class).unwrap());
}

/// Runs the USB stack (enumeration, control requests). Must run forever.
#[embassy_executor::task]
async fn usb_task(mut device: UsbDevice<'static, UsbDriver>) -> ! {
    device.run().await
}

#[embassy_executor::task]
async fn forward_task(mut class: CdcAcmClass<'static, UsbDriver>) {
    loop {
        class.wait_connection().await;
        let _ = forward(&mut class).await; // returns when the host disconnects
    }
}

/// Sends every frame from `FRAMES` as one text line. Our frames:
/// `lqi=187 seq=12 dst=0xffff src=0x0001 payload=...\r\n`
/// (`lqi=` only in `rx` builds: sent frames have no LQI). Foreign frames: one short notice.
async fn forward(class: &mut CdcAcmClass<'static, UsbDriver>) -> Result<(), EndpointError> {
    loop {
        let packet = FRAMES.receive().await;
        let mut line = Line::new();
        format_frame(&packet, &mut line);

        // A line can be longer than one USB packet: send it in chunks
        for chunk in line.as_bytes().chunks(MAX_PACKET as usize) {
            class.write_packet(chunk).await?;
        }
        // A full last packet means "more data follows": an empty packet ends the transfer
        if line.len.is_multiple_of(MAX_PACKET as usize) {
            class.write_packet(&[]).await?;
        }
    }
}

fn format_frame(packet: &Packet, line: &mut Line) {
    // Fits by construction (see `Line`), so writing can't fail
    match Header::parse(packet) {
        Some((header, payload)) => {
            #[cfg(feature = "rx")]
            let _ = write!(line, "lqi={} ", packet.lqi());
            let _ = write!(
                line,
                "seq={} dst=0x{:04x} src=0x{:04x} payload=",
                header.seq, header.dst, header.src
            );
            for byte in payload {
                let _ = write!(line, "{byte:02x}");
            }
        }
        #[cfg(feature = "rx")]
        None => match frame::pan_id(packet) {
            Some(pan) => {
                let _ = write!(line, "frame on PAN 0x{pan:04x} detected");
            }
            None => {
                let _ = line.write_str("frame without PAN detected");
            }
        },
        // tx only queues frames it built itself
        #[cfg(feature = "tx")]
        None => {}
    }
    let _ = line.write_str("\r\n");
}

/// Longest line: "lqi=255 seq=255 dst=0xffff src=0xffff payload=" (46)
/// + 116 payload bytes in hex (232) + "\r\n" (2) = 280.
const LINE_LEN: usize = 280;

/// Fixed-size text buffer
struct Line {
    buf: [u8; LINE_LEN],
    len: usize,
}

impl Line {
    fn new() -> Self {
        Self {
            buf: [0; LINE_LEN],
            len: 0,
        }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

impl Write for Line {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.len + s.len();
        let dst = self.buf.get_mut(self.len..end).ok_or(core::fmt::Error)?;
        dst.copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}
