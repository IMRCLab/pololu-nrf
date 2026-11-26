//! USB Mass Storage device class on Embassy.
//!
//! Trimmed copy of `deluge-usb-classes` 0.1.0: only the `msc` and `bot` modules
//! (upstream feature `device-msc`). See `PATCHES.md` for the local changes.

#![no_std]

pub mod bot;
pub mod msc;
