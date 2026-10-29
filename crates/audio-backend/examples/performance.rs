// SPDX-License-Identifier: GPL-3.0-or-later
#[path = "support/performance.rs"]
mod support;
use audio_backend::{
    OutputDiscovery,
    cpal_backend::{CpalDiscovery, CpalPlayback, PlaybackConfig, RequestedBuffer, SampleEncoding},
    routing::OutputRouting,
};
use std::{
    error::Error,
    fs::File,
    io::{self, BufRead, BufWriter, Seek, SeekFrom, Write},
    time::Duration,
};
use support::{Instruction, RATE};

struct Wav {
    file: BufWriter<File>,
    bytes: u32,
    channels: u16,
}
impl Wav {
    fn new(path: &str, channels: u16) -> Result<Self, Box<dyn Error>> {
        let mut f = BufWriter::new(File::create(path)?);
        f.write_all(b"RIFF")?;
        f.write_all(&36_u32.to_le_bytes())?;
        f.write_all(b"WAVEfmt ")?;
        f.write_all(&16_u32.to_le_bytes())?;
        f.write_all(&1_u16.to_le_bytes())?;
        f.write_all(&channels.to_le_bytes())?;
        f.write_all(&RATE.to_le_bytes())?;
        f.write_all(&(RATE * u32::from(channels) * 2).to_le_bytes())?;
        f.write_all(&(channels * 2).to_le_bytes())?;
        f.write_all(&16_u16.to_le_bytes())?;
        f.write_all(b"data")?;
        f.write_all(&0_u32.to_le_bytes())?;
        Ok(Self {
            file: f,
            bytes: 0,
            channels,
        })
    }
    fn write(&mut self, samples: &[f32]) -> Result<(), Box<dyn Error>> {
        let next = self
            .bytes
            .checked_add(u32::try_from(samples.len() * 2)?)
            .filter(|v| *v <= u32::MAX - 36)
            .ok_or("WAV limit reached")?;
        for sample in samples {
            self.file
                .write_all(&((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
        }
        self.bytes = next;
        Ok(())
    }
    fn finish(mut self) -> Result<(), Box<dyn Error>> {
        self.file.seek(SeekFrom::Start(4))?;
        self.file.write_all(&(36 + self.bytes).to_le_bytes())?;
        self.file.seek(SeekFrom::Start(40))?;
        self.file.write_all(&self.bytes.to_le_bytes())?;
        self.file.flush()?;
        println!(
            "Wrote {} frames, {} channels",
            self.bytes / u32::from(self.channels) / 2,
            self.channels
        );
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" {
        println!(
            "performance --render output.wav [--script path]\nperformance --play [--script path] [--device ordinal] [--channels 2|4]\nperformance --check [--device ordinal] [--channels 2|4]\nperformance --devices\nCommands: deck a|b; quant immediate|beat|bar|phrase; launch/stop/retrigger <1..8>; gain <slot> <0..1>; cutoff <slot> <Hz|off>; mute <slot> <true|false>; fill [quant]; every <N> beats|bars; phrasefill <1..8>; rule on|off|cancel; cross <0..1>; cue [a|b|both|off]; status; wait <beats>; quit.\nNo sound without --play. Live playback counts down three seconds. Stereo output has no private cue."
        );
        return Ok(());
    }
    if args == ["--devices"] {
        for d in CpalDiscovery::default().output_devices()? {
            println!("{} {:?}: {:?}", d.ordinal, d.name, d.configurations);
        }
        return Ok(());
    }
    let mode = args[0].as_str();
    if !matches!(mode, "--render" | "--play" | "--check") {
        return Err("unknown mode".into());
    }
    let mut index = 1;
    let mut output = None;
    let mut script = None;
    let mut ordinal = None;
    let mut channels = if mode == "--render" { 4 } else { 2 };
    if mode == "--render" {
        output = Some(args.get(index).ok_or("missing output path")?.clone());
        index += 1;
    }
    while index < args.len() {
        let flag = &args[index];
        let value = args.get(index + 1).ok_or("missing option value")?;
        match flag.as_str() {
            "--script" => script = Some(value.clone()),
            "--device" => ordinal = Some(value.parse()?),
            "--channels" => channels = value.parse()?,
            _ => return Err("unknown option".into()),
        }
        index += 2;
    }
    if !matches!(channels, 2 | 4) {
        return Err("use two or four channels".into());
    }
    let script_lines = if let Some(path) = &script {
        Some(std::fs::read_to_string(path)?)
    } else {
        None
    };
    if let (Some(path), Some(source)) = (&output, &script) {
        if std::path::Path::new(path)
            .canonicalize()
            .ok()
            .is_some_and(|p| Some(p) == std::path::Path::new(source).canonicalize().ok())
        {
            return Err("output cannot overwrite script".into());
        }
    }
    let routing = OutputRouting::new(
        channels,
        [0, 1],
        if channels == 4 { Some([2, 3]) } else { None },
    )?;
    println!(
        "Preparing Copper Steps and Glass Tide: 8 procedural parts each, 120 BPM, 48 kHz. {:?}",
        routing.cue_availability()
    );
    let (engine, mut session) = support::prepare(routing)?;
    let mut offline = Some(engine);
    let mut stream = None;
    let mut monitor = None;
    if mode != "--render" {
        let built = CpalPlayback::open_live(
            ordinal,
            PlaybackConfig {
                sample_rate_hz: RATE,
                channels,
                encoding: SampleEncoding::F32,
                buffer: RequestedBuffer::Default,
            },
            routing,
            offline.take().ok_or("engine unavailable")?,
        )?;
        println!(
            "Device: {} {:?}; routing {:?}",
            built.device_name(),
            built.config(),
            built.routing()
        );
        if mode == "--check" {
            println!("Paused stream built; no playback");
            return Ok(());
        }
        stream = Some(built);
        println!(
            "Quiet procedural audio will start after this countdown; lower headphone volume first."
        );
        for count in (1..=3).rev() {
            println!("{count}...");
            std::thread::sleep(Duration::from_secs(1));
        }
        let owner = stream.as_mut().ok_or("stream missing")?;
        monitor = Some(owner.monitor());
        owner.start()?;
    }
    let mut wav = output
        .as_deref()
        .map(|p| Wav::new(p, channels))
        .transpose()?;
    let (sender, receiver) = std::sync::mpsc::sync_channel(32);
    std::thread::spawn(move || {
        if let Some(lines) = script_lines {
            for line in lines.lines() {
                if sender.send(line.to_owned()).is_err() {
                    break;
                }
            }
        } else {
            for line in io::stdin().lock().lines() {
                match line {
                    Ok(line) => {
                        if sender.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    });
    let mut buffer = [0.0; 512 * 4];
    loop {
        if monitor.as_ref().is_some_and(|m| m.status().failed) {
            return Err("audio stream failed".into());
        }
        let line = match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => line,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };
        println!("> {line}");
        let instruction = match session.command(&line) {
            Ok(value) => value,
            Err(error) => {
                if script.is_some() {
                    return Err(error);
                }
                eprintln!("Rejected: {error}");
                continue;
            }
        };
        match instruction {
            Instruction::Quit => break,
            Instruction::CueUnavailable => {
                eprintln!("Private cue unavailable: stereo master-only; cue command not applied")
            }
            Instruction::Wait(frames) => {
                if let Some(engine) = &mut offline {
                    let mut remaining = frames;
                    while remaining > 0 {
                        let count = remaining.min(512);
                        engine.render(&mut buffer[..count * usize::from(channels)], routing);
                        if engine.failed() {
                            return Err("offline engine failed".into());
                        }
                        if let Some(file) = &mut wav {
                            file.write(&buffer[..count * usize::from(channels)])?;
                        }
                        remaining -= count;
                    }
                    println!("transport frame {}", engine.frame().0);
                } else if let Some(m) = &monitor {
                    let target = m
                        .status()
                        .source_frames
                        .checked_add(frames as u64)
                        .ok_or("clock overflow")?;
                    let start = std::time::Instant::now();
                    while m.status().source_frames < target {
                        if m.status().failed
                            || start.elapsed()
                                > Duration::from_secs(frames as u64 / u64::from(RATE) + 5)
                        {
                            return Err("audio clock failed or stalled".into());
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                }
            }
            Instruction::Continue => {}
        }
        if line.trim() == "status" {
            println!("{}", session.status());
        }
    }
    session.fadeout()?;
    if let Some(engine) = &mut offline {
        for _ in 0..10 {
            engine.render(&mut buffer[..480 * usize::from(channels)], routing);
            if let Some(file) = &mut wav {
                file.write(&buffer[..480 * usize::from(channels)])?;
            }
        }
        println!("Final shared transport frame {}", engine.frame().0);
    }
    if let Some(owner) = &mut stream {
        std::thread::sleep(Duration::from_millis(100));
        owner.stop()?;
        println!("{:?}", owner.monitor().status());
        if owner.monitor().status().failed {
            return Err("audio stream failed".into());
        }
    }
    if let Some(file) = wav {
        file.finish()?;
    }
    Ok(())
}
