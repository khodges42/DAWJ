// SPDX-License-Identifier: GPL-3.0-or-later
//! No MIDI or audio hardware required: real engine, normalized virtual messages.
use audio_backend::{demo, live::SlotState, routing::OutputRouting};
use dawj::{
    Coordinator, Mode,
    midi::{Mapper, device::input_bridge, profile::Profile},
};
fn main() -> Result<(), String> {
    let (mut engine, control, mixer) = demo::prepare()?;
    let mut co = Coordinator::new(control, mixer, false);
    let path = std::path::Path::new("target/synthetic-midi-profile.json");
    Profile::default().save(path)?;
    let mut midi = Mapper::new(Profile::load(path)?)?;
    let (input, events) = input_bridge(1);
    let send = |midi: &mut Mapper, co: &mut Coordinator, bytes: &[u8]| {
        input.receive(bytes);
        for packet in events.try_iter() {
            midi.handle(packet.event, Some(&mut *co));
        }
    };
    let route = OutputRouting::stereo_master_only();
    engine.render(&mut [0.0; 2], route);
    co.poll();
    send(&mut midi, &mut co, &[0x90, 36, 127]);
    assert_eq!(
        co.snapshot.unwrap().decks[0].slots[0].lifecycle,
        SlotState::Stopped
    );
    engine.render(&mut [0.0; 2], route);
    co.poll();
    assert_eq!(
        co.snapshot.unwrap().decks[0].slots[0].lifecycle,
        SlotState::QueuedStart
    );
    let updates = midi.feedback(Some(&co));
    println!("Queued: {updates:?}");
    midi.mark_sent(&updates);
    engine.render(&mut vec![0.0; 48002], route);
    co.poll();
    assert_eq!(
        co.snapshot.unwrap().decks[0].slots[0].lifecycle,
        SlotState::Playing
    );
    println!("Playing: {:?}", midi.feedback(Some(&co)));
    for bytes in [[0x90, 81, 127], [0xB0, 16, 127], [0xB0, 16, 64]] {
        send(&mut midi, &mut co, &bytes);
    }
    co.flush_parameters();
    engine.render(&mut [0.0; 256], route);
    co.poll();
    assert!(
        co.snapshot.unwrap().decks[0].slots[0]
            .parameters
            .cutoff_hz
            .is_some()
    );
    send(&mut midi, &mut co, &[0x90, 82, 127]);
    assert_eq!(co.focus.mode, Mode::Volume);
    for bytes in [[0xB0, 16, 0], [0xB0, 16, 127], [0xB0, 16, 80]] {
        send(&mut midi, &mut co, &bytes);
    }
    co.flush_parameters();
    engine.render(&mut [0.0; 256], route);
    co.poll();
    send(&mut midi, &mut co, &[0x80, 82, 0]);
    assert_eq!(co.focus.mode, Mode::Filter);
    println!(
        "Restored FILTER; accepted gain/cutoff {:?}",
        co.snapshot.unwrap().decks[0].slots[0].parameters
    );
    println!(
        "Synthetic MIDI acceptance passed; persisted {}",
        path.display()
    );
    Ok(())
}
