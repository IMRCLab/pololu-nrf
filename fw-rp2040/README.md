# Building and flashing

Pick a runner in `.cargo/config.toml`, then run `cargo run --release`.

## Debug probe (with defmt logs)

1. Set `runner = "probe-rs run --chip RP2040 --protocol swd"`.
2. Connect the probe to the 3pi+ (pin names on the underside of the PCB; you may need to solder pins):
   - Yellow → SWDIO
   - Orange → SWCLK
   - Black → GND
3. Power the robot and run `cargo run --release`. Logs appear in the terminal.

## USB bootloader (no logs)

1. Set `runner = "elf2uf2-rs -d"`.
2. Connect the robot via USB.
3. Hold **B**, press **Reset**, release **B**.
4. Run `cargo run --release`.

# USB drive (work in progress)

When connected over USB, the robot shows up as a USB drive (`0x1209:0x0001`,
"3pi+ 2040 drive"). The drive is the 14 MiB of flash after the firmware (from 2 MiB on, see
`memory.x`), so its files survive resets and firmware updates. On first boot, if the drive has
no boot sector or partition table yet, the firmware formats it as FAT with a `README.TXT`.
Flashing MicroPython (or anything else larger than 2 MiB) overwrites the drive.

Writes are cached per 4 KiB flash sector and reach flash 200 ms after the host stops writing,
or on eject. Eject the drive before unplugging.

The mass storage class is a trimmed and patched copy of `deluge-usb-classes` in
`vendor/deluge-usb-classes` (see `PATCHES.md` there).

Test on Linux (replace `sdX`; `lsblk` lists the devices):

```bash
head -c 3000000 /dev/urandom > test.bin && cp test.bin "/run/media/$USER/3PI DRIVE/" && sync
```

Eject the drive, reset the robot (or replug without batteries), then:

```bash
cmp test.bin "/run/media/$USER/3PI DRIVE/test.bin" && echo same
```

To format from the host, unmount without ejecting and close anything that has the drive open:

```bash
sudo umount /dev/sdX && sudo mkfs.fat -n TEST /dev/sdX
```

On Windows and macOS, check that the drive mounts, that formatting it works, that ejecting it
works, and that a copied file is still the same after resetting the robot.
