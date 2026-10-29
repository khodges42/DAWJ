// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded resident WAV preparation. No decoder, I/O or ownership replacement in rendering.
use parts::{AssetId, Deck, Part, PlaybackMode, SLOTS, State};
use std::{fs::File, io::Read, path::Path};
pub const MAX_WAV_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_DECK_FRAMES: usize = 8 * 60 * 192000;

#[derive(Clone, Copy, Debug)]
pub enum BoundaryPolicy {
    Preserve,
    Fade { frames: u32 },
}

/// Canonical interleaved stereo at the declared stream rate.
pub struct LoadedSample {
    id: AssetId,
    rate: u32,
    pcm: Box<[f32]>,
}
impl LoadedSample {
    pub fn load(id: AssetId, path: &Path, rate: u32) -> Result<Self, String> {
        let mut data = Vec::new();
        File::open(path)
            .map_err(|e| e.to_string())?
            .take((MAX_WAV_BYTES + 1) as u64)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        Self::decode(id, &data, rate).map_err(str::to_owned)
    }
    pub fn frames(&self) -> usize {
        self.pcm.len() / 2
    }
    pub fn decode(id: AssetId, bytes: &[u8], rate: u32) -> Result<Self, &'static str> {
        let invalid = "unsupported or malformed WAV";
        if bytes.len() > MAX_WAV_BYTES
            || bytes.len() < 12
            || &bytes[..4] != b"RIFF"
            || &bytes[8..12] != b"WAVE"
        {
            return Err(invalid);
        }
        let u16at = |b: &[u8], i| u16::from_le_bytes([b[i], b[i + 1]]);
        let u32at = |b: &[u8], i| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        let end = (u32at(bytes, 4) as usize).checked_add(8).ok_or(invalid)?;
        if end != bytes.len() {
            return Err(invalid);
        }
        let mut fmt = None;
        let mut data = None;
        let mut offset = 12;
        while offset < end {
            if end - offset < 8 {
                return Err(invalid);
            }
            let len = u32at(bytes, offset + 4) as usize;
            let start = offset + 8;
            let next = start.checked_add(len).ok_or(invalid)?;
            if next > end {
                return Err(invalid);
            }
            match &bytes[offset..offset + 4] {
                b"fmt " => {
                    if fmt.is_some() || len < 16 {
                        return Err(invalid);
                    }
                    fmt = Some(&bytes[start..next]);
                }
                b"data" => {
                    if data.is_some() {
                        return Err(invalid);
                    }
                    data = Some(&bytes[start..next]);
                }
                _ => {}
            }
            offset = next.checked_add(len % 2).ok_or(invalid)?;
            if offset > end {
                return Err(invalid);
            }
        }
        let f = fmt.ok_or(invalid)?;
        let data = data.ok_or(invalid)?;
        let encoding = u16at(f, 0);
        let channels = u16at(f, 2);
        let source_rate = u32at(f, 4);
        let bits = u16at(f, 14);
        if rate == 0 || source_rate != rate {
            return Err(
                "sample-rate mismatch: prepare an asset at the stream rate; no runtime resampling",
            );
        }
        if !(channels == 1 || channels == 2)
            || !matches!((encoding, bits), (1, 16 | 24 | 32) | (3, 32))
        {
            return Err(invalid);
        }
        let width = usize::from(bits / 8);
        let align = usize::from(channels) * width;
        if usize::from(u16at(f, 12)) != align
            || u32at(f, 8) != rate.checked_mul(align as u32).ok_or(invalid)?
            || data.is_empty()
            || data.len() % align != 0
        {
            return Err(invalid);
        }
        let mut pcm = Vec::with_capacity(data.len() / align * 2);
        for frame in data.chunks_exact(align) {
            let mut stereo = [0.0; 2];
            for ch in 0..usize::from(channels) {
                let b = &frame[ch * width..(ch + 1) * width];
                stereo[ch] = match (encoding, bits) {
                    (1, 16) => f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0,
                    (1, 24) => {
                        ((i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32) / 8388608.0
                    }
                    (1, 32) => {
                        (i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64 / 2147483648.0) as f32
                    }
                    (3, 32) => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                    _ => return Err(invalid),
                };
                if !stereo[ch].is_finite() || stereo[ch].abs() > 1.0 {
                    return Err("nonfinite or out-of-range WAV samples");
                }
            }
            if channels == 1 {
                stereo[1] = stereo[0];
            }
            pcm.extend_from_slice(&stereo);
        }
        Ok(Self {
            id,
            rate,
            pcm: pcm.into_boxed_slice(),
        })
    }
}

