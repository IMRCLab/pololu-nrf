//! IEEE 802.15.4 radio.
//! `rx`: puts every correctly received frame into `FRAMES`.
//! `tx`: sends a test frame every 100 ms.

use embassy_executor::Spawner;
use embassy_nrf::radio::ieee802154::{Packet, Radio};
use embassy_nrf::{Peri, bind_interrupts, peripherals};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

bind_interrupts!(struct Irqs {
    RADIO => embassy_nrf::radio::InterruptHandler<peripherals::RADIO>;
});

/// 802.15.4 channel to use (11..=26)
const CHANNEL: u8 = 25;

/// Frames to report over USB (`rx`: received, `tx`: sent). Full queue = frame dropped.
pub static FRAMES: Channel<CriticalSectionRawMutex, Packet, 8> = Channel::new();

/// Sets up the radio and spawns the task for the selected role.
/// Requires the HFCLK to run from the external crystal (see `main`).
pub fn init(spawner: Spawner, radio: Peri<'static, peripherals::RADIO>) {
    let mut radio = Radio::new(radio, Irqs);
    radio.set_channel(CHANNEL);

    #[cfg(feature = "rx")]
    spawner.spawn(listener_task(radio).unwrap());
    #[cfg(feature = "tx")]
    spawner.spawn(transmitter_task(radio).unwrap());
}

#[cfg(feature = "rx")]
#[embassy_executor::task]
async fn listener_task(mut radio: Radio<'static>) {
    loop {
        let mut packet = Packet::new();
        // Err = CRC failed: frame was corrupted on air, drop it
        if radio.receive(&mut packet).await.is_ok() {
            // Never wait here, so the radio is listening again right away
            let _ = FRAMES.try_send(packet);
        }
    }
}

/// This device's short address
#[cfg(feature = "tx")]
const SRC_ADDR: u16 = 0x0001;
/// Receiver's short address. 0xFFFF = broadcast (everyone)
#[cfg(feature = "tx")]
const DST_ADDR: u16 = 0xFFFF;
/// Sent after the header in every frame
#[cfg(feature = "tx")]
const PAYLOAD: &[u8] = &[0xAB, 0xCD];

#[cfg(feature = "tx")]
#[embassy_executor::task]
async fn transmitter_task(mut radio: Radio<'static>) {
    use crate::frame::Header;
    use embassy_time::{Duration, Ticker};

    let mut ticker = Ticker::every(Duration::from_millis(100));
    let mut seq: u8 = 0;
    loop {
        ticker.next().await;

        let header = Header {
            seq,
            dst: DST_ADDR,
            src: SRC_ADDR,
        };
        let header = header.to_bytes();

        // | header | payload |
        let mut packet = Packet::new();
        packet.set_len((header.len() + PAYLOAD.len()) as u8);
        let (head, payload) = packet.split_at_mut(header.len());
        head.copy_from_slice(&header);
        payload.copy_from_slice(PAYLOAD);
        // Err = channel busy (CCA): skip this one, try again next tick
        if radio.try_send(&mut packet).await.is_ok() {
            let _ = FRAMES.try_send(packet);
        }

        seq = seq.wrapping_add(1);
    }
}
