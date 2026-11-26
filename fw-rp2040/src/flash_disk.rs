//! Drive (FAT volume) in the flash after the firmware
//!
//! Flash is erased in 4 KiB sectors, but USB and the FAT library write smaller
//! pieces. Writes therefore go to a one-sector cache that is written to flash
//! when another sector is touched or on `flush()`.

use embassy_rp::flash::{self, Blocking, ERASE_SIZE, PAGE_SIZE};
use embassy_rp::peripherals::FLASH;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embedded_fatfs::{FileSystem, FormatVolumeOptions, FsOptions, format_volume};
use embedded_io_async::{ErrorKind, ErrorType, Read, Seek, SeekFrom, Write};

/// Flash chip size of the 3pi+ 2040
const FLASH_SIZE: usize = 16 * 1024 * 1024;

/// Start of the drive in flash. The firmware must end before it (FLASH length
/// in memory.x).
const DISK_OFFSET: u32 = 2 * 1024 * 1024;

/// Drive size: the rest of the flash (14 MiB)
pub const DISK_SIZE: u32 = FLASH_SIZE as u32 - DISK_OFFSET;

const SECTOR_SIZE: usize = ERASE_SIZE;

const _: () = assert!((DISK_OFFSET as usize).is_multiple_of(SECTOR_SIZE));

/// Volume label of a drive formatted by the firmware (11 characters)
const VOLUME_LABEL: [u8; 11] = *b"3PI DRIVE  ";

const README: &[u8] = b"3pi+ 2040 drive.\r\n\
Log files from the robot are stored here, and its configuration can be edited here.\r\n";

pub type Flash = flash::Flash<'static, FLASH, Blocking, FLASH_SIZE>;

/// The drive, shared between USB and the firmware
pub type SharedDisk = Mutex<CriticalSectionRawMutex, FlashDisk>;

pub struct FlashDisk {
    flash: Flash,
    /// Copy of sector `cached`, possibly with changes not yet in flash
    cache: [u8; SECTOR_SIZE],
    /// Index of the cached sector, counted from the start of the drive
    cached: Option<u32>,
    /// The cache has changes not yet in flash
    dirty: bool,
}

impl FlashDisk {
    pub fn new(flash: Flash) -> Self {
        Self {
            flash,
            cache: [0; SECTOR_SIZE],
            cached: None,
            dirty: false,
        }
    }

    /// Reads `buf.len()` bytes from drive offset `offset`
    pub fn read(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), flash::Error> {
        check_range(offset, buf.len())?;
        let mut done = 0;
        while done < buf.len() {
            let pos = offset + done as u32;
            let (sector, start, len) = split(pos, buf.len() - done);
            let part = &mut buf[done..done + len];
            if self.cached == Some(sector) {
                part.copy_from_slice(&self.cache[start..start + len]);
            } else {
                self.flash.blocking_read(DISK_OFFSET + pos, part)?;
            }
            done += len;
        }
        Ok(())
    }

    /// Writes `data` to drive offset `offset` (into the cache)
    pub fn write(&mut self, offset: u32, data: &[u8]) -> Result<(), flash::Error> {
        check_range(offset, data.len())?;
        let mut done = 0;
        while done < data.len() {
            let (sector, start, len) = split(offset + done as u32, data.len() - done);
            if self.cached != Some(sector) {
                self.flush()?;
                // A partly written sector keeps the rest of its contents
                if len < SECTOR_SIZE {
                    let base = DISK_OFFSET + sector * SECTOR_SIZE as u32;
                    self.flash.blocking_read(base, &mut self.cache)?;
                }
                self.cached = Some(sector);
            }
            self.cache[start..start + len].copy_from_slice(&data[done..done + len]);
            self.dirty = true;
            done += len;
        }
        Ok(())
    }

