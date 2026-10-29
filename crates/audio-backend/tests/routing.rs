// SPDX-License-Identifier: GPL-3.0-or-later
use audio_backend::{
    mixer::{
        CrossfadeCurve, CueSelection, MASTER_CEILING, MixerControl, MixerParameters, StereoBuses,
        StereoMixer,
    },
    playback::{PlaybackSource, PreparedBuffer},
    routing::{CueAvailability, OutputRouting},
};

fn source(frame: [f32; 2], frames: usize) -> PlaybackSource {
    PlaybackSource::Buffer(
        PreparedBuffer::new((0..frames).flat_map(|_| frame).collect(), 48000, 2).unwrap(),
    )
}
fn mixer(cue: CueSelection, crossfader: f32) -> (StereoMixer, MixerControl) {
    let (mixer, control) = StereoMixer::new(
        source([0.2, -0.1], 10000),
        source([-0.4, 0.3], 10000),
        48000,
    )
    .unwrap();
    control
        .set(MixerParameters {
            gain_a: 1.0,
            gain_b: 1.0,
            crossfader,
            curve: CrossfadeCurve::Linear,
        })
        .unwrap();
    control.set_cue(cue);
    (mixer, control)
}
fn settle(mixer: &mut StereoMixer) -> StereoBuses {
    let mut samples = [StereoBuses::default(); 1200];
    mixer.render_buses(&mut samples);
    samples[1199]
}
fn close(actual: [f32; 2], expected: [f32; 2]) {
    for (a, e) in actual.into_iter().zip(expected) {
        assert!((a - e).abs() < 1e-6, "{actual:?} != {expected:?}");
    }
}

#[test]
fn cue_selection_is_post_gain_and_pre_crossfader() {
    for (cue, expected) in [
        (CueSelection::Off, [0.0, 0.0]),
        (CueSelection::A, [0.025, -0.0125]),
        (CueSelection::B, [-0.05, 0.0375]),
        (CueSelection::Both, [-0.025, 0.025]),
    ] {
        for crossfader in [0.0, 0.5, 1.0] {
            let (mut mixer, control) = mixer(cue, crossfader);
            close(settle(&mut mixer).cue, expected);
            control
                .set(MixerParameters {
                    gain_a: 0.0,
                    gain_b: 0.0,
                    crossfader,
                    ..MixerParameters::default()
                })
                .unwrap();
            close(settle(&mut mixer).cue, [0.0; 2]);
        }
    }
}

#[test]
fn selected_b_cannot_leak_into_a_only_master() {
    let (mut mixer, _) = mixer(CueSelection::B, 0.0);
    let frame = settle(&mut mixer);
    close(frame.master, [0.025, -0.0125]);
    close(frame.cue, [-0.05, 0.0375]);
}

#[test]
fn four_and_six_channel_mapping_preserves_polarity_and_silences_unused_channels() {
    for route in [
        OutputRouting::new(4, [0, 1], Some([2, 3])).unwrap(),
        OutputRouting::new(6, [5, 2], Some([3, 0])).unwrap(),
    ] {
        let (mut mixer, _) = mixer(CueSelection::B, 0.0);
        let channels = usize::from(route.channels());
        let mut output = vec![9.0; 1200 * channels + 1];
        mixer.render_routed(&mut output, route);
        let frame = &output[1199 * channels..1200 * channels];
        let mut expected = vec![0.0; channels];
        for (ch, value) in route.master().into_iter().zip([0.025, -0.0125]) {
            expected[usize::from(ch)] = value;
        }
        for (ch, value) in route.cue().unwrap().into_iter().zip([-0.05, 0.0375]) {
            expected[usize::from(ch)] = value;
        }
        for (a, e) in frame.iter().zip(expected) {
            assert!((a - e).abs() < 1e-6);
        }
        assert_eq!(*output.last().unwrap(), 0.0);
        assert_eq!(mixer.positions(), [1200, 1200]);
    }
}

