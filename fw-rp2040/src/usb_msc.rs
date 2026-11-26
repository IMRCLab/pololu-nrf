//! USB drive (mass storage): proof of concept backed by a RAM disk
//!
//! The disk is formatted as FAT at boot and gets a `README.TXT`, so it mounts
//! right away. Its contents are lost on reset.

use embassy_executor::Spawner;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::{Peri, bind_interrupts};
use embassy_usb::{Builder, UsbDevice};
use embedded_fatfs::{FileSystem, FormatVolumeOptions, FsOptions, format_volume};
use embedded_io_async::{ErrorKind, ErrorType, Read, Seek, SeekFrom, Write};
use static_cell::{ConstStaticCell, StaticCell};
use usb_classes::bot::{self, BlockDevice, BotBuffer, Inquiry};
use usb_classes::msc::MscClass;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

type UsbDriver = Driver<'static, USB>;
type EndpointIn = <UsbDriver as embassy_usb::driver::Driver<'static>>::EndpointIn;
type EndpointOut = <UsbDriver as embassy_usb::driver::Driver<'static>>::EndpointOut;

/// Largest USB full-speed bulk packet
const MAX_PACKET: u16 = 64;

/// SCSI block size used by the mass storage class
const BLOCK_SIZE: usize = bot::BLOCK_SIZE as usize;

/// RAM disk size: 256 blocks
const DISK_SIZE: usize = 128 * 1024;

const README: &[u8] = b"3pi+ 2040 RAM disk (USB drive proof of concept).\r\n\
Files written here are lost when the robot is reset.\r\n";

/// Formats the RAM disk, sets up the USB device and spawns its tasks.
pub async fn init(spawner: Spawner, usb: Peri<'static, USB>) {
    static DISK: ConstStaticCell<[u8; DISK_SIZE]> = ConstStaticCell::new([0; DISK_SIZE]);
    let mut disk = RamDisk {
        data: DISK.take(),
        pos: 0,
    };
    // Without a filesystem the drive still shows up, the host just offers to format it
    let _ = format(&mut disk).await;
    disk.pos = 0;

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

    spawner.spawn(usb_task(builder.build()).unwrap());
    spawner.spawn(msc_task(disk, ep_in, ep_out, BOT_BUF.take()).unwrap());
}

/// Formats `disk` as FAT and writes `README.TXT`
async fn format(disk: &mut RamDisk) -> Result<(), embedded_fatfs::Error<ErrorKind>> {
    let options = FormatVolumeOptions::new().volume_label(*b"3PI DRIVE  ");
    format_volume(disk, options).await?;
    disk.pos = 0;

    let fs = FileSystem::new(&mut *disk, FsOptions::new()).await?;
    let mut file = fs.root_dir().create_file("README.TXT").await?;
    file.write_all(README).await?;
    file.flush().await?;
    drop(file);
    fs.unmount().await
}

/// Runs the USB stack (enumeration, control requests). Must run forever.
#[embassy_executor::task]
async fn usb_task(mut device: UsbDevice<'static, UsbDriver>) -> ! {
    device.run().await
}

/// Answers the host's SCSI commands. Must run forever.
#[embassy_executor::task]
async fn msc_task(
    disk: RamDisk,
    ep_in: EndpointIn,
    ep_out: EndpointOut,
    buf: &'static mut BotBuffer,
) -> ! {
    bot::run(disk, ep_in, ep_out, buf).await
}

/// Disk in RAM, seen as 512-byte blocks by USB and as a byte stream by the
/// FAT library
struct RamDisk {
    data: &'static mut [u8; DISK_SIZE],
    /// Byte position for the FAT library's `Read`/`Write`/`Seek`
    pos: u64,
}

impl RamDisk {
    /// Byte range of `count` blocks starting at `lba`, if it is on the disk
    fn blocks(lba: u32, count: u32) -> Option<core::ops::Range<usize>> {
        let start = (lba as usize).checked_mul(BLOCK_SIZE)?;
        let end = start.checked_add((count as usize).checked_mul(BLOCK_SIZE)?)?;
        (end <= DISK_SIZE).then_some(start..end)
    }
}

impl BlockDevice for RamDisk {
    fn block_count(&self) -> u32 {
        (DISK_SIZE / BLOCK_SIZE) as u32
    }

    fn is_ready(&self) -> bool {
        true
    }

    async fn ensure_ready(&mut self) -> bool {
        true
    }

    async fn read(&mut self, lba: u32, count: u32, buf: &mut [u8]) -> Result<(), ()> {
        let range = Self::blocks(lba, count).ok_or(())?;
        buf.get_mut(..range.len())
            .ok_or(())?
            .copy_from_slice(&self.data[range]);
        Ok(())
    }

    async fn write(&mut self, lba: u32, count: u32, buf: &[u8]) -> Result<(), ()> {
        let range = Self::blocks(lba, count).ok_or(())?;
        let len = range.len();
        self.data[range].copy_from_slice(buf.get(..len).ok_or(())?);
        Ok(())
    }

    fn inquiry(&self) -> Inquiry {
        Inquiry {
            vendor: *b"IMRCLab ",
            product: *b"3pi+ 2040 drive ",
            revision: *b"0.1 ",
        }
    }
}

impl ErrorType for RamDisk {
    type Error = ErrorKind;
}

impl Read for RamDisk {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let start = (self.pos as usize).min(DISK_SIZE);
        let n = buf.len().min(DISK_SIZE - start);
        buf[..n].copy_from_slice(&self.data[start..start + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl Write for RamDisk {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let start = (self.pos as usize).min(DISK_SIZE);
        let n = buf.len().min(DISK_SIZE - start);
        if n == 0 && !buf.is_empty() {
            return Err(ErrorKind::WriteZero);
        }
        self.data[start..start + n].copy_from_slice(&buf[..n]);
        self.pos += n as u64;
        Ok(n)
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Seek for RamDisk {
    async fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        let new = match pos {
            SeekFrom::Start(p) => Some(p),
            SeekFrom::End(d) => (DISK_SIZE as u64).checked_add_signed(d),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
        };
        self.pos = new.ok_or(ErrorKind::InvalidInput)?;
        Ok(self.pos)
    }
}