pub struct SampleVoice {
    rate: u32,
    part: Part,
    pcm: Box<[f32]>,
    start: usize,
    end: usize,
    position: usize,
    elapsed: u64,
}
impl SampleVoice {
    pub fn prepare(
        part: Part,
        sample: LoadedSample,
        rate: u32,
        policy: BoundaryPolicy,
    ) -> Result<Self, &'static str> {
        part.validate()?;
        if sample.id != part.playback.asset || sample.rate != rate {
            return Err("part asset identity or rate mismatch");
        }
        let (start, end) = match part.playback.mode {
            PlaybackMode::OneShot => (0, sample.frames()),
            PlaybackMode::Loop { start, end } => (
                usize::try_from(start).map_err(|_| "loop overflow")?,
                usize::try_from(end).map_err(|_| "loop overflow")?,
            ),
        };
        if start >= end || end > sample.frames() {
            return Err("loop outside sample");
        }
        let mut pcm = sample.pcm;
        if let BoundaryPolicy::Fade { frames } = policy {
            let length = end - start;
            // Preserve 1/2/3-frame transients. Longer regions receive finite prepared fades.
            if length >= 4 {
                let fade = (frames as usize).min(length / 2);
                for i in 0..fade {
                    let gain = i as f32 / fade as f32;
                    for ch in 0..2 {
                        pcm[(start + i) * 2 + ch] *= gain;
                        pcm[(end - 1 - i) * 2 + ch] *= gain;
                    }
                }
            }
        }
        Ok(Self {
            part,
            rate,
            pcm,
            start,
            end,
            position: start,
            elapsed: 0,
        })
    }
    pub(crate) fn rate(&self) -> u32 {
        self.rate
    }
    pub fn part(&self) -> Part {
        self.part
    }
    /// Next source frame, in canonical asset coordinates. EOF is exclusive end.
    pub fn position(&self) -> usize {
        self.position
    }
    pub fn elapsed_frames(&self) -> u64 {
        self.elapsed
    }
    pub fn finished(&self) -> bool {
        matches!(self.part.playback.mode, PlaybackMode::OneShot) && self.position == self.end
    }
    pub fn next_frame(&mut self) -> [f32; 2] {
        if self.finished() {
            return [0.0; 2];
        }
        let frame = [self.pcm[self.position * 2], self.pcm[self.position * 2 + 1]];
        self.position += 1;
        self.elapsed = self.elapsed.saturating_add(1);
        if self.position == self.end && matches!(self.part.playback.mode, PlaybackMode::Loop { .. })
        {
            self.position = self.start;
        }
        frame
    }
    pub fn length(&self) -> u64 {
        if matches!(self.part.playback.mode, PlaybackMode::Loop { .. }) {
            u64::MAX
        } else {
            (self.end - self.start) as u64
        }
    }
}

