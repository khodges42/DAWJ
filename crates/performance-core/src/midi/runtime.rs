// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared bounded MIDI draining, disconnect handling, pickup and LED cadence.
use super::{
    Mapper,
    device::{DeviceStatus, Devices},
};
use crate::Coordinator;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
pub fn load_profile(path: &std::path::Path) -> (super::profile::Profile, String) {
    use super::profile::Profile;
    if path.exists() {
        match Profile::load(path) {
            Ok(profile) => (profile, String::new()),
            Err(error) => (
                Profile::default(),
                format!("Saved profile rejected: {error}"),
            ),
        }
    } else {
        (Profile::default(), String::new())
    }
}
pub fn shutdown(mapper: &Mapper, devices: &mut Devices) {
    devices.shutdown(
        mapper
            .profile
            .leds
            .iter()
            .map(|led| led.output.message(mapper.profile.values.stopped))
            .collect(),
    );
}
pub fn poll(
    mapper: &mut Mapper,
    devices: &Devices,
    state_cache: &mut DeviceStatus,
    dropped_cache: &mut u64,
    feedback_tick: &mut Instant,
    mut co: Option<&mut Coordinator>,
) {
    let state = devices.status();
    let dropped = devices.dropped.load(Ordering::Relaxed);
    if state.input_epoch != state_cache.input_epoch || dropped != (*dropped_cache) {
        mapper.reset(co.as_deref_mut());
        if dropped != (*dropped_cache) {
            // A lost release makes queued button presses unsafe to replay.
            for _ in devices
                .events
                .try_iter()
                .take(super::device::EVENT_CAPACITY)
            {}
            mapper.status =
                format!("MIDI input overflow: {dropped} dropped; temporary modes/pickup reset");
        }
    }
    if state.output_epoch != state_cache.output_epoch {
        mapper.refresh_feedback();
    }
    (*dropped_cache) = dropped;
    (*state_cache) = state;
    for packet in devices.events.try_iter().take(128) {
        if packet.epoch == state_cache.input_epoch {
            let revision = mapper.revision;
            mapper.handle(packet.event, co.as_deref_mut());
            if mapper.revision != revision {
                for _ in devices
                    .events
                    .try_iter()
                    .take(super::device::EVENT_CAPACITY)
                {}
                break;
            }
        }
    }
    if let Some(co) = &mut co {
        co.flush_parameters();
    }
    if (*feedback_tick).elapsed() >= Duration::from_millis(50) && state_cache.output.is_some() {
        (*feedback_tick) = Instant::now();
        let updates: Vec<_> = mapper
            .feedback(co.as_deref())
            .into_iter()
            .take(16)
            .collect();
        if !updates.is_empty() && devices.feedback(updates.iter().map(|(_, m)| *m).collect()) {
            mapper.mark_sent(&updates);
        }
    }
}
