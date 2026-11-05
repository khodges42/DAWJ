// SPDX-License-Identifier: GPL-3.0-or-later
//! Material previews and manual musical meaning, not another playback scheduler.
use crate::{
    Coordinator,
    session::{MediaInfo, Sound, Workspace},
    waveform::{Envelope, EnvelopeSource, SharedCache},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    hash::{Hash, Hasher},
};
use timebase::{BeatTicks, TempoChange, TempoMap};
pub const BAR: i64 = 3840;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub id: u32,
    pub tick: i64,
    pub label: String,
    #[serde(default)]
    pub intensity: Option<f32>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Structure {
    pub bars: u8,
    pub sections: Vec<Section>,
    #[serde(default = "yes")]
    pub show_labels: bool,
}
fn yes() -> bool {
    true
}
impl Default for Structure {
    fn default() -> Self {
        Self {
            bars: 8,
            sections: Vec::new(),
            show_labels: true,
        }
    }
}
impl Structure {
    pub fn validate(&self) -> Result<(), String> {
        if self.bars == 0 || self.bars > 64 || self.sections.len() > 64 {
            return Err("Structure requires 1-64 bars and at most 64 sections".into());
        }
        let mut ids = BTreeSet::new();
        let mut last = -1;
        for s in &self.sections {
            if s.id == 0
                || !ids.insert(s.id)
                || s.tick <= last
                || s.tick >= i64::from(self.bars) * BAR
                || s.tick % 240 != 0
                || s.label.trim().is_empty()
                || s.label.chars().count() > 48
                || s.label.chars().any(char::is_control)
                || s.intensity
                    .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
            {
                return Err("Sections need unique IDs, ordered sixteenth-note positions, labels (1-48 characters), and intensity 0-1".into());
            }
            last = s.tick;
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    Structure,
    Waveform,
    Hybrid,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Address {
    pub deck: usize,
    pub slot: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartMaterial {
    pub thumbnail: Option<Vec<Envelope>>,
    pub deck: usize,
    pub slot: usize,
    pub id: u64,
    pub name: String,
    pub kind: String,
    pub pattern: Option<parts::pattern::Pattern>,
    pub target_pattern: Option<parts::pattern::Pattern>,
    pub pattern_known: bool,
    pub period_ticks: Option<i64>,
    pub density: Option<Vec<f32>>,
    pub energy: Vec<f32>,
    pub peaks: Option<Vec<[[f32; 2]; 2]>>,
    pub boundaries: Vec<f32>,
    pub repetitions: f64,
    pub source: Option<String>,
    pub source_start: u64,
    pub source_end: u64,
    pub source_seconds: f64,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceView {
    pub id: String,
    pub frames: u64,
    pub rate: u32,
    pub peaks: Vec<Envelope>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeckMaterial {
    pub notice: Option<String>,
    pub composition: Option<u64>,
    pub name: String,
    pub length_ticks: i64,
    pub structure: Structure,
    pub parts: Vec<PartMaterial>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Material {
    pub decks: [DeckMaterial; 2],
    pub sources: Vec<SourceView>,
    pub bpm: f64,
    pub columns: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reply {
    pub revision: String,
    pub data: Option<Material>,
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Position {
    pub fraction: f32,
    pub tick: i64,
    pub cursors: Vec<Cursor>,
    pub events: Vec<Event>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Cursor {
    pub slot: usize,
    pub source: Option<f32>,
    pub pattern: Option<f32>,
    pub offset: f32,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Event {
    pub slot: usize,
    pub fraction: f32,
    pub label: String,
    pub projected: bool,
    pub tick: i64,
}

pub fn revision(w: &Workspace, co: Option<&Coordinator>) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    w.project.id.hash(&mut h);
    w.revision.hash(&mut h);
    for m in &w.media {
        m.deck.hash(&mut h);
        m.slot.hash(&mut h);
        m.waveform.hash(&mut h);
        m.prepared_frames.hash(&mut h);
    }
    co.and_then(|c| c.snapshot)
        .and_then(|s| s.bpm)
        .unwrap_or(w.project.settings.bpm)
        .to_bits()
        .hash(&mut h);
    co.is_some().hash(&mut h);
    if let Some(co) = co {
        for d in 0..2 {
            for slot in 0..8 {
                co.snapshot
                    .map(|s| s.decks[d].slots[slot].pattern.revision)
                    .hash(&mut h);
                co.pattern_targets[d][slot].hash(&mut h);
            }
        }
    }
    format!("{:016x}", h.finish())
}
pub fn validate_request(columns: u16, visible: &[Address]) -> Result<(), String> {
    if !(32..=1024).contains(&columns)
        || visible.len() > 16
        || visible.iter().any(|p| p.deck >= 2 || p.slot >= 8)
        || visible.iter().copied().collect::<BTreeSet<_>>().len() != visible.len()
    {
        Err("Overview needs 32-1024 columns and up to sixteen unique part addresses".into())
    } else {
        Ok(())
    }
}
pub fn build(
    w: &Workspace,
    co: Option<&Coordinator>,
    cache: &SharedCache,
    style: Style,
    columns: u16,
    visible: &[Address],
    detail: Option<Address>,
) -> Result<Material, String> {
    validate_request(columns, visible)?;
    if detail.is_some_and(|a| !visible.contains(&a)) {
        return Err("Source detail must be a visible part".into());
    }
    let columns =
        usize::from(columns).min((8192 / visible.len().max(1)).saturating_sub(128).max(32));
    let bpm = co
        .and_then(|c| c.snapshot)
        .and_then(|s| s.bpm)
        .unwrap_or(w.project.settings.bpm);
    let map = TempoMap::new(
        48000,
        &[TempoChange {
            at: BeatTicks(0),
            bpm,
        }],
    )
    .map_err(|e| e.to_string())?;
    let mut sources = Vec::new();
    let mut keys = BTreeSet::new();
    let decks = std::array::from_fn(|deck| {
        let c = w.project.decks[deck].and_then(|id| w.project.composition(id).ok());
        let structure = c.map_or_else(Structure::default, |c| c.structure.clone());
        let length = i64::from(structure.bars) * BAR;
        let mut parts = Vec::new();
        if let Some(c) = c {
            for a in visible.iter().filter(|a| a.deck == deck) {
                let sound = &c.parts[a.slot];
                let info = w.media.iter().find(|m| m.deck == deck && m.slot == a.slot);
                let pyramid = info
                    .and_then(|m| m.waveform.as_ref())
                    .and_then(|key| cache.lock().ok()?.get(key));
                let actual = co
                    .and_then(|co| {
                        co.snapshot
                            .map(|s| (co, s.decks[deck].slots[a.slot].pattern.revision))
                    })
                    .map(|(co, r)| co.pattern_definition(deck, a.slot, r))
                    .unwrap_or(Some(sound.pattern));
                let mut part = project_part(
                    sound,
                    info,
                    pyramid.as_deref().map(|p| p as &dyn EnvelopeSource),
                    actual,
                    PreviewWindow {
                        ticks: length,
                        map: &map,
                        columns,
                        style,
                    },
                );
                part.deck = deck;
                part.slot = a.slot;
                if detail == Some(*a)
                    && let Some(p) = &pyramid
                    && keys.insert(p.id.clone())
                {
                    sources.push(SourceView {
                        id: p.id.clone(),
                        frames: p.frames,
                        rate: p.rate,
                        peaks: p.columns(0, p.frames, columns.min(512)),
                    });
                }
                parts.push(part);
            }
        }
        DeckMaterial {
            notice:w.demo.then(||"Demo source previews unavailable; authored compositions provide analyzed material".into()),
            composition: c.map(|c| c.id),
            name: c.map_or_else(|| "Empty deck".into(), |c| c.name.clone()),
            length_ticks: length,
            structure,
            parts,
        }
    });
    Ok(Material {
        decks,
        sources,
        bpm,
        columns,
    })
}
/// Source windows are canonical 48 kHz frames, independent of nominal beat lengths.
pub fn source_region(sound: &Sound, frames: u64) -> (u64, u64) {
    if sound.looping {
        (
            sound.loop_start,
            sound.loop_end.or(sound.sample_end).unwrap_or(frames),
        )
    } else {
        (sound.sample_start, sound.sample_end.unwrap_or(frames))
    }
}
fn frame(map: &TempoMap, tick: i64) -> u64 {
    map.frame_at(BeatTicks(tick))
        .map_or(0, |f| f.0.max(0) as u64)
}
fn combine(target: &mut Envelope, next: Envelope, weight: f32) {
    for c in 0..2 {
        target.min[c] = target.min[c].min(next.min[c]);
        target.max[c] = target.max[c].max(next.max[c]);
    }
    target.rms = (target.rms * target.rms + next.rms * next.rms * weight).sqrt();
}
pub fn loop_envelope(p: &dyn EnvelopeSource, start: u64, end: u64, a: u64, b: u64) -> Envelope {
    if start >= end || end > p.frames() || a >= b {
        return Envelope::default();
    }
    let length = end - start;
    if b - a >= length {
        return p.envelope(start, end);
    }
    let at = start + a % length;
    let amount = b - a;
    if at + amount <= end {
        p.envelope(at, at + amount)
    } else {
        let mut out = Envelope::default();
        combine(
            &mut out,
            p.envelope(at, end),
            (end - at) as f32 / amount as f32,
        );
        combine(
            &mut out,
            p.envelope(start, start + amount - (end - at)),
            (amount - (end - at)) as f32 / amount as f32,
        );
        out
    }
}
pub struct PreviewWindow<'a> {
    pub ticks: i64,
    pub map: &'a TempoMap,
    pub columns: usize,
    pub style: Style,
}
pub fn project_part(
    sound: &Sound,
    info: Option<&MediaInfo>,
    p: Option<&dyn EnvelopeSource>,
    actual: Option<Option<parts::pattern::Pattern>>,
    window: PreviewWindow<'_>,
) -> PartMaterial {
    let PreviewWindow {
        ticks: length,
        map,
        columns,
        style,
    } = window;
    let columns = columns.clamp(1, 1024);
    let pattern = actual.flatten();
    let frames = p.map_or_else(
        || info.map_or(0, |m| m.prepared_frames as u64),
        |p| p.frames(),
    );
    let (start, end) = source_region(sound, frames);
    let valid = start < end && end <= frames;
    let rate = p.map_or(48000, |p| p.rate());
    let source_frame = |tick| {
        (u128::from(frame(map, tick)) * u128::from(rate) / u128::from(map.sample_rate())) as u64
    };
    let length_frames = source_frame(length).max(1);
    let duration = end.saturating_sub(start);
    let period = pattern.map(|p| i64::from(p.bars) * BAR);
    let boundaries = if let Some(period) = period {
        (0..=length / period)
            .map(|i| (i * period) as f32 / length as f32)
            .collect()
    } else if sound.looping && valid {
        (0..=length_frames.div_ceil(duration).min(128))
            .map(|i| (i * duration) as f32 / length_frames as f32)
            .filter(|v| *v <= 1.0)
            .collect()
    } else {
        vec![0.0]
    };
    // Monophonic hit segments stop at the next real step trigger, not sample summation.
    let mut segments = Vec::new();
    if let Some(pattern) = pattern {
        for step in 0..length / 240 {
            let velocity = pattern.velocity((step % (i64::from(pattern.bars) * 16)) as usize);
            if velocity > 0 {
                segments.push((source_frame(step * 240), velocity));
            }
        }
    } else if !sound.looping {
        segments.push((0, 127));
    }
    let mut energy = Vec::with_capacity(columns);
    let mut peaks = Vec::with_capacity(if style == Style::Structure {
        0
    } else {
        columns
    });
    for i in 0..columns {
        let a = source_frame(length * i as i64 / columns as i64);
        let b = source_frame(length * (i + 1) as i64 / columns as i64).max(a + 1);
        let mut out = Envelope::default();
        if valid && let Some(p) = p {
            if pattern.is_none() && sound.looping {
                out = loop_envelope(p, start, end, a, b);
            } else {
                // Only the latest trigger at/before this column can still sound:
                // earlier voices were cut by retrigger. Do not rescan long tails.
                let first = segments
                    .partition_point(|(at, _)| *at <= a)
                    .saturating_sub(1);
                for (index, (at, velocity)) in segments
                    .iter()
                    .enumerate()
                    .skip(first)
                    .take_while(|(_, s)| s.0 < b)
                {
                    let until = segments
                        .get(index + 1)
                        .map_or(at.saturating_add(duration), |n| {
                            n.0.min(at.saturating_add(duration))
                        });
                    let left = a.max(*at);
                    let right = b.min(until);
                    if left >= right {
                        continue;
                    }
                    let mut hit = p.envelope(start + left - at, start + right - at);
                    let gain = f32::from(*velocity) / 127.0;
                    for ch in 0..2 {
                        hit.min[ch] *= gain;
                        hit.max[ch] *= gain;
                    }
                    hit.rms *= gain;
                    combine(&mut out, hit, (right - left) as f32 / (b - a) as f32);
                }
            }
        }
        energy.push(out.rms);
        if style != Style::Structure {
            peaks.push([out.min, out.max]);
        }
    }
    let density = pattern.map(|p| {
        (0..length / BAR)
            .map(|bar| {
                p.steps[bar as usize % usize::from(p.bars)]
                    .iter()
                    .filter(|v| **v > 0)
                    .count() as f32
                    / 16.0
            })
            .collect()
    });
    let error = info.and_then(|m| m.error.clone()).or_else(|| {
        if sound.media.is_none() && sound.factory.is_none() {
            Some("No sound assigned".into())
        } else if p.is_none() {
            Some("Source preview unavailable".into())
        } else if !valid {
            Some("Invalid source region".into())
        } else {
            None
        }
    });
    PartMaterial {
        thumbnail: p.filter(|_| valid).map(|p| {
            (0..columns.min(128))
                .map(|i| {
                    let count = columns.min(128) as u64;
                    let a = start + duration * i as u64 / count;
                    let b = start + duration * (i + 1) as u64 / count;
                    p.envelope(a, b.max(a + 1).min(end))
                })
                .collect()
        }),
        deck: 0,
        slot: 0,
        id: sound.id,
        name: sound.name.clone(),
        kind: if pattern.is_some() {
            "pattern"
        } else if sound.looping {
            "loop"
        } else {
            "one_shot"
        }
        .into(),
        pattern,
        target_pattern: if actual != Some(sound.pattern) {
            sound.pattern
        } else {
            None
        },
        pattern_known: actual.is_some(),
        period_ticks: period,
        density,
        energy,
        peaks: (style != Style::Structure).then_some(peaks),
        boundaries,
        repetitions: if let Some(period) = period {
            length as f64 / period as f64
        } else if sound.looping && valid {
            length_frames as f64 / duration as f64
        } else {
            1.0
        },
        source: p.map(|p| p.identity().to_owned()),
        source_start: start,
        source_end: end,
        source_seconds: duration as f64 / f64::from(rate),
        error,
    }
}
pub fn positions(w: &Workspace, co: &Coordinator) -> Option<[Position; 2]> {
    let project = &w.project;
    let s = co.snapshot?;
    Some(std::array::from_fn(|d| {
        let c = project.decks[d].and_then(|id| project.composition(id).ok());
        let length = c.map_or(8 * BAR, |c| i64::from(c.structure.bars) * BAR);
        let epoch = s.decks[d].epoch.0;
        let start = epoch + (s.beat.0 - epoch).div_euclid(length) * length;
        let tick = (s.beat.0 - epoch).rem_euclid(length);
        let fraction = tick as f32 / length as f32;
        let window = crate::timeline::Window {
            start: BeatTicks(start),
            end: BeatTicks(start + length),
            phrase_start: BeatTicks(start),
            phrase_length: BeatTicks(length),
        };
        let mut markers = crate::timeline::markers(&s, window);
        markers.retain(|m| m.deck == d);
        // Confirmed/manual deadlines must survive dense conditional rule projections.
        markers.sort_by_key(|m| (m.projected, !m.manual, m.at));
        let mut events: Vec<Event> = markers
            .into_iter()
            .take(48)
            .map(|m| Event {
                slot: m.slot,
                fraction: window.fraction(m.at),
                label: crate::timeline::marker_label(&[m]),
                projected: m.projected,
                tick: m.at.0,
            })
            .collect();
        for slot in 0..8 {
            let p = s.decks[d].slots[slot].pattern;
            if p.pending_revision.is_some()
                && let Some(at) = p.pending_at
                && at.0 >= start
                && at.0 <= start + length
            {
                events.push(Event {
                    slot,
                    fraction: window.fraction(at),
                    label: "Edit".into(),
                    projected: false,
                    tick: at.0,
                });
            }
        }
        let cursors = (0..8)
            .filter_map(|slot| {
                let live = s.decks[d].slots[slot];
                live.active?;
                let pattern = live
                    .pattern
                    .anchor
                    .zip(live.pattern.bars)
                    .filter(|_| live.pattern.running)
                    .map(|(a, b)| {
                        (s.beat.0 - a.0).rem_euclid(i64::from(b) * BAR) as f32
                            / (f32::from(b) * BAR as f32)
                    });
                let offset = live
                    .pattern
                    .anchor
                    .zip(live.pattern.bars)
                    .map_or(0.0, |(a, b)| {
                        (a.0 - epoch).rem_euclid(i64::from(b) * BAR) as f32 / length as f32
                    });
                let source = c.and_then(|c| {
                    let p = &c.parts[slot];
                    let frames = w
                        .media
                        .iter()
                        .find(|m| m.deck == d && m.slot == slot)
                        .map_or(0, |m| m.prepared_frames as u64);
                    let (a, b) = source_region(p, frames);
                    if b > a {
                        // Publication position is elapsed voice frames, not an absolute
                        // asset cursor. Map the immutable prepared region at native 1x.
                        live.position.map(|v| {
                            let offset = if p.looping { v % (b - a) } else { v.min(b - a) };
                            offset as f32 / (b - a) as f32
                        })
                    } else {
                        None
                    }
                });
                Some(Cursor {
                    slot,
                    source,
                    pattern,
                    offset,
                })
            })
            .collect();
        Position {
            fraction,
            tick,
            cursors,
            events,
        }
    }))
}
