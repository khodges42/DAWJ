// SPDX-License-Identifier: GPL-3.0-or-later
//! Small local jam document. File I/O and preparation are non-realtime operations.
use crate::{
    Coordinator, Intent, Mode,
    ipc::{Grid, Rule},
};
use audio_backend::{
    live::{LiveControl, LiveEngine},
    mixer::MixerControl,
    part_dsp::PartParameters,
    samples::{BoundaryPolicy, LoadedSample, SampleVoice},
};
use parts::{AssetId, Part, PartId, PlaybackConfiguration, PlaybackMode, SLOTS, TimingPolicy};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use timebase::{BeatTicks, TempoChange, TempoMap};
pub const VERSION: u16 = 3;
pub const MAX_DOCUMENT: usize = 512 * 1024;
pub(crate) fn id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
        * 1024;
    NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
        Some(now.max(old.saturating_add(1)))
    })
    .unwrap()
    .saturating_add(1)
    .max(now)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sound {
    #[serde(default)]
    pub variations: [Option<parts::pattern::Pattern>; 4],
    #[serde(default)]
    pub variation: Option<u8>,
    #[serde(default)]
    pub factory: Option<crate::factory::Reference>,
    #[serde(default)]
    pub hints: Option<crate::factory::Hints>,
    #[serde(default)]
    pub pattern: Option<parts::pattern::Pattern>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub sample_start: u64,
    #[serde(default)]
    pub sample_end: Option<u64>,
    pub id: u64,
    pub name: String,
    pub media: Option<String>,
    pub looping: bool,
    pub loop_start: u64,
    pub loop_end: Option<u64>,
    pub nominal_ticks: i64,
    pub quantum_ticks: i64,
    pub gain: f32,
    pub cutoff_hz: Option<f32>,
    pub rule: Option<Rule>,
}
impl Sound {
    pub fn variation(&mut self, index: usize, store: bool) -> Result<(), String> {
        if index >= 4 {
            return Err("Variation must be 1..4".into());
        }
        if store {
            self.variations[index] = Some(self.pattern.ok_or("Create a pattern first")?);
        } else {
            self.pattern =
                Some(self.variations[index].ok_or("Store a pattern in this variation first")?);
        }
        self.variation = Some(index as u8);
        Ok(())
    }
    pub(crate) fn empty(slot: usize) -> Self {
        Self {
            variations: [None; 4],
            variation: None,
            factory: None,
            hints: None,
            pattern: None,
            color: None,
            sample_start: 0,
            sample_end: None,
            id: id(),
            name: format!("Part {}", slot + 1),
            media: None,
            looping: true,
            loop_start: 0,
            loop_end: None,
            nominal_ticks: 3840,
            quantum_ticks: 960,
            gain: 1.0,
            cutoff_hz: None,
            rule: None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Composition {
    #[serde(default)]
    pub structure: crate::visualization::Structure,
    pub id: u64,
    pub name: String,
    pub parts: [Sound; SLOTS],
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartTarget {
    pub part: u64,
    pub gain: f32,
    pub cutoff_hz: Option<f32>,
    #[serde(default)]
    pub rule: Option<Rule>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(default)]
    pub part_targets: [[Option<PartTarget>; SLOTS]; 2],
    pub bpm: f64,
    pub phrase_bars: u8,
    pub crossfader: f32,
    pub deck_gain: [f32; 2],
    pub cue: [bool; 2],
    pub output_trim_db: f32,
    pub quantization: Grid,
    pub mode: Mode,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            part_targets: [[None; SLOTS]; 2],
            bpm: 120.0,
            phrase_bars: 8,
            crossfader: 0.5,
            deck_gain: [1.0; 2],
            cue: [false; 2],
            output_trim_db: 24.0,
            quantization: Grid::Beat,
            mode: Mode::Volume,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    #[serde(default)]
    pub sample_overrides: std::collections::BTreeMap<String, crate::factory::Hints>,
    pub version: u16,
    pub id: u64,
    pub name: String,
    pub compositions: Vec<Composition>,
    pub decks: [Option<u64>; 2],
    pub settings: Settings,
}
impl Default for Project {
    fn default() -> Self {
        Self {
            sample_overrides: Default::default(),
            version: VERSION,
            id: id(),
            name: "Untitled jam".into(),
            compositions: Vec::new(),
            decks: [None; 2],
            settings: Settings::default(),
        }
    }
}
fn name(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 120 && !s.chars().any(char::is_control)
}
fn validate_rule(rule: Option<Rule>) -> Result<(), String> {
    match rule {
        Some(Rule::Every { n, .. }) if !(1..=1024).contains(&n) => {
            Err("Rule period outside 1..=1024".into())
        }
        Some(Rule::PhraseBar {
            bar, phrase_bars, ..
        }) if bar == 0 || bar > phrase_bars || ![4, 8].contains(&phrase_bars) => {
            Err("Invalid phrase rule".into())
        }
        _ => Ok(()),
    }
}
impl Project {
    pub fn composition(&self, id: u64) -> Result<&Composition, String> {
        self.compositions
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| "Unknown composition".into())
    }
    pub fn composition_mut(&mut self, id: u64) -> Result<&mut Composition, String> {
        self.compositions
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or_else(|| "Unknown composition".into())
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != VERSION {
            return Err("Unsupported local session version".into());
        }
        if !name(&self.name) || self.compositions.len() > 8 {
            return Err("Invalid session name or more than eight compositions".into());
        }
        if self.sample_overrides.len() > 128 {
            return Err("At most 128 metadata corrections per session".into());
        };
        for (k, v) in &self.sample_overrides {
            if k.len() > 1200 {
                return Err("Metadata reference too long".into());
            }
            v.validate()?;
        }
        let mut ids = std::collections::HashSet::new();
        for n in std::iter::once(self.id).chain(
            self.compositions
                .iter()
                .flat_map(|c| std::iter::once(c.id).chain(c.parts.iter().map(|p| p.id))),
        ) {
            if n == 0 || n > 9_007_199_254_740_991 || !ids.insert(n) {
                return Err("Invalid or duplicate stable identity".into());
            }
        }
        for c in &self.compositions {
            c.structure.validate()?;
            if !name(&c.name) {
                return Err("Invalid composition name".into());
            }
            for p in &c.parts {
                for variation in p.variations.iter().flatten() {
                    variation.validate().map_err(str::to_string)?;
                }
                if p.variation
                    .is_some_and(|i| i >= 4 || p.variations[usize::from(i)] != p.pattern)
                {
                    return Err("Invalid selected pattern variation".into());
                }
                if let Some(r) = &p.factory {
                    r.validate()?;
                    if p.media.is_some() {
                        return Err("Factory and user media references must stay distinct".into());
                    }
                }
                if let Some(h) = &p.hints {
                    h.validate()?;
                }
                if let Some(pattern) = p.pattern {
                    pattern.validate().map_err(str::to_string)?;
                    if p.looping {
                        return Err("Step patterns trigger one-shots; turn Loop off first".into());
                    }
                }
                if p.color.as_ref().is_some_and(|c| {
                    c.len() != 7
                        || !c.starts_with('#')
                        || !c[1..].chars().all(|x| x.is_ascii_hexdigit())
                }) || p.sample_end.is_some_and(|e| e <= p.sample_start)
                {
                    return Err("Invalid color or sample region".into());
                }
                if !name(&p.name)
                    || p.media
                        .as_ref()
                        .is_some_and(|s| s.is_empty() || s.len() > 4096 || s.contains('\0'))
                    || p.nominal_ticks <= 0
                    || p.quantum_ticks <= 0
                    || p.nominal_ticks > 960 * 4 * 1024
                    || p.quantum_ticks > 960 * 4 * 1024
                    || p.loop_end.is_some_and(|e| e <= p.loop_start)
                {
                    return Err("Invalid part name, path, timing or loop bounds".into());
                }
                if !p.gain.is_finite()
                    || !(0.0..=1.0).contains(&p.gain)
                    || p.cutoff_hz
                        .is_some_and(|v| !v.is_finite() || !(20.0..=20000.0).contains(&v))
                {
                    return Err("Invalid part gain or cutoff".into());
                }
                validate_rule(p.rule)?;
            }
        }
        for n in self.decks.into_iter().flatten() {
            self.composition(n)?;
        }
        let s = &self.settings;
        for p in s.part_targets.iter().flatten().flatten() {
            if p.part == 0
                || p.part > 9_007_199_254_740_991
                || !p.gain.is_finite()
                || !(0.0..=1.0).contains(&p.gain)
                || p.cutoff_hz
                    .is_some_and(|v| !v.is_finite() || !(20.0..=20000.0).contains(&v))
            {
                return Err("Invalid saved part target".into());
            }
            validate_rule(p.rule)?;
        }
        if !s.bpm.is_finite()
            || !(1.0..=1000.0).contains(&s.bpm)
            || ![4, 8].contains(&s.phrase_bars)
            || !s.crossfader.is_finite()
            || !(0.0..=1.0).contains(&s.crossfader)
            || s.deck_gain
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || !s.output_trim_db.is_finite()
            || !(-24.0..=36.0).contains(&s.output_trim_db)
        {
            return Err("Invalid session mixer, tempo or phrase settings".into());
        }
        Ok(())
    }
    pub fn create(&mut self, title: String, deck: usize) -> Result<u64, String> {
        if deck >= 2 || self.compositions.len() >= 8 || !name(&title) {
            return Err("Choose deck A/B, a name and at most eight compositions".into());
        }
        let n = id();
        self.compositions.push(Composition {
            structure: Default::default(),
            id: n,
            name: title,
            parts: std::array::from_fn(Sound::empty),
        });
        self.decks[deck] = Some(n);
        Ok(n)
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut b = Vec::new();
        file.take(MAX_DOCUMENT as u64 + 1)
            .read_to_end(&mut b)
            .map_err(|e| e.to_string())?;
        if b.len() > MAX_DOCUMENT {
            return Err("Session exceeds 512 KiB limit".into());
        }
        let mut p: Self =
            serde_json::from_slice(&b).map_err(|e| format!("Invalid session JSON: {e}"))?;
        if (1..VERSION).contains(&p.version) {
            p.version = VERSION;
        }
        p.validate()?;
        let base = absolute(path)?
            .parent()
            .ok_or("Session has no parent")?
            .to_path_buf();
        for c in &mut p.compositions {
            for s in &mut c.parts {
                if let Some(m) = &mut s.media {
                    *m = resolve(&base, m).display().to_string();
                }
            }
        }
        Ok(p)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let path = absolute(path)?;
        let base = path.parent().ok_or("Session has no parent")?;
        let mut disk = self.clone();
        for c in &mut disk.compositions {
            for s in &mut c.parts {
                if let Some(m) = &mut s.media {
                    let p = Path::new(m);
                    if let Ok(relative) = p.strip_prefix(base) {
                        *m = relative.display().to_string()
                    }
                }
            }
        }
        let bytes = serde_json::to_vec_pretty(&disk).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_DOCUMENT {
            return Err("Session exceeds 512 KiB limit".into());
        }
        let temp = base.join(format!(".dawj-{}-{}.tmp", std::process::id(), id()));
        let result = (|| {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(|e| e.to_string())?;
            f.write_all(&bytes)
                .and_then(|_| f.sync_all())
                .map_err(|e| e.to_string())?;
            drop(f);
            std::fs::rename(&temp, &path).map_err(|e| e.to_string())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result
    }
}
pub fn absolute(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("Empty file path".into());
    }
    Ok(if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    })
}
fn resolve(base: &Path, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaInfo {
    #[serde(default)]
    pub waveform: Option<String>,
    pub deck: usize,
    pub slot: usize,
    pub path: String,
    pub filename: String,
    pub duration: f64,
    pub sample_rate: u32,
    pub channels: u16,
    pub prepared_frames: usize,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Workspace {
    pub project: Project,
    pub path: Option<String>,
    pub dirty: bool,
    pub revision: u64,
    pub demo: bool,
    pub busy: bool,
    pub message: String,
    pub media: Vec<MediaInfo>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Structure {
        composition: u64,
        structure: crate::visualization::Structure,
    },
    Variation {
        composition: u64,
        slot: usize,
        index: usize,
        store: bool,
    },
    Pattern {
        composition: u64,
        slot: usize,
        pattern: Option<parts::pattern::Pattern>,
    },
    Factory {
        composition: u64,
        slot: usize,
        reference: crate::factory::Reference,
    },
    Correct {
        reference: crate::factory::Reference,
        hints: crate::factory::Hints,
    },
    Duplicate {
        composition: u64,
        deck: usize,
        name: String,
    },
    ExportComposition {
        composition: u64,
        path: String,
    },
    OpenComposition {
        deck: usize,
        path: String,
    },
    Undo {},
    Redo {},
    New {},
    Open {
        path: String,
    },
    Save {
        path: Option<String>,
    },
    Rename {
        name: String,
    },
    Create {
        name: String,
        deck: usize,
    },
    RenameComposition {
        composition: u64,
        name: String,
    },
    Assign {
        deck: usize,
        composition: Option<u64>,
    },
    Import {
        composition: u64,
        slot: usize,
        path: String,
    },
    Remove {
        composition: u64,
        slot: usize,
    },
    Edit {
        composition: u64,
        slot: usize,
        part: Box<Sound>,
    },
    Demo {},
}
pub struct Prepared {
    pub changed: [[bool; SLOTS]; 2],
    pub voices: Vec<SampleVoice>,
    pub addresses: Vec<(usize, usize)>,
    pub media: Vec<MediaInfo>,
}
pub fn prepare(project: &Project, decks: [bool; 2], strict: bool) -> Result<Prepared, String> {
    prepare_mask(
        project,
        std::array::from_fn(|d| [decks[d]; SLOTS]),
        strict,
        None,
        None,
    )
}
/// Decode only new voice definitions. Names/knobs/rules never duplicate resident PCM.
pub fn prepare_changes(
    project: &Project,
    before: &Project,
    decks: [bool; 2],
    strict: bool,
) -> Result<Prepared, String> {
    prepare_mask(
        project,
        change_mask(project, before, decks),
        strict,
        None,
        None,
    )
}
pub fn change_mask(project: &Project, before: &Project, decks: [bool; 2]) -> [[bool; SLOTS]; 2] {
    std::array::from_fn(|d| {
        std::array::from_fn(|s| {
            if !decks[d] {
                return false;
            }
            let get = |p: &Project| {
                p.decks[d]
                    .and_then(|n| p.composition(n).ok())
                    .map(|c| c.parts[s].clone())
            };
            match (get(before), get(project)) {
                (Some(a), Some(b)) => {
                    a.id != b.id
                        || a.media != b.media
                        || a.factory != b.factory
                        || a.sample_start != b.sample_start
                        || a.sample_end != b.sample_end
                        || a.looping != b.looping
                        || a.loop_start != b.loop_start
                        || a.loop_end != b.loop_end
                        || a.nominal_ticks != b.nominal_ticks
                        || a.quantum_ticks != b.quantum_ticks
                }
                (None, None) => false,
                _ => true,
            }
        })
    })
}
pub fn prepare_catalog(
    project: &Project,
    before: Option<&Project>,
    decks: [bool; 2],
    strict: bool,
    catalog: &crate::factory::Catalog,
) -> Result<Prepared, String> {
    prepare_mask(
        project,
        before.map_or_else(
            || std::array::from_fn(|d| [decks[d]; SLOTS]),
            |b| change_mask(project, b, decks),
        ),
        strict,
        Some(catalog),
        None,
    )
}
pub fn prepare_catalog_cached(
    project: &Project,
    before: Option<&Project>,
    decks: [bool; 2],
    strict: bool,
    catalog: &crate::factory::Catalog,
    cache: &crate::waveform::SharedCache,
) -> Result<Prepared, String> {
    prepare_mask(
        project,
        before.map_or_else(
            || std::array::from_fn(|d| [decks[d]; SLOTS]),
            |b| change_mask(project, b, decks),
        ),
        strict,
        Some(catalog),
        Some(cache),
    )
}
fn prepare_mask(
    project: &Project,
    changed: [[bool; SLOTS]; 2],
    strict: bool,
    catalog: Option<&crate::factory::Catalog>,
    cache: Option<&crate::waveform::SharedCache>,
) -> Result<Prepared, String> {
    project.validate()?;
    let mut result = Prepared {
        changed,
        voices: Vec::new(),
        addresses: Vec::new(),
        media: Vec::new(),
    };
    for (d, mask) in changed.into_iter().enumerate() {
        if !mask.into_iter().any(|v| v) {
            continue;
        }
        let Some(c) = project.decks[d]
            .map(|n| project.composition(n))
            .transpose()?
        else {
            continue;
        };
        for (slot, p) in c.parts.iter().enumerate() {
            if !mask[slot] {
                continue;
            }
            if p.media.is_none() && p.factory.is_none() {
                continue;
            }
            let source = if let Some(r) = &p.factory {
                catalog
                    .ok_or_else(|| format!("Factory resources unavailable: {}", r.key()))
                    .and_then(|c| c.resolve(r))
            } else {
                Ok(PathBuf::from(p.media.as_deref().unwrap_or_default()))
            };
            let path = source.as_ref().map_or_else(
                |_| {
                    p.factory
                        .as_ref()
                        .map_or(String::new(), |r| format!("factory:{}", r.key()))
                },
                |p| p.display().to_string(),
            );
            let mut info = MediaInfo {
                waveform: None,
                deck: d,
                slot,
                path: path.clone(),
                filename: Path::new(&path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                duration: 0.0,
                sample_rate: 0,
                channels: 0,
                prepared_frames: 0,
                error: None,
            };
            let prepared = (|| {
                let asset = AssetId(id());
                let resolved = source.map_err(|e| e.to_string())?;
                let (sample, metadata) = LoadedSample::prepare_file(asset, &resolved, 48000)?;
                info.duration = metadata.frames as f64 / f64::from(metadata.sample_rate);
                info.sample_rate = metadata.sample_rate;
                info.channels = metadata.channels;
                info.prepared_frames = metadata.prepared_frames;
                if let Some(cache) = cache {
                    let key = crate::waveform::Pyramid::identity(sample.interleaved(), 2, 48000);
                    let found = cache
                        .lock()
                        .map_err(|_| "Waveform cache unavailable")?
                        .get(&key)
                        .is_some();
                    if !found {
                        let peaks =
                            crate::waveform::Pyramid::analyze(sample.interleaved(), 2, 48000)
                                .map_err(str::to_string)?;
                        cache
                            .lock()
                            .map_err(|_| "Waveform cache unavailable")?
                            .insert(peaks);
                    }
                    info.waveform = Some(key);
                }
                let part = Part {
                    id: PartId(p.id),
                    playback: PlaybackConfiguration {
                        asset,
                        mode: if p.looping {
                            PlaybackMode::Loop {
                                start: p.loop_start,
                                end: p
                                    .loop_end
                                    .or(p.sample_end)
                                    .unwrap_or(metadata.prepared_frames as u64),
                            }
                        } else {
                            PlaybackMode::OneShot
                        },
                    },
                    timing: TimingPolicy {
                        nominal_length: BeatTicks(p.nominal_ticks),
                        quantum: BeatTicks(p.quantum_ticks),
                    },
                };
                SampleVoice::prepare_region(
                    part,
                    sample,
                    48000,
                    BoundaryPolicy::Fade { frames: 64 },
                    p.sample_start,
                    p.sample_end,
                )
                .map_err(str::to_string)
            })();
            match prepared {
                Ok(v) => {
                    result.addresses.push((d, slot));
                    if result
                        .voices
                        .iter()
                        .map(SampleVoice::resident_frames)
                        .sum::<usize>()
                        + v.resident_frames()
                        > 2 * audio_backend::samples::MAX_DECK_FRAMES
                    {
                        return Err("Session PCM exceeds 256 MiB preparation budget".into());
                    }
                    result.voices.push(v)
                }
                Err(e) => {
                    if strict {
                        return Err(format!("{}: {e}", info.filename));
                    }
                    info.error = Some(e)
                }
            }
            result.media.push(info);
        }
    }
    Ok(result)
}
pub fn engine(project: &Project) -> Result<(LiveEngine, LiveControl, MixerControl), String> {
    let map = TempoMap::new(
        48000,
        &[TempoChange {
            at: BeatTicks(0),
            bpm: project.settings.bpm,
        }],
    )
    .map_err(|e| format!("Tempo: {e:?}"))?;
    let scheduler = event_engine::live::MusicalScheduler::new(
        map,
        [BeatTicks(0); 2],
        [BeatTicks(i64::from(project.settings.phrase_bars) * 3840); 2],
    )
    .map_err(str::to_string)?;
    LiveEngine::empty(scheduler).map_err(str::to_string)
}
pub fn install(
    co: &mut Coordinator,
    project: &Project,
    prepared: Prepared,
    decks: [bool; 2],
) -> Result<Vec<MediaInfo>, String> {
    if decks
        .into_iter()
        .enumerate()
        .any(|(d, a)| a && !co.deck_editable(d))
    {
        return Err("Destination deck is active, pending or has installed rules".into());
    }
    let mut patterns = Vec::new();
    for (d, affected) in decks.into_iter().enumerate() {
        if affected {
            for slot in 0..SLOTS {
                let next = project.decks[d]
                    .and_then(|n| project.composition(n).ok())
                    .and_then(|c| c.parts[slot].pattern);
                if next.is_some()
                    || co
                        .snapshot
                        .is_some_and(|s| s.decks[d].slots[slot].pattern.bars.is_some())
                {
                    patterns.push((d, slot, next));
                }
            }
        }
    }
    co.check_pattern_capacity(patterns.len())
        .map_err(str::to_string)?;
    let indices = co.install_voices(prepared.voices).map_err(str::to_string)?;
    for (d, affected) in decks.into_iter().enumerate() {
        if affected {
            for s in 0..SLOTS {
                if prepared.changed[d][s] {
                    co.banks[d][s] = None;
                }
            }
            co.names[d] = std::array::from_fn(|s| {
                project.decks[d]
                    .and_then(|n| project.composition(n).ok())
                    .map_or_else(|| format!("Part {}", s + 1), |c| c.parts[s].name.clone())
            });
        }
    }
    for ((d, s), bank) in prepared.addresses.into_iter().zip(indices) {
        co.banks[d][s] = Some(bank);
    }
    for (d, affected) in decks.into_iter().enumerate() {
        if !affected {
            continue;
        }
        for slot in 0..SLOTS {
            let p = project.decks[d]
                .and_then(|n| project.composition(n).ok())
                .map(|c| &c.parts[slot]);
            let target = project.settings.part_targets[d][slot]
                .filter(|t| p.is_some_and(|p| t.part == p.id));
            co.queued_parameters[d][slot] = Some(PartParameters {
                gain: target.map_or_else(|| p.map_or(1.0, |p| p.gain), |t| t.gain),
                cutoff_hz: target.map_or_else(|| p.and_then(|p| p.cutoff_hz), |t| t.cutoff_hz),
                muted: false,
            });
        }
    }
    co.set_patterns(&patterns).map_err(str::to_string)?;
    Ok(prepared.media)
}
pub fn configure(co: &mut Coordinator, s: &Settings) -> Result<(), String> {
    for i in [
        Intent::Mode(s.mode),
        Intent::Quantize(s.quantization.into()),
        Intent::Crossfader(s.crossfader),
        Intent::DeckGain(0, s.deck_gain[0]),
        Intent::DeckGain(1, s.deck_gain[1]),
        Intent::OutputTrim(s.output_trim_db),
    ] {
        co.dispatch(i, crate::InputSource::Gui)
            .map_err(str::to_string)?;
    }
    for d in 0..2 {
        if s.cue[d] && co.cue_available {
            co.dispatch(Intent::Cue(d), crate::InputSource::Gui)
                .map_err(str::to_string)?;
        }
    }
    Ok(())
}
impl Workspace {
    /// Bounded comparison also notices MIDI changes without serializing the document.
    pub fn changed(&self, co: &Coordinator) -> bool {
        let s = &self.project.settings;
        if s.crossfader != co.parameters.crossfader
            || s.deck_gain != [co.parameters.gain_a, co.parameters.gain_b]
            || s.output_trim_db != co.output_trim_db
            || s.cue != co.cues
            || s.mode != co.persistent_mode
            || audio_backend::live::Quantize::from(s.quantization) != co.focus.quantization
            || co.snapshot.and_then(|p| p.bpm).is_some_and(|b| b != s.bpm)
        {
            return true;
        }
        for d in 0..2 {
            if let Some(n) = self.project.decks[d]
                && let Ok(c) = self.project.composition(n)
            {
                for (slot, p) in c.parts.iter().enumerate() {
                    let target = s.part_targets[d][slot].filter(|t| t.part == p.id);
                    if let Some(actual) = co.parameter_target(d, slot) {
                        let gain = target.map_or(p.gain, |t| t.gain);
                        let cutoff = target.map_or(p.cutoff_hz, |t| t.cutoff_hz);
                        if (gain - actual.gain).abs() > 0.0001
                            || match (cutoff, actual.cutoff_hz) {
                                (Some(a), Some(b)) => (a - b).abs() > 1.0,
                                (None, None) => false,
                                _ => true,
                            }
                        {
                            return true;
                        }
                    }
                    if let Some(rule) = co.snapshot.and_then(|s| s.decks[d].slots[slot].rule) {
                        let saved = target
                            .map_or(p.rule, |t| t.rule)
                            .map(audio_backend::live::TriggerRule::from);
                        if saved != Some(rule.rule) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    pub fn capture(&mut self, co: &Coordinator) {
        let s = &mut self.project.settings;
        s.crossfader = co.parameters.crossfader;
        s.deck_gain = [co.parameters.gain_a, co.parameters.gain_b];
        s.cue = co.cues;
        s.output_trim_db = co.output_trim_db;
        s.mode = co.persistent_mode;
        s.quantization = co.focus.quantization.into();
        if let Some(state) = co.snapshot {
            if let Some(b) = state.bpm {
                s.bpm = b;
            }
            for d in 0..2 {
                if let Some(n) = self.project.decks[d]
                    && let Some(c) = self.project.compositions.iter_mut().find(|c| c.id == n)
                {
                    for (slot, p) in c.parts.iter_mut().enumerate() {
                        if let Some(params) = co.parameter_target(d, slot) {
                            p.gain = params.gain;
                            p.cutoff_hz = params.cutoff_hz;
                            let rule = state.decks[d].slots[slot]
                                .rule
                                .map(|r| r.rule.into())
                                .or_else(|| {
                                    self.project.settings.part_targets[d][slot]
                                        .filter(|t| t.part == p.id)
                                        .map_or(p.rule, |t| t.rule)
                                });
                            self.project.settings.part_targets[d][slot] = Some(PartTarget {
                                part: p.id,
                                gain: params.gain,
                                cutoff_hz: params.cutoff_hz,
                                rule,
                            });
                        }
                        if let Some(r) = state.decks[d].slots[slot].rule {
                            p.rule = Some(r.rule.into());
                        }
                    }
                }
            }
        }
    }
}
