// SPDX-License-Identifier: GPL-3.0-or-later
//! Rebuildable source previews. Called only by preparation/control workers, never rendering.
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

const BASE_COLUMNS: usize = 1024;
pub const MAX_ASSETS: usize = 96;
pub type SharedCache = Arc<Mutex<Cache>>;
/// Optional frame-envelope capability for future material providers, without hosting them.
pub trait EnvelopeSource {
    fn identity(&self) -> &str;
    fn frames(&self) -> u64;
    fn rate(&self) -> u32;
    fn envelope(&self, start: u64, end: u64) -> Envelope;
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Envelope {
    pub min: [f32; 2],
    pub max: [f32; 2],
    /// Source RMS, before fades, gain, filtering, deck mixing and limiting.
    pub rms: f32,
}
#[derive(Clone, Copy, Debug)]
struct Bin {
    min: [f32; 2],
    max: [f32; 2],
    squares: f64,
    frames: u64,
}
impl Bin {
    fn merge(self, other: Self) -> Self {
        Self {
            min: std::array::from_fn(|c| self.min[c].min(other.min[c])),
            max: std::array::from_fn(|c| self.max[c].max(other.max[c])),
            squares: self.squares + other.squares,
            frames: self.frames + other.frames,
        }
    }
    fn envelope(self) -> Envelope {
        Envelope {
            min: self.min,
            max: self.max,
            rms: (self.squares / (self.frames * 2) as f64).sqrt() as f32,
        }
    }
}
pub struct Pyramid {
    pub id: String,
    pub frames: u64,
    pub rate: u32,
    stride: u64,
    levels: Vec<Vec<Bin>>,
}
impl EnvelopeSource for Pyramid {
    fn identity(&self) -> &str {
        &self.id
    }
    fn frames(&self) -> u64 {
        self.frames
    }
    fn rate(&self) -> u32 {
        self.rate
    }
    fn envelope(&self, start: u64, end: u64) -> Envelope {
        self.range(start, end)
    }
}
impl Pyramid {
    pub fn identity(pcm: &[f32], channels: usize, rate: u32) -> String {
        let channels = channels.clamp(1, 2);
        let frames = pcm.len() / channels;
        let mut hash = 0xcbf29ce484222325u64;
        for b in rate
            .to_le_bytes()
            .into_iter()
            .chain((frames as u64).to_le_bytes())
        {
            hash = (hash ^ u64::from(b)).wrapping_mul(0x100000001b3);
        }
        for f in pcm.chunks_exact(channels.max(1)) {
            for c in 0..2 {
                for b in f[c.min(channels - 1)].to_bits().to_le_bytes() {
                    hash = (hash ^ u64::from(b)).wrapping_mul(0x100000001b3);
                }
            }
        }
        format!("wave1-{hash:016x}")
    }
    /// Compact stereo min/max pyramid; mono is duplicated without cancelling opposite phases.
    pub fn analyze(pcm: &[f32], channels: usize, rate: u32) -> Result<Self, &'static str> {
        if ![1, 2].contains(&channels)
            || rate == 0
            || pcm.is_empty()
            || !pcm.len().is_multiple_of(channels)
            || pcm.iter().any(|v| !v.is_finite() || v.abs() > 1.0)
        {
            return Err("Invalid waveform source");
        }
        let frames = pcm.len() / channels;
        let stride = frames.div_ceil(BASE_COLUMNS).max(1);
        let mut base = Vec::with_capacity(frames.div_ceil(stride));
        for chunk in pcm.chunks(stride * channels) {
            let mut bin = Bin {
                min: [f32::INFINITY; 2],
                max: [f32::NEG_INFINITY; 2],
                squares: 0.0,
                frames: 0,
            };
            for f in chunk.chunks_exact(channels) {
                for c in 0..2 {
                    let v = f[c.min(channels - 1)];
                    bin.min[c] = bin.min[c].min(v);
                    bin.max[c] = bin.max[c].max(v);
                    bin.squares += f64::from(v) * f64::from(v);
                }
                bin.frames += 1;
            }
            base.push(bin);
        }
        let mut levels = vec![base];
        while levels.last().is_some_and(|l| l.len() > 1) {
            levels.push(
                levels
                    .last()
                    .unwrap()
                    .chunks(2)
                    .map(|c| if c.len() == 2 { c[0].merge(c[1]) } else { c[0] })
                    .collect(),
            );
        }
        Ok(Self {
            id: Self::identity(pcm, channels, rate),
            frames: frames as u64,
            rate,
            stride: stride as u64,
            levels,
        })
    }
    pub fn memory_bytes(&self) -> usize {
        self.levels
            .iter()
            .map(|l| l.capacity() * std::mem::size_of::<Bin>())
            .sum()
    }
    /// Conservative min/max query: edge bins can include neighbouring source frames.
    pub fn range(&self, start: u64, end: u64) -> Envelope {
        let end = end.min(self.frames);
        if start >= end {
            return Envelope::default();
        }
        let mut level = 0;
        while level + 1 < self.levels.len()
            && self.stride * (1u64 << (level + 1)) <= (end - start).max(1) / 2
        {
            level += 1;
        }
        let stride = self.stride * (1u64 << level);
        let a = (start / stride) as usize;
        let b = (end.div_ceil(stride) as usize).min(self.levels[level].len());
        self.levels[level][a..b]
            .iter()
            .copied()
            .reduce(Bin::merge)
            .map_or_else(Envelope::default, Bin::envelope)
    }
    pub fn columns(&self, start: u64, end: u64, count: usize) -> Vec<Envelope> {
        let end = end.min(self.frames);
        let start = start.min(end);
        let count = count.clamp(1, 1024);
        (0..count)
            .map(|i| {
                let a = start + (end - start) * i as u64 / count as u64;
                let b = start + (end - start) * (i + 1) as u64 / count as u64;
                self.range(a, b.max(a + 1).min(end))
            })
            .collect()
    }
}
#[derive(Default)]
pub struct Cache {
    entries: BTreeMap<String, (Arc<Pyramid>, u64)>,
    pinned: BTreeSet<String>,
    clock: u64,
    pub analyses: u64,
}
impl Cache {
    pub fn insert(&mut self, pyramid: Pyramid) -> String {
        let key = pyramid.id.clone();
        self.clock = self.clock.wrapping_add(1);
        if let Some((_, stamp)) = self.entries.get_mut(&key) {
            *stamp = self.clock;
            return key;
        }
        self.analyses += 1;
        while self.entries.len() >= MAX_ASSETS {
            let victim = self
                .entries
                .iter()
                .filter(|(k, _)| !self.pinned.contains(*k))
                .min_by_key(|(_, (_, t))| *t)
                .map(|(k, _)| k.clone());
            let Some(victim) = victim else { break };
            self.entries.remove(&victim);
        }
        self.entries
            .insert(key.clone(), (Arc::new(pyramid), self.clock));
        key
    }
    pub fn get(&mut self, key: &str) -> Option<Arc<Pyramid>> {
        self.clock = self.clock.wrapping_add(1);
        let (p, stamp) = self.entries.get_mut(key)?;
        *stamp = self.clock;
        Some(p.clone())
    }
    pub fn pin(&mut self, keys: impl Iterator<Item = String>) {
        self.pinned = keys.take(16).collect();
    }
    pub fn memory_bytes(&self) -> usize {
        self.entries.values().map(|(p, _)| p.memory_bytes()).sum()
    }
}
