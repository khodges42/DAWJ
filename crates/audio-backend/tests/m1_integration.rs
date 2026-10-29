// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline end-to-end mixer/automation/routing matrix, no device required.
use audio_backend::{
    mixer::{CrossfadeCurve, CueSelection, MASTER_CEILING, MixerParameters, StereoMixer},
    playback::{PlaybackSource, PreparedBuffer},
    routing::OutputRouting,
};

fn run(rate: u32, block: usize) -> Vec<f32> {
    let frames = rate as usize / 5;
    let source = |frame: [f32; 2], length| {
        PlaybackSource::Buffer(
            PreparedBuffer::new((0..length).flat_map(|_| frame).collect(), rate, 2).unwrap(),
        )
    };
    let (mut mixer, control) = StereoMixer::new(
        source([0.2, 0.0], frames),
        source([0.0, -0.3], frames / 2),
        rate,
    )
    .unwrap();
    let route = OutputRouting::new(6, [5, 0], Some([3, 1])).unwrap();
    let mut output = vec![9.0; (frames + 100) * 6];
    for (range, x, cue) in [
        (0..frames / 3, 0.0, CueSelection::B),
        (frames / 3..frames * 2 / 3, 1.0, CueSelection::Both),
        (frames * 2 / 3..frames + 100, 0.5, CueSelection::A),
    ] {
        control
            .set(MixerParameters {
                gain_a: 0.75,
                gain_b: 0.5,
                crossfader: x,
                curve: if x == 1.0 {
                    CrossfadeCurve::Linear
                } else {
                    CrossfadeCurve::EqualPower
                },
            })
            .unwrap();
        control.set_cue(cue);
        for chunk in output[range.start * 6..range.end * 6].chunks_mut(block * 6) {
            let before = mixer.positions();
            mixer.render_routed(chunk, route);
            let after = mixer.positions();
            assert!(after[0] >= before[0] && after[1] >= before[1]);
        }
    }
    assert_eq!(mixer.positions(), [frames as u64, (frames / 2) as u64]);
    assert!(mixer.finished());
    assert!(
        output
            .iter()
            .all(|s| s.is_finite() && s.abs() <= MASTER_CEILING)
    );
    for frame in output.chunks_exact(6) {
        assert_eq!(frame[2], 0.0);
        assert_eq!(frame[4], 0.0);
    }
    // Initially A is master-only, B is cue-only, with exact channel isolation.
    for frame in output[..frames / 3 * 6].chunks_exact(6) {
        assert_eq!(frame[0], 0.0);
        assert_eq!(frame[3], 0.0);
    }
    assert!(output[frames * 6..].iter().all(|s| *s == 0.0));
    output
}

#[test]
fn sample_rate_callback_and_automation_matrix() {
    for rate in [44100, 48000, 96000] {
        let reference = run(rate, 1);
        for block in [31, 64, 128, 511, 1024, 8192, 16384] {
            assert_eq!(run(rate, block), reference, "rate={rate}, block={block}");
        }
    }
}