#[test]
fn stereo_fallback_never_folds_cue_into_master() {
    let route = OutputRouting::stereo_master_only();
    assert_eq!(
        route.cue_availability(),
        CueAvailability::DisabledMasterOnly
    );
    assert_eq!(route.cue(), None);
    let (mut on, _) = mixer(CueSelection::Both, 0.0);
    let (mut off, _) = mixer(CueSelection::Off, 0.0);
    let mut with_cue = [0.0; 3000];
    let mut without_cue = [0.0; 3000];
    on.render_routed(&mut with_cue, route);
    off.render_routed(&mut without_cue, route);
    assert_eq!(with_cue, without_cue);
    assert!(OutputRouting::new(2, [0, 1], Some([0, 1])).is_err());
    assert_eq!(
        OutputRouting::new(4, [0, 1], Some([2, 3]))
            .unwrap()
            .cue_availability(),
        CueAvailability::SeparateChannelsUnverified
    );
}

#[test]
fn invalid_maps_are_rejected() {
    for (channels, master, cue) in [
        (0, [0, 1], None),
        (1, [0, 1], None),
        (4, [1, 1], None),
        (4, [0, 4], None),
        (4, [0, 1], Some([2, 2])),
        (4, [0, 1], Some([2, 4])),
        (4, [0, 1], Some([1, 3])),
        (3, [0, 1], Some([2, 0])),
    ] {
        assert!(OutputRouting::new(channels, master, cue).is_err());
    }
}

#[test]
fn cue_selection_changes_ramp_without_affecting_master() {
    let (mut changed, control) = mixer(CueSelection::Off, 0.0);
    let (mut reference, _) = mixer(CueSelection::Off, 0.0);
    settle(&mut changed);
    settle(&mut reference);
    control.set_cue(CueSelection::Both);
    let mut output = [StereoBuses::default(); 1200];
    let mut expected = output;
    changed.render_buses(&mut output);
    reference.render_buses(&mut expected);
    let mut previous = [0.0; 2];
    for (a, b) in output.iter().zip(expected) {
        assert_eq!(a.master, b.master);
        for (ch, prior) in previous.iter().enumerate() {
            assert!((a.cue[ch] - prior).abs() < 0.0002);
        }
        previous = a.cue;
    }
    control.set_cue(CueSelection::Off);
    close(settle(&mut changed).cue, [0.0; 2]);
}

#[test]
fn cue_overload_has_its_own_limiter_and_meters() {
    let create = |cue| {
        let (mixer, control) =
            StereoMixer::new(source([1.0, 0.5], 10000), source([1.0, 0.5], 10000), 48000).unwrap();
        control
            .set(MixerParameters {
                gain_a: 1.0,
                gain_b: 1.0,
                crossfader: 0.0,
                ..MixerParameters::default()
            })
            .unwrap();
        control.set_cue(cue);
        (mixer, control)
    };
    let (mut both, controls) = create(CueSelection::Both);
    let (mut off, _) = create(CueSelection::Off);
    let mut actual = [StereoBuses::default(); 3000];
    let mut expected = actual;
    both.render_buses(&mut actual);
    off.render_buses(&mut expected);
    for (a, b) in actual.iter().zip(expected) {
        assert_eq!(a.master, b.master);
        assert!(a.cue.iter().all(|s| s.abs() <= MASTER_CEILING));
        assert!((a.cue[0] * 0.5 - a.cue[1]).abs() < 1e-6);
    }
    close(controls.cue_peaks(), [MASTER_CEILING, MASTER_CEILING * 0.5]);
}

#[test]
fn routing_is_partition_invariant_through_cue_automation() {
    fn run(block: usize) -> Vec<f32> {
        let (mut mixer, control) = mixer(CueSelection::A, 0.0);
        let route = OutputRouting::new(4, [3, 1], Some([0, 2])).unwrap();
        let mut output = vec![0.0; 18000];
        for (range, cue) in [
            (0..6000, CueSelection::A),
            (6000..12000, CueSelection::Both),
            (12000..18000, CueSelection::B),
        ] {
            control.set_cue(cue);
            for chunk in output[range].chunks_mut(block * 4) {
                mixer.render_routed(chunk, route);
            }
        }
        output
    }
    assert_eq!(run(1), run(8192));
    assert_eq!(run(31), run(1024));
}
