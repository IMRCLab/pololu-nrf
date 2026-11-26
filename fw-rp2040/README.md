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

When connected over USB, the robot shows up as a small USB drive
(`0x1209:0x0001`, "3pi+ 2040 drive"). For now it is a 128 KiB RAM disk, formatted at
boot with a `README.TXT`. Its contents are lost on reset.

The mass storage class is a trimmed and patched copy of `deluge-usb-classes` in
`vendor/deluge-usb-classes` (see `PATCHES.md` there).

Test on Linux:

```bash
lsblk                                  # find the device, e.g. /dev/sdX
sudo mount /dev/sdX /mnt && ls /mnt && cat /mnt/README.TXT
head -c 60000 /dev/urandom > test.bin && sudo cp test.bin /mnt/ && sync
sudo umount /mnt && sudo mount /dev/sdX /mnt && cmp test.bin /mnt/test.bin
sudo umount /mnt && sudo eject /dev/sdX
```

On Windows and macOS, check that the drive mounts, that formatting it works, that ejecting
it works, and that a file copied to it reads back the same after ejecting and replugging.
Replug with the robot switched on (batteries in), otherwise unplugging resets it and the
RAM disk is lost.
