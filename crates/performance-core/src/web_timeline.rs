// SPDX-License-Identifier: GPL-3.0-or-later
//! Wire projection of the existing pure timeline. No frontend scheduling math.
use crate::{
    Coordinator,
    timeline::{self, View},
};
use serde::{Deserialize, Serialize};
use timebase::BeatTicks;
#[derive(Debug, Serialize, Deserialize)]
pub struct GridLine {
    pub fraction: f32,
    pub bar: Option<i64>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Group {
    pub lane: usize,
    pub fraction: f32,
    pub label: String,
    pub members: Vec<usize>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Marker {
    pub deck: usize,
    pub slot: usize,
    pub tick: i64,
    pub fraction: f32,
    pub label: String,
    pub kind: timeline::MarkerKind,
    pub projected: bool,
    pub manual: bool,
    pub command_id: Option<u64>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Projection {
    pub groups: Vec<Group>,
    pub generation: u64,
    pub start: i64,
    pub end: i64,
    pub phrase_start: i64,
    pub phrase_length: i64,
    pub playhead: Option<f32>,
    pub zoom_bars: u8,
    pub follow: bool,
    pub grid: Vec<GridLine>,
    pub markers: Vec<Marker>,
    pub activity: [usize; 2],
    pub inspector: String,
    pub phrase_position: [(i64, i64); 2],
}
impl Projection {
    pub fn from_coordinator(co: &Coordinator, view: &View) -> Option<Self> {
        let s = co.snapshot?;
        let w = view.window(&s);
        let (d, p) = co.focus.address();
        let markers = timeline::markers(&s, w);
        let mut groups = Vec::new();
        for lane in 0..3 {
            let mut rows = std::collections::BTreeMap::<i64, Vec<usize>>::new();
            for (i, m) in markers.iter().enumerate() {
                if (lane < 2 && m.deck == lane) || (lane == 2 && (m.deck, m.slot) == (d, p)) {
                    rows.entry(m.at.0).or_default().push(i);
                }
            }
            for (tick, mut members) in rows {
                members.sort_by_key(|i| !markers[*i].manual);
                let group: Vec<_> = members.iter().map(|i| markers[*i]).collect();
                groups.push(Group {
                    lane,
                    fraction: w.fraction(BeatTicks(tick)),
                    label: timeline::marker_label(&group),
                    members,
                });
            }
        }
        Some(Self {
            groups,
            generation: s.generation,
            start: w.start.0,
            end: w.end.0,
            phrase_start: w.phrase_start.0,
            phrase_length: w.phrase_length.0,
            playhead: (s.beat >= w.start && s.beat <= w.end).then(|| w.fraction(s.beat)),
            // The window is capped by the actual engine phrase. Report what is
            // displayed, rather than an eight-bar request over a four-bar phrase.
            zoom_bars: ((w.end.0 - w.start.0) / timeline::BAR) as u8,
            follow: view.follow,
            grid: (w.start.0..=w.end.0)
                .step_by(960)
                .map(|tick| GridLine {
                    fraction: w.fraction(BeatTicks(tick)),
                    bar: (tick % timeline::BAR == 0).then(|| {
                        (tick - w.phrase_start.0).rem_euclid(w.phrase_length.0) / timeline::BAR + 1
                    }),
                })
                .collect(),
            markers: markers
                .iter()
                .map(|m| Marker {
                    deck: m.deck,
                    slot: m.slot,
                    tick: m.at.0,
                    fraction: w.fraction(m.at),
                    label: timeline::marker_label(&[*m]),
                    kind: m.kind,
                    projected: m.projected,
                    manual: m.manual,
                    command_id: m.command_id,
                })
                .collect(),
            activity: s
                .decks
                .map(|d| d.slots.iter().filter(|p| p.active.is_some()).count()),
            inspector: timeline::inspector(s.decks[d].slots[p]),
            phrase_position: [
                timeline::phrase_position(&s, 0)?,
                timeline::phrase_position(&s, 1)?,
            ],
        })
    }
}
