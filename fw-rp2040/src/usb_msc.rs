//! USB drive (mass storage) backed by the flash drive
//!
//! USB starts at boot, but the drive is only offered to the host once
//! [`enable_drive`] is called (drive mode). Until then, and for good in robot
//! mode, the host sees the drive without a medium, so the host and the
//! firmware never use the filesystem at the same time.

use core::sync::atomic::{AtomicBool, Ordering};

use embassy_executor::Spawner;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::{Peri, bind_interrupts};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, with_timeout};
use embassy_usb::{Builder, Handler, UsbDevice};
use static_cell::{ConstStaticCell, StaticCell};
use usb_classes::bot::{self, BlockDevice, BotBuffer, Inquiry};
use usb_classes::msc::MscClass;

use crate::flash_disk::{DISK_SIZE, FlashDisk, SharedDisk};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

type UsbDriver = Driver<'static, USB>;
type EndpointIn = <UsbDriver as embassy_usb::driver::Driver<'static>>::EndpointIn;
type EndpointOut = <UsbDriver as embassy_usb::driver::Driver<'static>>::EndpointOut;

/// Largest USB full-speed bulk packet
const MAX_PACKET: u16 = 64;

/// SCSI block size used by the mass storage class
const BLOCK_SIZE: u32 = bot::BLOCK_SIZE;

/// Without writes for this long, cached data goes to flash. Hosts don't ask
/// for that themselves (no SYNCHRONIZE CACHE for a write-through device), so
/// this bounds what is lost when the cable is pulled without ejecting.
const IDLE_FLUSH: Duration = Duration::from_millis(200);

/// Signaled after every write from the host
static WRITTEN: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Signaled when a host has configured the device
static CONFIGURED: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// The host may use the drive
static DRIVE_ENABLED: AtomicBool = AtomicBool::new(false);

/// Sets up the USB device and spawns its tasks. Returns the drive, shared
/// with USB.
pub fn init(spawner: Spawner, usb: Peri<'static, USB>, disk: FlashDisk) -> &'static SharedDisk {
    static DISK: StaticCell<SharedDisk> = StaticCell::new();
    let disk = DISK.init(Mutex::new(disk));

    let driver = Driver::new(usb, Irqs);

    // 0x1209:0x0001 is the pid.codes test ID
    let mut config = embassy_usb::Config::new(0x1209, 0x0001);
    config.manufacturer = Some("IMRCLab");
    config.product = Some("3pi+ 2040 drive");
    config.serial_number = Some("0001");
    config.max_power = 100; // current draw in mA
    // A single interface: no interface association descriptor needed
    config.composite_with_iads = false;
    config.device_class = 0x00;
    config.device_sub_class = 0x00;
    config.device_protocol = 0x00;

    static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
    static MSC: StaticCell<MscClass> = StaticCell::new();
    static HOST_DETECT: StaticCell<HostDetect> = StaticCell::new();
    static BOT_BUF: ConstStaticCell<BotBuffer> = ConstStaticCell::new(BotBuffer::new());

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESC.init([0; 256]),
        BOS_DESC.init([0; 256]),
        &mut [], // no Microsoft OS descriptors
        CONTROL_BUF.init([0; 64]),
    );

    let (msc, ep_out, ep_in) = MscClass::new(&mut builder, MAX_PACKET);
    builder.handler(MSC.init(msc));
    builder.handler(HOST_DETECT.init(HostDetect));

    spawner.spawn(usb_task(builder.build()).unwrap());
    spawner.spawn(msc_task(UsbDisk(disk), ep_in, ep_out, BOT_BUF.take()).unwrap());
    spawner.spawn(idle_flush_task(disk).unwrap());
    disk
}

/// Waits up to `timeout` for a USB host to configure the device. Returns
/// whether one did.
pub async fn host_connected(timeout: Duration) -> bool {
    with_timeout(timeout, CONFIGURED.wait()).await.is_ok()
}

/// Offers the drive to the host (drive mode). The firmware must not use the
/// drive anymore.
pub fn enable_drive() {
    DRIVE_ENABLED.store(true, Ordering::Release);
}

fn drive_enabled() -> bool {
    DRIVE_ENABLED.load(Ordering::Acquire)
}

/// Reports when the host has configured the device
struct HostDetect;

impl Handler for HostDetect {
    fn configured(&mut self, configured: bool) {
        if configured {
            CONFIGURED.signal(());
        }
    }
}

/// Runs the USB stack (enumeration, control requests). Must run forever.
#[embassy_executor::task]
async fn usb_task(mut device: UsbDevice<'static, UsbDriver>) -> ! {
    device.run().await
}

/// Answers the host's SCSI commands. Must run forever.
#[embassy_executor::task]
async fn msc_task(
    disk: UsbDisk,
    ep_in: EndpointIn,
    ep_out: EndpointOut,
    buf: &'static mut BotBuffer,
) -> ! {
    bot::run(disk, ep_in, ep_out, buf).await
}

/// Writes cached data to flash once the host stops writing.
#[embassy_executor::task]
async fn idle_flush_task(disk: &'static SharedDisk) -> ! {
    loop {
        WRITTEN.wait().await;
        while with_timeout(IDLE_FLUSH, WRITTEN.wait()).await.is_ok() {}
        // A failed flush keeps the data cached: the next write or flush retries
        let _ = disk.lock().await.flush();
    }
}

/// The flash drive as seen by USB: 512-byte blocks
struct UsbDisk(&'static SharedDisk);

impl BlockDevice for UsbDisk {
    fn block_count(&self) -> u32 {
        DISK_SIZE / BLOCK_SIZE
    }

    fn is_ready(&self) -> bool {
        drive_enabled()
    }

    async fn ensure_ready(&mut self) -> bool {
        drive_enabled()
    }

    async fn read(&mut self, lba: u32, count: u32, buf: &mut [u8]) -> Result<(), ()> {
        if !drive_enabled() {
            return Err(());
        }
        let len = (count * BLOCK_SIZE) as usize;
        let buf = buf.get_mut(..len).ok_or(())?;
        let offset = lba.checked_mul(BLOCK_SIZE).ok_or(())?;
        self.0.lock().await.read(offset, buf).map_err(|_| ())
    }

    async fn write(&mut self, lba: u32, count: u32, buf: &[u8]) -> Result<(), ()> {
        if !drive_enabled() {
            return Err(());
        }
        let len = (count * BLOCK_SIZE) as usize;
        let buf = buf.get(..len).ok_or(())?;
        let offset = lba.checked_mul(BLOCK_SIZE).ok_or(())?;
        let result = self.0.lock().await.write(offset, buf);
        WRITTEN.signal(());
        result.map_err(|_| ())
    }

    async fn flush(&mut self) -> Result<(), ()> {
        if !drive_enabled() {
            return Ok(());
        }
        self.0.lock().await.flush().map_err(|_| ())
    }

    fn inquiry(&self) -> Inquiry {
        Inquiry {
            vendor: *b"IMRCLab ",
            product: *b"3pi+ 2040 drive ",
            revision: *b"0.1 ",
        }
    }
}
