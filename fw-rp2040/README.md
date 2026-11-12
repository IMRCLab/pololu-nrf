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
