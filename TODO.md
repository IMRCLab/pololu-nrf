# TODO

## Milestones

| Milestone                  | Rough Deadline    |
| -------------------------- | ----------------- |
| PCB ordered                | 15 Dec 2026       |
| Firmware done              | 15 Jan 2027       |
| Single-robot functionality | 15 Feb 2027       |
| Multi-robot solutions      | 31 Mar 2027       |
| Buffer + final docs        | 1 Apr – 15 Apr    |

---

## Firmware

### Peripherals

- [ ] Blinky on nRF with Rust
- [ ] SD card with nRF
- [ ] ToF sensor with nRF
- [ ] nRF UART to Pololu
- [ ] Line sensors
- [ ] ? *Writing from Pololu and nRF to one SD card*
  - Result decides if one or two SD card slots on the PCB are needed

### Communication

- [ ] Communication (TDMA?)
- [ ] Port workstation to nrf comm to Rust
    - ESB vs reserve one TDMA slot for the Crazyradio?

---

## Hardware

- [ ] Design PCB
  - [ ] Schematic
  - [ ] Layout
- [ ] Order boards
- [ ] Assemble and test

---

## Single-robot functionality

- [ ] Cloud map
- [ ] Localization via cloud map
- [ ] Planning algorithms (which ones?)

---

## Multi-robot assignment solutions

- [ ] Consensus
- [ ] Map building 
- [ ] Collision avoidance
- [ ] Exploration

---
## Documentation

- [ ] 