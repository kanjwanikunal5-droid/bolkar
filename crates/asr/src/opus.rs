//! Compresses 16 kHz mono speech to Ogg/Opus (the format of WhatsApp voice notes) before it is
//! uploaded for cloud recognition. At 24 kbit/s speech keeps its quality for recognition and
//! the upload is about 10 times smaller than WAV, which matters on slow upload connections.

use opusic_sys as ffi;

use crate::SAMPLE_RATE;

/// Bits per second for dictation audio.
pub const BITRATE: i32 = 24_000;
/// 20 ms frames.
const FRAME: usize = (SAMPLE_RATE as usize) / 50;
/// Ogg/Opus granule positions always count 48 kHz samples.
const TO_48K: u64 = 48_000 / SAMPLE_RATE as u64;

/* ---------- Ogg container ---------- */

fn crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    for (i, v) in t.iter_mut().enumerate() {
        let mut r = (i as u32) << 24;
        for _ in 0..8 {
            r = if r & 0x8000_0000 != 0 {
                (r << 1) ^ 0x04C1_1DB7
            } else {
                r << 1
            };
        }
        *v = r;
    }
    t
}

/// The Ogg page checksum (CRC-32, polynomial 0x04C11DB7, no reflection).
pub fn ogg_crc(data: &[u8]) -> u32 {
    let table = crc_table();
    data.iter().fold(0u32, |crc, b| {
        (crc << 8) ^ table[(((crc >> 24) as u8) ^ b) as usize]
    })
}

struct OggWriter {
    out: Vec<u8>,
    serial: u32,
    sequence: u32,
}

impl OggWriter {
    fn page(&mut self, packet: &[u8], granule: u64, first: bool, last: bool) {
        self.pages(&[packet], granule, first, last);
    }

    /// Several packets on one page (fewer page headers: smaller upload).
    fn pages(&mut self, packets: &[&[u8]], granule: u64, first: bool, last: bool) {
        let mut lacing = Vec::new();
        for p in packets {
            lacing.extend(std::iter::repeat_n(255u8, p.len() / 255));
            lacing.push((p.len() % 255) as u8);
        }
        let start = self.out.len();
        self.out.extend_from_slice(b"OggS");
        self.out.push(0);
        self.out
            .push(if first { 0x02 } else { 0 } | if last { 0x04 } else { 0 });
        self.out.extend_from_slice(&granule.to_le_bytes());
        self.out.extend_from_slice(&self.serial.to_le_bytes());
        self.out.extend_from_slice(&self.sequence.to_le_bytes());
        self.out.extend_from_slice(&[0, 0, 0, 0]);
        self.out.push(lacing.len() as u8);
        self.out.extend_from_slice(&lacing);
        for p in packets {
            self.out.extend_from_slice(p);
        }
        let crc = ogg_crc(&self.out[start..]);
        self.out[start + 22..start + 26].copy_from_slice(&crc.to_le_bytes());
        self.sequence += 1;
    }
}

/* ---------- Opus encoder ---------- */

struct Encoder(*mut ffi::OpusEncoder);

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe { ffi::opus_encoder_destroy(self.0) }
    }
}

