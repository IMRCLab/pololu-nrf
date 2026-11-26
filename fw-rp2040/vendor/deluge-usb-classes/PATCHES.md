# Local changes to deluge-usb-classes

Source: [deluge-usb-classes 0.1.0](https://crates.io/crates/deluge-usb-classes/0.1.0)
from <https://github.com/FirestormAudio/deluge-sdk>, MIT OR Apache-2.0
(see `LICENSE-MIT`, `LICENSE-APACHE`).

Only `src/msc.rs` and `src/bot.rs` were kept (upstream feature `device-msc`).
`Cargo.toml` and `src/lib.rs` are rewritten for this subset.

Patches (candidates for upstream):

1. **Cortex-M0+ support.** thumbv6m has no atomic read-modify-write.
   - `msc.rs`: `AtomicBool::swap` is replaced by load + store in a critical section.
   - `bot.rs`: the `TX_BYTES`/`RX_BYTES` `AtomicU64` throughput counters are removed.
2. **Packet-sized endpoints.** The embassy-usb-driver `read`/`write` calls move a
   single packet. Upstream's driver accepted whole transfers, but drivers like
   embassy-rp's (64-byte full-speed packets) do not. Data phases now use
   `write_transfer` and a `read_exact` helper that reads packets until the
   buffer is full.
3. **Smaller buffers.** `CHUNK_SECTORS` is 8 instead of 64: two 4 KiB banks
   instead of two 32 KiB banks.
4. **Flush hook.** `BlockDevice::flush()` (default no-op) is called on
   SYNCHRONIZE CACHE and on a host eject, so a write-back cache can be written out.
5. **Eject is undone by a replug.** A USB bus reset (`Handler::reset`) sets a
   flag that clears the "ejected" latch before the next command. Upstream only
   cleared it when the transport loop restarted, so a device that keeps running
   across a replug (battery powered) stayed "no medium" forever.
6. **Short data-in phases end with a zero-length packet.** When a command sends
   less than the host asked for (failed or unsupported commands) and the last
   packet was full or nothing was sent, a zero-length packet ends the data
   phase before the CSW. Otherwise the host takes the CSW as data. Upstream's
   BOT would STALL bulk-in here, which embassy-usb doesn't offer to classes.
