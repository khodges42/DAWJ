// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded sample-trigger pattern. Zero velocity is a rest; 1..127 is a hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pattern {
    pub bars: u8,
    pub steps: [[u8; 16]; 4],
}
impl Default for Pattern {
    fn default() -> Self {
        Self {
            bars: 1,
            steps: [[0; 16]; 4],
        }
    }
}
impl Pattern {
    pub fn validate(self) -> Result<Self, &'static str> {
        if ![1, 2, 4].contains(&self.bars) || self.steps.iter().flatten().any(|v| *v > 127) {
            Err("Pattern requires 1, 2 or 4 bars and velocities 0..127")
        } else {
            Ok(self)
        }
    }
    pub fn velocity(self, step: usize) -> u8 {
        self.steps[step / 16][step % 16]
    }
}