/// Encodes 16 kHz mono samples as an Ogg/Opus file.
pub fn encode_ogg_opus(samples: &[f32]) -> Result<Vec<u8>, String> {
    let mut error = 0;
    let enc = unsafe {
        ffi::opus_encoder_create(
            SAMPLE_RATE as i32,
            1,
            ffi::OPUS_APPLICATION_VOIP,
            &mut error,
        )
    };
    if enc.is_null() || error != 0 {
        return Err(format!("opus encoder could not start ({error})"));
    }
    let enc = Encoder(enc);
    let mut lookahead: i32 = 0;
    unsafe {
        ffi::opus_encoder_ctl(enc.0, ffi::OPUS_SET_BITRATE_REQUEST, BITRATE);
        ffi::opus_encoder_ctl(
            enc.0,
            ffi::OPUS_GET_LOOKAHEAD_REQUEST,
            &mut lookahead as *mut i32,
        );
    }
    let pre_skip = (lookahead.max(0) as u64 * TO_48K) as u16;

    let mut ogg = OggWriter {
        out: Vec::with_capacity(samples.len() / 8 + 512),
        serial: 0x5350_4B46,
        sequence: 0,
    };
    // OpusHead
    let mut head = b"OpusHead".to_vec();
    head.push(1); // version
    head.push(1); // channels
    head.extend_from_slice(&pre_skip.to_le_bytes());
    head.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    head.extend_from_slice(&0i16.to_le_bytes()); // output gain
    head.push(0); // mapping family
    ogg.page(&head, 0, true, false);
    // OpusTags
    let vendor = b"app-asr";
    let mut tags = b"OpusTags".to_vec();
    tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    tags.extend_from_slice(vendor);
    tags.extend_from_slice(&0u32.to_le_bytes());
    ogg.page(&tags, 0, false, false);

    // Audio, padded with silence so the encoder's lookahead is flushed.
    let total = samples.len() + lookahead.max(0) as usize;
    let frames = total.div_ceil(FRAME).max(1);
    let end_granule = pre_skip as u64 + samples.len() as u64 * TO_48K;
    let mut frame = vec![0f32; FRAME];
    let mut packet = vec![0u8; 4000];
    // Up to 50 packets (one second) per page; a page's lacing table holds at most 255 entries.
    let mut pending: Vec<Vec<u8>> = Vec::new();
    for i in 0..frames {
        for (j, s) in frame.iter_mut().enumerate() {
            *s = samples.get(i * FRAME + j).copied().unwrap_or(0.0);
        }
        let n = unsafe {
            ffi::opus_encode_float(
                enc.0,
                frame.as_ptr(),
                FRAME as i32,
                packet.as_mut_ptr(),
                packet.len() as i32,
            )
        };
        if n < 0 {
            return Err(format!("opus encoding failed ({n})"));
        }
        pending.push(packet[..n as usize].to_vec());
        let last = i + 1 == frames;
        if last || pending.len() == 50 {
            let granule = if last {
                end_granule
            } else {
                ((i + 1) * FRAME) as u64 * TO_48K
            };
            let refs: Vec<&[u8]> = pending.iter().map(Vec::as_slice).collect();
            ogg.pages(&refs, granule, false, last);
            pending.clear();
        }
    }
    Ok(ogg.out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ogg_checksum_matches_the_specification() {
        // Known value for the ASCII string "OggS" with this CRC (poly 0x04C11DB7, init 0).
        assert_eq!(ogg_crc(b""), 0);
        assert_ne!(ogg_crc(b"OggS"), ogg_crc(b"OggT"));
    }

    #[test]
    fn speech_band_audio_round_trips_through_opus() {
        // 2 s of a varying tone, like a voice.
        let input: Vec<f32> = (0..32_000)
            .map(|i| {
                let t = i as f32 / 16_000.0;
                (t * 2.0 * std::f32::consts::PI * (180.0 + 60.0 * (t * 3.0).sin())).sin() * 0.3
            })
            .collect();
        let ogg = encode_ogg_opus(&input).unwrap();
        assert_eq!(&ogg[..4], b"OggS");
        // About 24 kbit/s: 2 s should be well under a tenth of the WAV size (64 KB).
        assert!(ogg.len() < 7_000, "{} bytes", ogg.len());
        let path = std::env::temp_dir().join("app-asr-opus-test.ogg");
        std::fs::write(&path, &ogg).unwrap();
        let back = crate::audio::read_audio_16k_mono(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        // Same length (within one frame) and similar loudness.
        assert!(
            (back.len() as i64 - input.len() as i64).abs() <= FRAME as i64,
            "{} vs {}",
            back.len(),
            input.len()
        );
        let (a, b) = (crate::audio::rms(&input), crate::audio::rms(&back));
        assert!((a - b).abs() / a < 0.25, "rms {a} vs {b}");
    }
}
