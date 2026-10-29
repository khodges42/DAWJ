// SPDX-License-Identifier: GPL-3.0-or-later
//! Logical stream-channel indices, not physical connector identities.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CueAvailability {
    /// Cue is never summed into the master or sent to an output.
    DisabledMasterOnly,
    /// Disjoint stream channels exist; physical headphone isolation is unverified.
    SeparateChannelsUnverified,
}

/// Immutable while streaming. Reopen a stream to change its channel mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputRouting {
    channels: u16,
    master: [u16; 2],
    cue: Option<[u16; 2]>,
}

impl OutputRouting {
    pub fn new(
        channels: u16,
        master: [u16; 2],
        cue: Option<[u16; 2]>,
    ) -> Result<Self, &'static str> {
        if channels < 2 {
            return Err("stereo master needs at least two channels");
        }
        let valid_pair =
            |pair: [u16; 2]| pair[0] != pair[1] && pair.iter().all(|&ch| ch < channels);
        if !valid_pair(master) {
            return Err("invalid master channel pair");
        }
        if let Some(pair) = cue {
            if !valid_pair(pair) || pair.iter().any(|ch| master.contains(ch)) {
                return Err("cue needs two distinct in-range channels separate from master");
            }
        }
        Ok(Self {
            channels,
            master,
            cue,
        })
    }
    pub fn stereo_master_only() -> Self {
        Self {
            channels: 2,
            master: [0, 1],
            cue: None,
        }
    }
    pub fn channels(self) -> u16 {
        self.channels
    }
    pub fn master(self) -> [u16; 2] {
        self.master
    }
    pub fn cue(self) -> Option<[u16; 2]> {
        self.cue
    }
    pub fn cue_availability(self) -> CueAvailability {
        if self.cue.is_some() {
            CueAvailability::SeparateChannelsUnverified
        } else {
            CueAvailability::DisabledMasterOnly
        }
    }
}