/// Snapshot adapter only: queued domain actions require the later ordered scheduler bridge.
pub struct SampleDeck {
    rate: u32,
    voices: [Option<SampleVoice>; SLOTS],
    length: u64,
}
impl SampleDeck {
    pub fn prepare(
        model: &Deck,
        mut assets: [Option<LoadedSample>; SLOTS],
        rate: u32,
        policy: BoundaryPolicy,
    ) -> Result<Self, &'static str> {
        let mut voices = std::array::from_fn(|_| None);
        let mut length = 0;
        let mut resident = 0_usize;
        for (slot, state) in model.states().iter().enumerate() {
            match state {
                State::Playing(part) => {
                    let sample = assets[slot].take().ok_or("missing active part asset")?;
                    resident = resident
                        .checked_add(sample.frames())
                        .ok_or("deck memory overflow")?;
                    if resident > MAX_DECK_FRAMES {
                        return Err("deck resident PCM limit exceeded");
                    }
                    let voice = SampleVoice::prepare(*part, sample, rate, policy)?;
                    length = length.max(voice.length());
                    voices[slot] = Some(voice);
                }
                State::Stopped if assets[slot].is_none() => {}
                _ => {
                    return Err(
                        "prepare only acknowledged playing/stopped snapshots with matching assets",
                    );
                }
            }
        }
        Ok(Self {
            voices,
            length,
            rate,
        })
    }
    pub(crate) fn rate(&self) -> u32 {
        self.rate
    }
    pub(crate) fn length(&self) -> u64 {
        self.length
    }
    pub(crate) fn next_frame(&mut self) -> [f32; 2] {
        let mut mixed = [0.0; 2];
        for voice in self.voices.iter_mut().flatten() {
            let frame = voice.next_frame();
            // Fixed eight-slot summing headroom; master/cue protection remains downstream.
            for ch in 0..2 {
                mixed[ch] += frame[ch] / SLOTS as f32;
            }
        }
        mixed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parts::{Command, PartId, PlaybackConfiguration, TimingPolicy};
    use timebase::BeatTicks;
    fn wav(channels: u16, encoding: u16, bits: u16, data: &[u8]) -> Vec<u8> {
        let align = channels * (bits / 8);
        let padding = data.len() % 2;
        let mut b = Vec::new();
        b.extend(b"RIFF");
        b.extend((36 + data.len() as u32 + padding as u32).to_le_bytes());
        b.extend(b"WAVEfmt ");
        b.extend(16_u32.to_le_bytes());
        b.extend(encoding.to_le_bytes());
        b.extend(channels.to_le_bytes());
        b.extend(48000_u32.to_le_bytes());
        b.extend((48000 * u32::from(align)).to_le_bytes());
        b.extend(align.to_le_bytes());
        b.extend(bits.to_le_bytes());
        b.extend(b"data");
        b.extend((data.len() as u32).to_le_bytes());
        b.extend(data);
        if padding != 0 {
            b.push(0);
        }
        b
    }
    fn part(mode: PlaybackMode) -> Part {
        Part {
            id: PartId(1),
            playback: PlaybackConfiguration {
                asset: AssetId(1),
                mode,
            },
            timing: TimingPolicy {
                nominal_length: BeatTicks(960),
                quantum: BeatTicks(240),
            },
        }
    }
    fn loaded() -> LoadedSample {
        LoadedSample::decode(
            AssetId(1),
            &wav(1, 1, 16, &[0, 32, 0, 64, 0, 192, 0, 224]),
            48000,
        )
        .unwrap()
    }
    #[test]
    fn mono_oneshot_stereo_loop_and_exact_positions() {
        let mut v = SampleVoice::prepare(
            part(PlaybackMode::OneShot),
            loaded(),
            48000,
            BoundaryPolicy::Preserve,
        )
        .unwrap();
        for expected in [0.25, 0.5, -0.5, -0.25] {
            assert_eq!(v.next_frame(), [expected; 2]);
        }
        assert!(v.finished());
        assert_eq!(v.next_frame(), [0.0; 2]);
        assert_eq!(v.position(), 4);
        assert_eq!(v.elapsed_frames(), 4);
        let mut v = SampleVoice::prepare(
            part(PlaybackMode::Loop { start: 1, end: 3 }),
            loaded(),
            48000,
            BoundaryPolicy::Preserve,
        )
        .unwrap();
        for expected in [0.5, -0.5, 0.5, -0.5] {
            assert_eq!(v.next_frame(), [expected; 2]);
        }
        assert_eq!(v.position(), 1);
        assert!(!v.finished());
        let data: [u8; 8] = [0, 32, 0, 192, 0, 64, 0, 224];
        let s = LoadedSample::decode(AssetId(1), &wav(2, 1, 16, &data), 48000).unwrap();
        let mut v = SampleVoice::prepare(
            part(PlaybackMode::OneShot),
            s,
            48000,
            BoundaryPolicy::Preserve,
        )
        .unwrap();
        assert_eq!(v.next_frame(), [0.25, -0.5]);
    }
    #[test]
    fn decoding_and_rejection_matrix() {
        for (encoding, bits, data) in [
            (1, 24, vec![0, 0, 64]),
            (1, 32, 1073741824_i32.to_le_bytes().to_vec()),
            (3, 32, 0.5_f32.to_le_bytes().to_vec()),
        ] {
            let s =
                LoadedSample::decode(AssetId(1), &wav(1, encoding, bits, &data), 48000).unwrap();
            assert_eq!(&*s.pcm, &[0.5, 0.5]);
        }
        let good = wav(1, 1, 16, &[0, 32]);
        for end in 0..good.len() {
            assert!(LoadedSample::decode(AssetId(1), &good[..end], 48000).is_err());
        }
        assert!(LoadedSample::decode(AssetId(1), &good, 44100).is_err());
        for (ch, enc, bits, data) in [
            (3, 1, 16, vec![0; 6]),
            (1, 1, 8, vec![0]),
            (1, 3, 32, f32::NAN.to_le_bytes().to_vec()),
            (1, 3, 32, 2.0_f32.to_le_bytes().to_vec()),
            (1, 1, 16, vec![]),
        ] {
            assert!(LoadedSample::decode(AssetId(1), &wav(ch, enc, bits, &data), 48000).is_err());
        }
        assert!(
            LoadedSample::load(
                AssetId(1),
                Path::new("target/nonexistent-sample.wav"),
                48000
            )
            .is_err()
        );
        assert!(
            SampleVoice::prepare(
                part(PlaybackMode::Loop { start: 0, end: 5 }),
                loaded(),
                48000,
                BoundaryPolicy::Preserve
            )
            .is_err()
        );
        let mut wrong = part(PlaybackMode::OneShot);
        wrong.playback.asset = AssetId(2);
        assert!(SampleVoice::prepare(wrong, loaded(), 48000, BoundaryPolicy::Preserve).is_err());
    }
    #[test]
    fn preparation_fades_and_short_onsets() {
        let mut v = SampleVoice::prepare(
            part(PlaybackMode::Loop { start: 0, end: 4 }),
            loaded(),
            48000,
            BoundaryPolicy::Fade { frames: 2 },
        )
        .unwrap();
        let frames: Vec<_> = (0..8).map(|_| v.next_frame()).collect();
        assert_eq!(frames[0], [0.0; 2]);
        assert_eq!(frames[3], [0.0; 2]);
        assert_eq!(frames[4], [0.0; 2]);
        let s = LoadedSample::decode(AssetId(1), &wav(1, 1, 16, &[0, 64]), 48000).unwrap();
        let voice = SampleVoice::prepare(
            part(PlaybackMode::OneShot),
            s,
            48000,
            BoundaryPolicy::Fade { frames: 32 },
        )
        .unwrap();
        let source = crate::playback::PlaybackSource::Sample(voice);
        let (mut mixer, _) = crate::mixer::StereoMixer::new(
            source,
            crate::playback::PlaybackSource::Tone {
                frequency_hz: 220.0,
                amplitude: 0.0,
                frames: 1,
            },
            48000,
        )
        .unwrap();
        let mut out = [0.0; 2];
        mixer.render(&mut out);
        assert!(out[0] > 0.0);
    }
    #[test]
    fn eight_part_snapshot_playback_partition_and_eof() {
        fn run(block: usize) -> Vec<f32> {
            let mut model = Deck::new(BeatTicks(0), BeatTicks(3840)).unwrap();
            for slot in 0..SLOTS {
                model
                    .command(
                        slot,
                        Command::Start {
                            part: part(PlaybackMode::Loop { start: 1, end: 3 }),
                            at: BeatTicks(0),
                        },
                    )
                    .unwrap();
            }
            model.advance(BeatTicks(0)).unwrap();
            let deck = SampleDeck::prepare(
                &model,
                std::array::from_fn(|_| Some(loaded())),
                48000,
                BoundaryPolicy::Preserve,
            )
            .unwrap();
            let shared = std::sync::Arc::new(crate::playback::Shared::default());
            shared
                .enabled
                .store(true, std::sync::atomic::Ordering::Release);
            let mut render = crate::playback::Renderer::new(
                crate::playback::PlaybackSource::Parts(Box::new(deck)),
                48000,
                2,
                shared.clone(),
            )
            .unwrap();
            let mut out = vec![0.0; 1000];
            for chunk in out.chunks_mut(block * 2) {
                render.render(chunk, |s| s);
            }
            assert_eq!(
                shared
                    .source_frames
                    .load(std::sync::atomic::Ordering::Relaxed),
                500
            );
            assert!(!shared.finished.load(std::sync::atomic::Ordering::Relaxed));
            out
        }
        assert_eq!(run(1), run(31));
        assert_eq!(run(128), run(8192));
        assert_eq!(&run(1)[..8], &[0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, -0.5]);
        let model = Deck::new(BeatTicks(0), BeatTicks(3840)).unwrap();
        assert!(
            SampleDeck::prepare(
                &model,
                std::array::from_fn(|_| Some(loaded())),
                48000,
                BoundaryPolicy::Preserve
            )
            .is_err()
        );
    }
}
