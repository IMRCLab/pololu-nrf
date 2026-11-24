# fw-nrf52840-sniffer

Rust firmware (embassy) for the **nRF52840 Dongle**.
Flashed over USB using the dongle's factory Nordic DFU bootloader, no debug probe needed.

## 1. One-time setup (Linux)

### Rust toolchain

Run

```bash
rustup target add thumbv7em-none-eabihf
rustup component add llvm-tools
cargo install cargo-binutils
```

### nrfutil

1. Download the `nrfutil` binary for Linux x86-64 from Nordic:
   https://www.nordicsemi.com/Products/Development-tools/nRF-Util
2. Put it on your `PATH` and make it executable:

   ```bash
   mv ~/Downloads/nrfutil ~/.local/bin/nrfutil
   chmod +x ~/.local/bin/nrfutil
   ```

3. Install the subcommands used for packaging and flashing:

   ```bash
   nrfutil install nrf5sdk-tools
   nrfutil install device
   ```

### Serial port access

The dongle's bootloader appears as `/dev/ttyACM*`, owned by the `dialout` group:

```bash
groups | grep -q dialout || sudo usermod -aG dialout $USER
```

If you were just added, log out and back in.

### Flash script

`cargo run` uses [`flash.sh`](flash.sh) to flash, so it must be executable:

```bash
chmod +x flash.sh
```

## 2. Build

```bash
cargo build --release
```

The ELF ends up in `target/thumbv7em-none-eabihf/release/fw-nrf52840-sniffer`.

## 3. Flash

1. Plug in the dongle.
2. Press the **RESET** button (the small sideways button on the edge opposite the
   USB connector; push it towards the connector). The red LED starts pulsing,
   which means the dongle is in DFU mode.
3. Run:

   ```bash
   cargo run --release --features rx
   ```
   for the receiver or
      ```bash
   cargo run --release --features tx
   ```
   for the sender (for testing)

`cargo run` builds the firmware and calls [`flash.sh`](flash.sh) (configured as the
runner in `.cargo/config.toml`), which converts the ELF to Intel HEX, packages it
into a DFU zip, and flashes it. The dongle reboots into the new firmware when done.

### Manual flashing (what `flash.sh` does)

```bash
rust-objcopy -O ihex target/thumbv7em-none-eabihf/release/fw-nrf52840-sniffer fw.hex
nrfutil nrf5sdk-tools pkg generate --hw-version 52 --sd-req 0x00 \
  --application fw.hex --application-version 1 fw.zip
nrfutil device program --firmware fw.zip --traits nordicDfu
```

## 4. Output

Each dongle shows up as a USB serial port, `/dev/ttyACM0`, `/dev/ttyACM1`, ... on Linux
(check with `ls /dev/ttyACM*`). Open one with a serial terminal, e.g. minicom:

```bash
minicom -D /dev/ttyACM0
```

Every frame is printed as one line:

- `rx`, frame on our PAN: `lqi=187 seq=12 dst=0xffff src=0x0001 payload=abcd`
- `rx`, frame from another network: `frame on PAN 0x1a2b detected`
- `tx`, every sent frame: `seq=12 dst=0xffff src=0x0001 payload=abcd`
