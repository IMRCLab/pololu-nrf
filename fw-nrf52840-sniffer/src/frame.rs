//! Our 802.15.4 frame format. All fields little-endian:
//! | frame control (2) | seq (1) | PAN ID (2) | dst (2) | src (2) | payload |

/// Network ID shared by all our devices
pub const PAN_ID: u16 = 0x1234;

/// Data frame, PAN ID compression, short dst + src address, frame version 1 (2006)
const FRAME_CONTROL: u16 = 0x9841;

const HEADER_LEN: usize = 9;

pub struct Header {
    pub seq: u8,
    pub dst: u16,
    pub src: u16,
}

impl Header {
    #[cfg(feature = "tx")]
    pub fn to_bytes(&self) -> [u8; HEADER_LEN] {
        let [fc0, fc1] = FRAME_CONTROL.to_le_bytes();
        let [pan0, pan1] = PAN_ID.to_le_bytes();
        let [dst0, dst1] = self.dst.to_le_bytes();
        let [src0, src1] = self.src.to_le_bytes();
        [fc0, fc1, self.seq, pan0, pan1, dst0, dst1, src0, src1]
    }

    /// Splits one of our frames into header and payload. `None` = not ours.
    pub fn parse(frame: &[u8]) -> Option<(Header, &[u8])> {
        let (h, payload) = frame.split_first_chunk::<HEADER_LEN>()?;

        let fc = u16::from_le_bytes([h[0], h[1]]);
        let pan = u16::from_le_bytes([h[3], h[4]]);
        let dst = u16::from_le_bytes([h[5], h[6]]);
        let src = u16::from_le_bytes([h[7], h[8]]);

        if fc != FRAME_CONTROL || pan != PAN_ID {
            return None;
        }
        let header = Header {
            seq: h[2],
            dst,
            src,
        };
        Some((header, payload))
    }
}

/// PAN ID of a foreign frame, if it has one. In 2003/2006 frames with any address,
/// the first PAN ID is right after the seq number. Other frames: `None`.
#[cfg(feature = "rx")]
pub fn pan_id(frame: &[u8]) -> Option<u16> {
    let h = frame.first_chunk::<5>()?; // None if shorter than 5 bytes
    let fc = u16::from_le_bytes([h[0], h[1]]);
    let version = (fc >> 12) & 0b11;
    let has_address = fc & 0xcc00 != 0; // dst or src address mode set
    if version > 1 || !has_address {
        return None;
    }
    Some(u16::from_le_bytes([h[3], h[4]]))
}
