// SPDX-License-Identifier: GPL-3.0-or-later
// Diagnostic-only allocator instrumentation; never linked into the engine.
use audio_backend::{
    live::{LiveEngine, Quantize, Request},
    part_dsp::PartParameters,
    routing::OutputRouting,
    samples::{BoundaryPolicy, LoadedSample, SampleVoice},
};
use event_engine::live::MusicalScheduler;
use parts::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use timebase::*;
struct Alloc;
static WATCH: AtomicBool = AtomicBool::new(false);
static ALLOC: AtomicUsize = AtomicUsize::new(0);
static FREE: AtomicUsize = AtomicUsize::new(0);
unsafe impl std::alloc::GlobalAlloc for Alloc {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        if WATCH.load(Ordering::Relaxed) {
            ALLOC.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { std::alloc::System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        if WATCH.load(Ordering::Relaxed) {
            FREE.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { std::alloc::System.dealloc(p, l) }
    }
}
#[global_allocator]
static GLOBAL: Alloc = Alloc;
fn scheduler() -> MusicalScheduler {
    MusicalScheduler::new(
        TempoMap::new(
            48000,
            &[TempoChange {
                at: BeatTicks(0),
                bpm: 120.0,
            }],
        )
        .unwrap(),
        [BeatTicks(0); 2],
        [BeatTicks(30720); 2],
    )
    .unwrap()
}
fn voice(id: u64, oneshot: bool) -> SampleVoice {
    let n = 4096u32;
    let mut w = Vec::new();
    w.extend(b"RIFF");
    w.extend((36 + n * 4).to_le_bytes());
    w.extend(b"WAVEfmt ");
    w.extend(16u32.to_le_bytes());
    w.extend(3u16.to_le_bytes());
    w.extend(1u16.to_le_bytes());
    w.extend(48000u32.to_le_bytes());
    w.extend(192000u32.to_le_bytes());
    w.extend(4u16.to_le_bytes());
    w.extend(32u16.to_le_bytes());
    w.extend(b"data");
    w.extend((n * 4).to_le_bytes());
    for _ in 0..n {
        w.extend(1f32.to_le_bytes());
    }
    let p = Part {
        id: PartId(id),
        playback: PlaybackConfiguration {
            asset: AssetId(id),
            mode: if oneshot {
                PlaybackMode::OneShot
            } else {
                PlaybackMode::Loop {
                    start: 0,
                    end: n as u64,
                }
            },
        },
        timing: TimingPolicy {
            nominal_length: BeatTicks(3840),
            quantum: BeatTicks(960),
        },
    };
    SampleVoice::prepare(
        p,
        LoadedSample::decode(AssetId(id), &w, 48000).unwrap(),
        48000,
        BoundaryPolicy::Fade { frames: 64 },
    )
    .unwrap()
}
fn main() {
    let route = OutputRouting::stereo_master_only();
    let (mut e, mut c, _) = LiveEngine::new(scheduler(), vec![voice(1, true)]).unwrap();
    c.submit(
        0,
        0,
        Request::Parameters(PartParameters {
            cutoff_hz: Some(20.0),
            ..Default::default()
        }),
        Quantize::Immediate,
    )
    .unwrap();
    c.submit(0, 0, Request::Launch(0), Quantize::Immediate)
        .unwrap();
    let mut out = vec![0.0; 8194];
    e.render(&mut out, route);
    println!(
        "filtered faded EOF: last={} next={} step={}",
        out[8190],
        out[8192],
        out[8192] - out[8190]
    );
    let (mut e, mut c, m) = LiveEngine::new(scheduler(), vec![voice(1, false)]).unwrap();
    m.set_cue(audio_backend::mixer::CueSelection::Both);
    for d in 0..2 {
        for s in 0..8 {
            c.submit(d, s, Request::Launch(0), Quantize::Immediate)
                .unwrap();
        }
    }
    let mut out = [0.0; 2048];
    e.render(&mut out, route);
    c.submit(0, 0, Request::Stop, Quantize::Immediate).unwrap();
    let last = out[2046];
    e.render(&mut out[..2], route);
    println!(
        "one of 16 stops: last={last} next={} step={}",
        out[0],
        out[0] - last
    );
    c.submit(0, 0, Request::Launch(0), Quantize::Immediate)
        .unwrap();
    e.render(&mut out, route);
    for d in 0..2 {
        for s in 0..8 {
            c.submit(
                d,
                s,
                Request::Rule {
                    bank: 0,
                    rule: audio_backend::live::TriggerRule::Every {
                        n: 1,
                        unit: audio_backend::live::RuleUnit::Beats,
                        phase: audio_backend::live::RulePhase::Global,
                    },
                },
                Quantize::Immediate,
            )
            .unwrap();
        }
    }
    e.render(&mut out, route);
    for block in [32, 128, 512, 2048] {
        let mut out = vec![0.0; block * 4];
        let route = OutputRouting::new(4, [0, 1], Some([2, 3])).unwrap();
        let mut max = 0u128;
        let mut total = 0u128;
        for i in 0..1000 {
            for d in 0..2 {
                for s in 0..8 {
                    c.submit(
                        d,
                        s,
                        Request::Parameters(PartParameters {
                            gain: 0.5,
                            muted: false,
                            cutoff_hz: Some(if i % 2 == 0 { 20.0 } else { 20000.0 }),
                        }),
                        Quantize::Immediate,
                    )
                    .unwrap();
                    c.submit(d, s, Request::Retrigger, Quantize::Phrase)
                        .unwrap();
                }
            }
            WATCH.store(true, Ordering::Relaxed);
            let t = std::time::Instant::now();
            e.render(&mut out, route);
            let ns = t.elapsed().as_nanos();
            WATCH.store(false, Ordering::Relaxed);
            max = max.max(ns);
            total += ns;
        }
        println!(
            "stress block={block} callbacks=1000 mean_us={} max_us={} budget_us={} failed={}",
            total / 1000000,
            max / 1000,
            block * 1000000 / 48000,
            e.failed()
        );
    }
    assert_eq!(ALLOC.load(Ordering::Relaxed), 0);
    assert_eq!(FREE.load(Ordering::Relaxed), 0);
    println!("watched render allocations=0 deallocations=0");
}
