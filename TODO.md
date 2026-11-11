# TODO

## Milestones

| Milestone                  | Rough Deadline    |
| -------------------------- | ----------------- |
| Firmware done              | 31 Dec 2026       |
| PCB ordered                | 15 Jan 2027       |
| Single-robot functionality | 15 Feb 2027       |
| Multi-robot solutions      | 31 Mar 2027       |
| Buffer + final docs        | 1 Apr – 15 Apr    |

---

## Firmware
### Architecture
- [x] Define which MCU runs what
- [ ] Define packet formats between MCUs
- [ ] Define packet formats between radios

### Peripherals

- [ ] Blinky on nRF with Rust
- [ ] nRF UART to Pololu
- [ ] SD card with nRF
- [ ] ToF sensor with nRF
- [x] Line sensors

### Communication
- [ ] Integrate embassy-nrf 802.15.4
  - [ ] broadcast send/receive
  - [ ] unicast send/receive
- [ ] Joystick control (replace Crazyradio with nRF (rust))
- [ ] Sniffer


---

## Hardware

- [ ] Design PCB
  - [ ] Schematic
  - [ ] Layout
- [ ] Order boards
- [ ] Assemble and test

---

## Single-robot functionality

- [ ] Localization via odometry + line sensor correction
- [ ] Occupancy grid from ToF sensor
- [ ] Controller
- [ ] Planner (A*)

---
## Documentation

- [ ] 

---
## (Multi-robot assignment solutions) ! -> private repo!

- [ ] Consensus
- [ ] Map building 
- [ ] Collision avoidance
- [ ] Exploration
