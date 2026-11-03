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
    pub pack: String,
    pub path: String,
}
impl Reference {
    pub fn validate(&self) -> Result<(), String> {
        if self.pack.is_empty()
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
        .last()
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
                || pitch.is_some() && *i == tokens.len() - 1)
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
    pub extra: BTreeMap<String, serde_json::Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pack {
    pub id: String,
    pub name: String,
    pub folder: String,
    pub license: Option<String>,
    pub provenance: Option<String>,
    pub review: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
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
                    .take(16385)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                match serde_json::from_slice::<Manifest>(&bytes) {
                    Ok(m) if bytes.len() <= 16384 && m.version == 1 => Some(m),
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
            if pack.review {
                c.warnings.push(format!(
                    "{}: missing license/provenance; local use allowed, release review required",
                    pack.name
                ));
            }
            c.walk(&dir, &dir, &id, 0, &mut visits)?;
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
                    reference: Reference {
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
        let path = Path::new(&self.root).join(&pack.folder).join(&r.path);
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
