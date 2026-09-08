//! Spec section 22: modal audio as a pure function of the stream.
//! Contact records excite damped second-order resonators; the whole
//! path stays in the +,-,*,/ closure (plus floor for quantization) so
//! every conforming implementation is bit-identical cross-platform.

use crate::fnv1a64;

pub const AUDIO_SR: u32 = 65536;
pub const SAMPLES_PER_TICK: u64 = 64;
pub const RING: usize = 16384;
pub const TAIL_BLOCKS: u64 = 260;
pub const OMEGA0: f64 = 0.0004448824124529259;
const PARTIAL: [f64; 3] = [1.0, 4.0, 9.0];
const RHO: [f64; 3] = [0.9990, 0.9985, 0.9980];
const AMP: [f64; 3] = [0.5, 0.3, 0.2];

pub struct Excitation {
    pub tick: u64,
    pub mass_a: f64,
    pub mass_b: f64,
    pub jn: f64,
}

pub fn synthesize(items: &[Excitation], final_tick: u64) -> Vec<i16> {
    let n = ((final_tick + TAIL_BLOCKS) * SAMPLES_PER_TICK) as usize;
    let mut buf = vec![0.0f64; n];
    for item in items {
        let e = ((item.tick + 1) * SAMPLES_PER_TICK) as usize;
        let mu = (item.mass_a * item.mass_b) / (item.mass_a + item.mass_b);
        for k in 0..3 {
            let omega = OMEGA0 * PARTIAL[k] / mu;
            let a = (2.0 - omega) * RHO[k];
            let b = RHO[k] * RHO[k];
            let s0 = AMP[k] * item.jn;
            let mut s_prev = s0;
            let mut s_prev2 = 0.0f64;
            for i in 0..RING {
                let s = if i == 0 {
                    s0
                } else if i == 1 {
                    a * s0
                } else {
                    a * s_prev - b * s_prev2
                };
                buf[e + i] += s;
                s_prev2 = s_prev;
                s_prev = s;
            }
        }
    }
    let mut pcm = Vec::with_capacity(n);
    for v in &buf {
        let clamped = v.clamp(-1.0, 1.0);
        pcm.push((clamped * 32767.0 + 0.5).floor() as i16);
    }
    pcm
}

pub fn wav_bytes(pcm: &[i16]) -> Vec<u8> {
    let data_size = (pcm.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_size as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36u32 + data_size).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&AUDIO_SR.to_le_bytes());
    out.extend_from_slice(&(AUDIO_SR * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());
    for s in pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

pub fn pcm_hash(pcm: &[i16]) -> u64 {
    let mut bytes = Vec::with_capacity(pcm.len() * 2);
    for s in pcm {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    fnv1a64(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_layout() {
        let wav = wav_bytes(&[0i16, 1, -1, 32767]);
        assert_eq!(wav.len(), 44 + 8);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(u32::from_le_bytes(wav[4..8].try_into().unwrap()), 44);
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 8);
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 1);
        assert_eq!(
            u32::from_le_bytes(wav[24..28].try_into().unwrap()),
            AUDIO_SR
        );
        assert_eq!(wav[44], 0);
        assert_eq!(wav[45], 0);
        assert_eq!(wav[46], 1);
        assert_eq!(wav[47], 0);
        assert_eq!(wav[48], 0xff);
        assert_eq!(wav[49], 0xff);
    }

    #[test]
    fn silence_hashes_to_fnv_of_zeros() {
        let pcm = synthesize(&[], 3);
        assert_eq!(pcm.len(), (3 + TAIL_BLOCKS) as usize * 64);
        assert!(pcm.iter().all(|&s| s == 0));
        assert_eq!(pcm_hash(&pcm), fnv1a64(&vec![0u8; pcm.len() * 2]));
    }

    #[test]
    fn synthesis_bit_identical_replay() {
        let items = vec![
            Excitation {
                tick: 5,
                mass_a: 1.25,
                mass_b: 0.75,
                jn: 0.3,
            },
            Excitation {
                tick: 40,
                mass_a: 2.0,
                mass_b: 1.5,
                jn: 0.05,
            },
        ];
        assert_eq!(synthesize(&items, 100), synthesize(&items, 100));
    }

    #[test]
    fn excited_ring_decays_and_stays_bounded() {
        let pcm = synthesize(
            &[Excitation {
                tick: 0,
                mass_a: 1.0,
                mass_b: 1.0,
                jn: 0.8,
            }],
            300,
        );
        let peak = pcm.iter().map(|&s| s.abs() as i32).max().expect("nonempty");
        assert!(peak > 8000, "peak {peak}");
        let tail_max = pcm[RING + 128..RING + 192]
            .iter()
            .map(|&s| s.abs() as i32)
            .max()
            .expect("nonempty");
        assert_eq!(tail_max, 0, "ring fully decayed at L");
    }
}