    /// Writes the cached sector to flash, if it has changes
    pub fn flush(&mut self) -> Result<(), flash::Error> {
        let Some(sector) = self.cached.filter(|_| self.dirty) else {
            return Ok(());
        };
        let base = DISK_OFFSET + sector * SECTOR_SIZE as u32;
        // Hosts often rewrite unchanged data: save the erase (time and wear)
        if !self.flash_matches_cache(base)? {
            self.flash.blocking_erase(base, base + SECTOR_SIZE as u32)?;
            self.flash.blocking_write(base, &self.cache)?;
        }
        self.dirty = false;
        Ok(())
    }

    fn flash_matches_cache(&mut self, base: u32) -> Result<bool, flash::Error> {
        let mut page = [0; PAGE_SIZE];
        for (i, cached) in self.cache.as_chunks::<PAGE_SIZE>().0.iter().enumerate() {
            self.flash
                .blocking_read(base + (i * PAGE_SIZE) as u32, &mut page)?;
            if page != *cached {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn check_range(offset: u32, len: usize) -> Result<(), flash::Error> {
    match offset.checked_add(len as u32) {
        Some(end) if end <= DISK_SIZE => Ok(()),
        _ => Err(flash::Error::OutOfBounds),
    }
}

/// For drive offset `pos` and up to `len` bytes: (sector, offset in sector,
/// bytes until the end of that sector or of `len`)
fn split(pos: u32, len: usize) -> (u32, usize, usize) {
    let sector = pos / SECTOR_SIZE as u32;
    let start = pos as usize % SECTOR_SIZE;
    (sector, start, len.min(SECTOR_SIZE - start))
}

/// Formats the drive as FAT with a `README.TXT`, unless sector 0 already holds a
/// boot sector or partition table (0x55AA signature): those are kept, also if
/// the host formatted the drive differently.
pub async fn format_if_unformatted(
    disk: &mut FlashDisk,
) -> Result<(), embedded_fatfs::Error<ErrorKind>> {
    let mut signature = [0; 2];
    disk.read(510, &mut signature)
        .map_err(|_| embedded_fatfs::Error::Io(ErrorKind::Other))?;
    if signature == [0x55, 0xAA] {
        return Ok(());
    }

    let mut stream = DiskStream::new(disk);
    let options = FormatVolumeOptions::new().volume_label(VOLUME_LABEL);
    format_volume(&mut stream, options).await?;
    stream.seek(SeekFrom::Start(0)).await?;

    let fs = FileSystem::new(&mut stream, FsOptions::new()).await?;
    let mut file = fs.root_dir().create_file("README.TXT").await?;
    file.write_all(README).await?;
    file.flush().await?;
    drop(file);
    fs.unmount().await?;
    stream.flush().await?;
    Ok(())
}

/// Byte stream over the drive, for the FAT library
pub struct DiskStream<'a> {
    disk: &'a mut FlashDisk,
    pos: u64,
}

impl<'a> DiskStream<'a> {
    pub fn new(disk: &'a mut FlashDisk) -> Self {
        Self { disk, pos: 0 }
    }

    /// Bytes from the current position to the end of the drive, at most `len`
    fn available(&self, len: usize) -> usize {
        len.min((DISK_SIZE as u64).saturating_sub(self.pos) as usize)
    }
}

impl ErrorType for DiskStream<'_> {
    type Error = ErrorKind;
}

impl Read for DiskStream<'_> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let n = self.available(buf.len());
        self.disk
            .read(self.pos as u32, &mut buf[..n])
            .map_err(|_| ErrorKind::Other)?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Write for DiskStream<'_> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let n = self.available(buf.len());
        if n == 0 && !buf.is_empty() {
            return Err(ErrorKind::WriteZero);
        }
        self.disk
            .write(self.pos as u32, &buf[..n])
            .map_err(|_| ErrorKind::Other)?;
        self.pos += n as u64;
        Ok(n)
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.disk.flush().map_err(|_| ErrorKind::Other)
    }
}

impl Seek for DiskStream<'_> {
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
