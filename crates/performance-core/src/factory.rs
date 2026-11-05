// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded, read-only factory discovery. All calls run outside audio/WebView threads.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    io::{Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_id: Option<String>,
    pub pack: String,
    pub path: String,
}
impl Reference {
    pub fn validate(&self) -> Result<(), String> {
        if self.pack.is_empty()
            || self.sample_id.as_ref().is_some_and(|id| {
                id.len() > 120
                    || id.is_empty()
                    || id.chars().any(|c| c.is_control() || "/\\:".contains(c))
            })
            || self.pack.len() > 120
            || self
                .pack
                .chars()
                .any(|c| c.is_control() || "/\\:".contains(c))
            || self.path.is_empty()
            || self.path.len() > 1024
            || self.path.contains('\\')
            || Path::new(&self.path)
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err("Invalid factory-relative reference".into());
        }
        Ok(())
    }
    pub fn key(&self) -> String {
        format!("{}/{}", self.pack, self.path)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hints {
    pub identifier: Option<String>,
    pub display_name: String,
    pub kind: String,
    pub note: Option<String>,
    pub key: Option<String>,
    pub bpm: Option<f64>,
}
impl Hints {
    pub fn validate(&self) -> Result<(), String> {
        if self.display_name.len() > 120
            || !["Unknown", "OneShot", "Loop"].contains(&self.kind.as_str())
            || self.identifier.as_ref().is_some_and(|s| s.len() > 64)
            || self.note.as_ref().is_some_and(|s| s.len() > 24)
            || self.key.as_ref().is_some_and(|s| s.len() > 24)
            || self
                .bpm
                .is_some_and(|b| !b.is_finite() || !(1.0..=1000.0).contains(&b))
        {
            Err("Invalid sample metadata correction".into())
        } else {
            Ok(())
        }
    }
}
fn note(token: &str) -> bool {
    let mut c = token.chars();
    if !matches!(c.next(), Some('A'..='G' | 'a'..='g')) {
        return false;
    }
    let rest = c.as_str();
    let rest = rest
        .strip_prefix('#')
        .or_else(|| rest.strip_prefix('b'))
        .unwrap_or(rest);
    rest.is_empty() || rest.parse::<i8>().is_ok_and(|n| (-2..=9).contains(&n))
}
pub fn hints(filename: &str, category: &str) -> Hints {
    let stem = Path::new(filename)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let tokens: Vec<_> = stem.split(['_', ' ']).filter(|s| !s.is_empty()).collect();
    let looped = tokens
        .iter()
        .any(|s| s.eq_ignore_ascii_case("loop") || s.eq_ignore_ascii_case("loops"))
        || category.split('/').any(|s| s.eq_ignore_ascii_case("loops"));
    let identifier = tokens
        .first()
        .filter(|s| {
            let letters = s.chars().take_while(|c| c.is_ascii_alphabetic()).count();
            letters > 0
                && letters <= 4
                && s.len() > letters
                && s[letters..].chars().all(|c| c.is_ascii_digit())
        })
        .map(|s| s.to_string());
    let mut bpm = None;
    let mut bpm_index = None;
    for (i, t) in tokens.iter().enumerate() {
        let lower = t.to_ascii_lowercase();
        let explicit = lower
            .strip_suffix("bpm")
            .or_else(|| lower.strip_prefix("bpm"));
        let candidate = explicit.and_then(|s| s.parse::<f64>().ok()).or_else(|| {
            if looped && i > 0 && tokens[i - 1].eq_ignore_ascii_case("loop") {
                t.parse().ok()
            } else {
                None
            }
        });
        if candidate.is_some_and(|b| (1.0..=1000.0).contains(&b)) {
            bpm = candidate;
            bpm_index = Some(i);
            break;
        }
    }
    let pitch = tokens
        .iter()
        .rev()
        .find(|s| {
            !s.to_ascii_lowercase().starts_with("vel")
                && !s.eq_ignore_ascii_case("left")
                && !s.eq_ignore_ascii_case("right")
        })
        .filter(|s| {
            note(s)
                || looped && {
                    let lower = s.to_ascii_lowercase();
                    lower
                        .strip_suffix("maj")
                        .or_else(|| lower.strip_suffix("min"))
                        .or_else(|| lower.strip_suffix('m'))
                        .is_some_and(note)
                }
        })
        .map(|s| s.to_string());
    let display_name = tokens
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            !(identifier.is_some() && *i == 0
                || bpm_index == Some(*i)
                || pitch
                    .as_ref()
                    .is_some_and(|p| p == *tokens.get(*i).unwrap_or(&"")))
        })
        .map(|(_, s)| *s)
        .collect::<Vec<_>>()
        .join(" ");
    Hints {
        identifier,
        display_name: if display_name.is_empty() {
            stem.chars().take(120).collect()
        } else {
            display_name.chars().take(120).collect()
        },
        kind: if looped {
            "Loop"
        } else if pitch.is_some() {
            "OneShot"
        } else {
            "Unknown"
        }
        .into(),
        note: if looped { None } else { pitch.clone() },
        key: if looped { pitch } else { None },
        bpm,
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioInfo {
    pub duration: f64,
    pub sample_rate: u32,
    pub channels: u16,
    pub format: String,
    pub frames: u64,
}
pub fn inspect(path: &Path) -> Result<AudioInfo, String> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let length = f.metadata().map_err(|e| e.to_string())?.len();
    if length > audio_backend::samples::MAX_WAV_BYTES as u64 {
        return Err("WAV exceeds 64 MiB input limit".into());
    }
    let mut header = [0; 12];
    f.read_exact(&mut header).map_err(|e| e.to_string())?;
    if &header[..4] != b"RIFF" || &header[8..] != b"WAVE" {
        return Err("Only RIFF WAV is currently decoded".into());
    }
    let mut fmt = None;
    let mut data = None;
    for _ in 0..256 {
        let mut h = [0; 8];
        if f.read_exact(&mut h).is_err() {
            break;
        }
        let size = u32::from_le_bytes(h[4..8].try_into().unwrap());
        let start = f.stream_position().map_err(|e| e.to_string())?;
        if start + u64::from(size) > length {
            return Err("Truncated WAV chunk".into());
        }
        if &h[..4] == b"fmt " && size >= 16 {
            let mut b = [0; 16];
            f.read_exact(&mut b).map_err(|e| e.to_string())?;
            fmt = Some((
                u16::from_le_bytes([b[0], b[1]]),
                u16::from_le_bytes([b[2], b[3]]),
                u32::from_le_bytes(b[4..8].try_into().unwrap()),
                u16::from_le_bytes([b[14], b[15]]),
            ));
        } else if &h[..4] == b"data" {
            data = Some(size);
        }
        if fmt.is_some() && data.is_some() {
            break;
        }
        f.seek(SeekFrom::Start(
            start + u64::from(size) + u64::from(size % 2),
        ))
        .map_err(|e| e.to_string())?;
    }
    let (encoding, channels, rate, bits) = fmt.ok_or("Missing WAV format")?;
    if ![1, 2].contains(&channels)
        || !(1000..=384000).contains(&rate)
        || !matches!((encoding, bits), (1, 16 | 24 | 32) | (3, 32))
    {
        return Err("Unsupported WAV encoding, channels or rate".into());
    }
    let frames =
        u64::from(data.ok_or("Missing WAV data")?) / (u64::from(channels) * u64::from(bits / 8));
    Ok(AudioInfo {
        duration: frames as f64 / f64::from(rate),
        sample_rate: rate,
        channels,
        format: format!("WAV {}{bits}", if encoding == 3 { "float" } else { "PCM" }),
        frames,
    })
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u16,
    pub id: String,
    pub name: String,
    pub license: Option<String>,
    pub creator: Option<String>,
    pub provenance: Option<String>,
    #[serde(default)]
    pub sources: BTreeMap<String, Source>,
    #[serde(default)]
    pub samples: BTreeMap<String, SampleOrigin>,
    #[serde(default)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Source {
    pub id: String,
    pub title: String,
    pub document: String,
    pub document_sha256: String,
    pub description: Option<String>,
    pub creator: Option<String>,
    pub contributors: Vec<String>,
    pub source_url: Option<String>,
    pub license: Option<String>,
    pub license_url: Option<String>,
    pub copyright: Option<String>,
    pub attribution: Option<String>,
    pub redistribution: Option<String>,
    pub modifications: Option<String>,
    pub preservation: Option<String>,
    pub text: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SampleOrigin {
    pub id: String,
    pub source: Option<String>,
    pub original_path: String,
    pub original_category: String,
    pub sha256: String,
    pub license: Option<String>,
    pub creator: Option<String>,
    pub attribution: Option<String>,
    pub metadata: BTreeMap<String, serde_json::Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pack {
    pub license_document: Option<String>,
    pub license_text: Option<String>,
    pub creator: Option<String>,
    pub sources: BTreeMap<String, Source>,
    pub id: String,
    pub name: String,
    pub folder: String,
    pub license: Option<String>,
    pub provenance: Option<String>,
    pub review: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub origin: Option<SampleOrigin>,
    pub reference: Reference,
    pub filename: String,
    pub category: String,
    pub hints: Hints,
    pub audio: Option<AudioInfo>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub loading: bool,
    pub root: String,
    pub packs: Vec<Pack>,
    pub entries: Vec<Entry>,
    pub warnings: Vec<String>,
}
impl Catalog {
    pub fn discover(root: &Path) -> Result<Self, String> {
        let mut c = Self {
            root: root.display().to_string(),
            ..Default::default()
        };
        if !root.is_dir() {
            c.warnings.push(
                "No factory sample directory found; external import remains available".into(),
            );
            return Ok(c);
        }
        let mut dirs = children(root)?;
        dirs.retain(|p| p.is_dir() && !p.is_symlink());
        let mut ids = HashSet::new();
        let mut visits = 0;
        for dir in dirs.into_iter().take(128) {
            let folder = dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let manifest = dir.join("pack.json");
            let m = if manifest.is_file() {
                let mut bytes = Vec::new();
                std::fs::File::open(&manifest)
                    .map_err(|e| e.to_string())?
                    .take(2_097_153)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                match serde_json::from_slice::<Manifest>(&bytes) {
                    Ok(m) if bytes.len() <= 2_097_152 && [1, 2].contains(&m.version) => Some(m),
                    _ => {
                        c.warnings
                            .push(format!("Invalid pack.json: {folder}; pack not indexed"));
                        continue;
                    }
                }
            } else {
                None
            };
            let id = m.as_ref().map_or_else(|| folder.clone(), |m| m.id.clone());
            Reference {
                sample_id: None,
                pack: id.clone(),
                path: "check.wav".into(),
            }
            .validate()?;
            if !ids.insert(id.clone()) {
                return Err(format!(
                    "Duplicate pack identity {id}; resolve conflicting manifests before using factory assets"
                ));
            }
            let pack = Pack {
                license_document: None,
                license_text: None,
                creator: m.as_ref().and_then(|m| m.creator.clone()),
                sources: m.as_ref().map_or_else(BTreeMap::new, |m| m.sources.clone()),
                id: id.clone(),
                name: m
                    .as_ref()
                    .map_or_else(|| folder.clone(), |m| m.name.clone()),
                folder,
                license: m.as_ref().and_then(|m| m.license.clone()),
                provenance: m.as_ref().and_then(|m| m.provenance.clone()),
                review: m
                    .as_ref()
                    .is_none_or(|m| m.license.is_none() || m.provenance.is_none()),
            };
            c.walk(&dir, &dir, &id, 0, &mut visits)?;
            let mut pack = pack;
            if let Some(name) = ["LICENSE.md", "LICENSE.txt", "LICENSE"]
                .into_iter()
                .find(|name| dir.join(name).is_file() && !dir.join(name).is_symlink())
            {
                pack.license_document = Some(name.into());
                if let Ok(file) = std::fs::File::open(dir.join(name)) {
                    let mut bytes = Vec::new();
                    if file.take(65537).read_to_end(&mut bytes).is_ok() && bytes.len() <= 65536 {
                        pack.license_text = String::from_utf8(bytes).ok();
                    }
                }
            }
            if let Some(m) = m {
                if m.sources.len() > 256 || m.samples.len() > 4096 {
                    return Err(format!("Manifest budget exceeded: {id}"));
                }
                let mut sample_ids = HashSet::new();
                for (path, origin) in &m.samples {
                    Reference {
                        pack: id.clone(),
                        path: path.clone(),
                        sample_id: Some(origin.id.clone()),
                    }
                    .validate()?;
                    if !sample_ids.insert(&origin.id) {
                        return Err(format!("Duplicate sample identity in {id}"));
                    }
                    if origin
                        .source
                        .as_ref()
                        .is_some_and(|key| !pack.sources.contains_key(key))
                    {
                        return Err(format!("Unknown provenance for {id}/{path}"));
                    }
                }
                for e in c.entries.iter_mut().filter(|e| e.reference.pack == id) {
                    if let Some(origin) = m.samples.get(&e.reference.path) {
                        e.reference.sample_id = Some(origin.id.clone());
                        e.origin = Some(origin.clone());
                    }
                }
                let canonical = dir.canonicalize().map_err(|e| e.to_string())?;
                for source in pack.sources.values_mut() {
                    let doc = dir.join(&source.document);
                    if let Ok(path) = doc.canonicalize()
                        && path.starts_with(&canonical)
                    {
                        source.text = std::fs::File::open(path).ok().and_then(|f| {
                            let mut bytes = Vec::new();
                            f.take(65537).read_to_end(&mut bytes).ok()?;
                            if bytes.len() > 65536 {
                                return None;
                            }
                            String::from_utf8(bytes).ok()
                        });
                    }
                }
                pack.review = c
                    .entries
                    .iter()
                    .filter(|e| e.reference.pack == id)
                    .any(|e| {
                        e.origin
                            .as_ref()
                            .and_then(|o| {
                                o.license.as_ref().or_else(|| {
                                    o.source
                                        .as_ref()
                                        .and_then(|s| pack.sources.get(s))
                                        .and_then(|s| s.license.as_ref())
                                })
                            })
                            .or(pack.license.as_ref())
                            .is_none()
                    });
            }
            if pack.review {
                c.warnings.push(format!(
                    "{}: some license/provenance fields are unknown; inspect Source details",
                    pack.name
                ));
            }
            c.packs.push(pack);
        }
        Ok(c)
    }
    fn walk(
        &mut self,
        base: &Path,
        dir: &Path,
        pack: &str,
        depth: u8,
        visits: &mut usize,
    ) -> Result<(), String> {
        if depth > 8 || *visits >= 8192 || self.entries.len() >= 4096 {
            if self
                .warnings
                .last()
                .is_none_or(|w| !w.starts_with("Index limit"))
            {
                self.warnings
                    .push("Index limit reached (4096 sounds, 8192 entries, depth 8)".into());
            }
            return Ok(());
        }
        for p in children(dir)? {
            *visits += 1;
            if p.is_symlink() {
                continue;
            }
            if p.is_dir() {
                self.walk(base, &p, pack, depth + 1, visits)?;
            } else if p
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("wav") || s.eq_ignore_ascii_case("mp3"))
            {
                let path = p
                    .strip_prefix(base)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                let category = Path::new(&path)
                    .parent()
                    .unwrap_or(Path::new(""))
                    .to_string_lossy()
                    .replace('\\', "/");
                let filename = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let result = inspect(&p);
                self.entries.push(Entry {
                    origin: None,
                    reference: Reference {
                        sample_id: None,
                        pack: pack.into(),
                        path,
                    },
                    hints: hints(&filename, &category),
                    filename,
                    category,
                    audio: result.as_ref().ok().cloned(),
                    error: result.err(),
                });
            }
            if *visits >= 8192 || self.entries.len() >= 4096 {
                break;
            }
        }
        Ok(())
    }
    pub fn resolve(&self, r: &Reference) -> Result<PathBuf, String> {
        r.validate()?;
        let pack = self
            .packs
            .iter()
            .find(|p| p.id == r.pack)
            .ok_or_else(|| format!("Missing factory pack: {}", r.pack))?;
        let relative = if let Some(id) = &r.sample_id {
            &self
                .entries
                .iter()
                .find(|e| e.reference.pack == r.pack && e.reference.sample_id.as_ref() == Some(id))
                .ok_or_else(|| format!("Missing stable factory sample: {id}"))?
                .reference
                .path
        } else {
            &r.path
        };
        let path = Path::new(&self.root).join(&pack.folder).join(relative);
        let base = Path::new(&self.root)
            .join(&pack.folder)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let resolved = path
            .canonicalize()
            .map_err(|e| format!("Missing factory sound {}: {e}", r.key()))?;
        if !resolved.starts_with(base) {
            return Err("Factory reference leaves its pack".into());
        }
        Ok(resolved)
    }
    pub fn entry(&self, r: &Reference) -> Option<&Entry> {
        self.entries.iter().find(|e| {
            e.reference.pack == r.pack
                && r.sample_id
                    .as_ref()
                    .map_or(e.reference.path == r.path, |id| {
                        e.reference.sample_id.as_ref() == Some(id)
                    })
        })
    }
}
fn children(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for e in std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .take(8193)
    {
        paths.push(e.map_err(|e| e.to_string())?.path())
    }
    paths.sort();
    Ok(paths)
}
