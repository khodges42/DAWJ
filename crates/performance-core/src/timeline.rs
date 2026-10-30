// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure musical projection. No frame clock, scheduling or event ownership.
use audio_backend::performance_state::{PartState, PendingKind, PerformanceState};
use timebase::BeatTicks;
pub const BAR: i64 = 4 * 960;
pub const MAX_OCCURRENCES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MarkerKind {
    Launch,
    Stop,
    Retrigger,
    Replace,
    Fill,
    Rule,
    RuleFill,
    Fired,
}
impl MarkerKind {
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Launch => "L",
            Self::Stop => "S",
            Self::Retrigger => "R",
            Self::Replace => "X",
            Self::Fill => "F",
            Self::Rule => "P",
            Self::RuleFill => "F*",
            Self::Fired => "!",
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Marker {
    pub deck: usize,
    pub slot: usize,
    pub at: BeatTicks,
    pub kind: MarkerKind,
    /// The first rule occurrence is published. Later occurrences are conditional projections.
    pub projected: bool,
    pub manual: bool,
    pub command_id: Option<u64>,
}
pub fn marker_label(group: &[Marker]) -> String {
    let Some(first) = group.first() else {
        return String::new();
    };
    let tag = |m: &Marker| format!("{}{}", if m.projected { "~" } else { "" }, m.kind.glyph());
    if group.len() == 1 {
        return format!("{}{}", tag(first), first.slot + 1);
    }
    let mut kinds = Vec::new();
    for m in group {
        let label = tag(m);
        if !kinds.contains(&label) {
            kinds.push(label);
        }
    }
    if kinds.len() == 1 {
        format!("{}x{}", kinds[0], group.len())
    } else {
        kinds.join("/")
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub start: BeatTicks,
    pub end: BeatTicks,
    pub phrase_start: BeatTicks,
    pub phrase_length: BeatTicks,
}
impl Window {
    pub fn contains(self, at: BeatTicks) -> bool {
        at >= self.start && at < self.end
    }
    pub fn fraction(self, at: BeatTicks) -> f32 {
        ((at.0 - self.start.0) as f64 / (self.end.0 - self.start.0) as f64) as f32
    }
}
#[derive(Clone, Debug)]
pub struct View {
    pub zoom_bars: u8,
    pub follow: bool,
    pub page: u8,
    pub anchor: Option<BeatTicks>,
}
impl Default for View {
    fn default() -> Self {
        Self {
            zoom_bars: 8,
            follow: true,
            page: 0,
            anchor: None,
        }
    }
}
impl View {
    pub fn set_follow(&mut self, on: bool, s: &PerformanceState) {
        let current = self.window(s);
        if !on {
            self.anchor = Some(current.phrase_start);
            self.page = ((current.start.0 - current.phrase_start.0)
                / (current.end.0 - current.start.0)) as u8;
        }
        self.follow = on;
    }
    pub fn window(&self, s: &PerformanceState) -> Window {
        // A global grid anchored at zero; deck epochs are displayed separately.
        let phrase = s.decks[0].phrase_length.0.max(BAR);
        let phrase_start = self
            .anchor
            .filter(|_| !self.follow)
            .unwrap_or(BeatTicks(s.beat.0.div_euclid(phrase) * phrase));
        let width = (i64::from(self.zoom_bars.max(1)) * BAR).min(phrase);
        let pages = (phrase / width).max(1);
        let page = if self.follow {
            (s.beat.0 - phrase_start.0).rem_euclid(phrase) / width
        } else {
            i64::from(self.page).min(pages - 1)
        };
        let start = BeatTicks(phrase_start.0 + page * width);
        Window {
            start,
            end: BeatTicks(start.0 + width),
            phrase_start,
            phrase_length: BeatTicks(phrase),
        }
    }
    pub fn page_by(&mut self, s: &PerformanceState, delta: i32) {
        let current = self.window(s);
        let width = current.end.0 - current.start.0;
        let pages = (current.phrase_length.0 / width).max(1);
        let old = (current.start.0 - current.phrase_start.0) / width;
        self.follow = false;
        self.anchor = Some(current.phrase_start);
        self.page = (old + i64::from(delta)).rem_euclid(pages) as u8;
    }
}
pub fn phrase_position(s: &PerformanceState, deck: usize) -> Option<(i64, i64)> {
    let d = s.decks.get(deck)?;
    if d.phrase_length.0 <= 0 {
        return None;
    }
    Some((d.phase.0.div_euclid(BAR) + 1, d.phrase_length.0 / BAR))
}
pub fn markers(s: &PerformanceState, w: Window) -> Vec<Marker> {
    let mut out = Vec::with_capacity(2 * 8 * (MAX_OCCURRENCES + 2));
    for deck in 0..2 {
        for slot in 0..8 {
            let p = s.decks[deck].slots[slot];
            if let Some(a) = p.pending
                && a.target >= w.start
                && a.target <= w.end
            {
                out.push(Marker {
                    deck,
                    slot,
                    at: a.target,
                    kind: match a.kind {
                        PendingKind::Launch if p.pending_one_shot => MarkerKind::Fill,
                        PendingKind::Launch => MarkerKind::Launch,
                        PendingKind::Stop => MarkerKind::Stop,
                        PendingKind::Retrigger => MarkerKind::Retrigger,
                        PendingKind::Replace => MarkerKind::Replace,
                    },
                    projected: false,
                    manual: true,
                    command_id: a.command_id,
                });
            }
            if let Some(r) = p.rule {
                if let Some(at) = r.last_fired.filter(|at| w.contains(*at)) {
                    out.push(Marker {
                        deck,
                        slot,
                        at,
                        kind: MarkerKind::Fired,
                        projected: false,
                        manual: false,
                        command_id: None,
                    });
                }
                if let Some(next) = r.next {
                    // Skip directly into the visible interval, then visit at most
                    // 32 occurrences; long sessions never cause a history scan.
                    let skip = r
                        .period
                        .filter(|p| p.0 > 0)
                        .map_or(0, |p| (w.start.0 - next.0).max(0).div_euclid(p.0));
                    for n in 0..MAX_OCCURRENCES {
                        let count = skip + n as i64;
                        let Some(offset) = r.period.map_or(Some(0), |p| p.0.checked_mul(count))
                        else {
                            break;
                        };
                        let Some(t) = next.0.checked_add(offset) else {
                            break;
                        };
                        if t > w.end.0 {
                            break;
                        }
                        if t >= w.start.0 {
                            out.push(Marker {
                                deck,
                                slot,
                                at: BeatTicks(t),
                                kind: if r.one_shot {
                                    MarkerKind::RuleFill
                                } else {
                                    MarkerKind::Rule
                                },
                                projected: count > 0,
                                manual: false,
                                command_id: None,
                            });
                        }
                        if r.period.is_none() {
                            break;
                        }
                    }
                }
            }
        }
    }
    out
}
pub fn inspector(p: PartState) -> String {
    let mut text = String::new();
    if p.active.is_some() {
        text.push_str(if p.active_one_shot {
            "FILL active"
        } else {
            "Loop active"
        });
    } else {
        text.push_str("No active source");
    }
    if let (Some(phase), Some(length)) = (p.phase, p.nominal_length) {
        text.push_str(&format!(
            " | nominal cycle {:.2}/{:.0} beats",
            phase.0 as f64 / 960.0,
            length.0 as f64 / 960.0
        ));
    }
    if let Some(r) = p.rule {
        text.push_str(&format!(
            " | rule {:?}, fired {}, skipped {}{}",
            r.rule,
            r.fired,
            r.skipped,
            if r.consumed {
                " (spent)"
            } else if r.next.is_none() {
                " (paused)"
            } else {
                ""
            }
        ));
        if let Some(next) = r.next {
            text.push_str(&format!(" | next {}", crate::musical_position(next)));
        }
    }
    text
}
