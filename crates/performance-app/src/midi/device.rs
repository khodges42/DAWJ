// SPDX-License-Identifier: GPL-3.0-or-later
//! One non-audio worker owns all MIDI handles and output sends.
use super::{Event, normalize};
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
pub const EVENT_CAPACITY: usize = 256;
#[derive(Clone, Copy, Debug)]
pub struct Packet {
    pub epoch: u64,
    pub event: Event,
}
#[derive(Clone)]
pub struct InputBridge {
    tx: SyncSender<Packet>,
    pub dropped: Arc<AtomicU64>,
    epoch: u64,
}
impl InputBridge {
    /// The real midir callback and synthetic sources use this same bounded path.
    pub fn receive(&self, bytes: &[u8]) {
        if let Some(event) = normalize(bytes) {
            if self
                .tx
                .try_send(Packet {
                    epoch: self.epoch,
                    event,
                })
                .is_err()
            {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    fn with_epoch(&self, epoch: u64) -> Self {
        Self {
            epoch,
            ..self.clone()
        }
    }
}
pub fn input_bridge(epoch: u64) -> (InputBridge, Receiver<Packet>) {
    let (tx, rx) = mpsc::sync_channel(EVENT_CAPACITY);
    (
        InputBridge {
            tx,
            dropped: Arc::new(AtomicU64::new(0)),
            epoch,
        },
        rx,
    )
}
pub fn unique_port(names: &[String], name: &str) -> Result<Option<usize>, &'static str> {
    let mut found = names.iter().enumerate().filter(|(_, n)| n.as_str() == name);
    let first = found.next().map(|(i, _)| i);
    if found.next().is_some() {
        Err("ambiguous MIDI device name")
    } else {
        Ok(first)
    }
}
#[derive(Clone, Default, Debug)]
pub struct DeviceStatus {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub input: Option<String>,
    pub output: Option<String>,
    pub input_epoch: u64,
    pub output_epoch: u64,
    pub message: String,
}
enum Command {
    Connect(Option<String>, Option<String>),
    Feedback(Vec<[u8; 3]>),
    Rescan,
    Shutdown(Vec<[u8; 3]>),
}
pub struct Devices {
    commands: SyncSender<Command>,
    pub events: Receiver<Packet>,
    status: Arc<Mutex<DeviceStatus>>,
    pub dropped: Arc<AtomicU64>,
    worker: Option<JoinHandle<()>>,
}
impl Devices {
    pub fn new(input: Option<String>, output: Option<String>) -> Self {
        let (commands, rx) = mpsc::sync_channel(8);
        let (bridge, events) = input_bridge(0);
        let status = Arc::new(Mutex::new(DeviceStatus::default()));
        let dropped = bridge.dropped.clone();
        let shared = status.clone();
        let worker = thread::spawn(move || run(rx, bridge, shared, input, output));
        Self {
            commands,
            events,
            status,
            dropped,
            worker: Some(worker),
        }
    }
    pub fn status(&self) -> DeviceStatus {
        self.status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    pub fn connect(&self, input: Option<String>, output: Option<String>) -> Result<(), String> {
        self.commands
            .try_send(Command::Connect(input, output))
            .map_err(|_| "MIDI worker command queue full/closed".into())
    }
    pub fn rescan(&self) -> Result<(), String> {
        self.commands
            .try_send(Command::Rescan)
            .map_err(|_| "MIDI worker command queue full/closed".into())
    }
    pub fn feedback(&self, messages: Vec<[u8; 3]>) -> bool {
        messages.len() <= 16 && self.commands.try_send(Command::Feedback(messages)).is_ok()
    }
}
impl Devices {
    pub fn shutdown(&mut self, off: Vec<[u8; 3]>) {
        if let Some(worker) = self.worker.take() {
            let _ = self.commands.send(Command::Shutdown(off));
            let _ = worker.join();
        }
    }
}
impl Drop for Devices {
    fn drop(&mut self) {
        // Non-realtime teardown. Queue drains on its own worker; no engine/audio handles.
        self.shutdown(Vec::new());
    }
}
fn run(
    rx: Receiver<Command>,
    bridge: InputBridge,
    status: Arc<Mutex<DeviceStatus>>,
    mut wanted_in: Option<String>,
    mut wanted_out: Option<String>,
) {
    let mut input: Option<MidiInputConnection<()>> = None;
    let mut output: Option<MidiOutputConnection> = None;
    let mut state = DeviceStatus::default();
    let mut scan = Instant::now() - Duration::from_secs(3);
    loop {
        match rx.recv_timeout(Duration::from_millis(10)) {
            Ok(Command::Shutdown(off)) => {
                if let Some(out) = &mut output {
                    for m in off {
                        let _ = out.send(&m);
                    }
                }
                break;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Command::Rescan) => scan = Instant::now() - Duration::from_secs(3),
            Ok(Command::Connect(i, o)) => {
                input = None;
                output = None;
                state.input = None;
                state.output = None;
                state.input_epoch = state.input_epoch.wrapping_add(1);
                state.output_epoch = state.output_epoch.wrapping_add(1);
                wanted_in = i;
                wanted_out = o;
                scan = Instant::now() - Duration::from_secs(3);
            }
            Ok(Command::Feedback(messages)) => {
                if let Some(out) = &mut output {
                    for m in messages {
                        if let Err(e) = out.send(&m) {
                            state.message = format!("MIDI output failed: {e}");
                            output = None;
                            state.output = None;
                            state.output_epoch = state.output_epoch.wrapping_add(1);
                            break;
                        }
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if scan.elapsed() >= Duration::from_secs(2) {
            scan = Instant::now();
            match (
                MidiInput::new("DAWJ discovery"),
                MidiOutput::new("DAWJ feedback"),
            ) {
                (Ok(mut mi), Ok(mo)) => {
                    let ip = mi.ports();
                    let op = mo.ports();
                    state.inputs = ip
                        .iter()
                        .take(64)
                        .map(|p| {
                            mi.port_name(p)
                                .unwrap_or_else(|_| "Unavailable port".into())
                        })
                        .collect();
                    state.outputs = op
                        .iter()
                        .take(64)
                        .map(|p| {
                            mo.port_name(p)
                                .unwrap_or_else(|_| "Unavailable port".into())
                        })
                        .collect();
                    if state
                        .input
                        .as_ref()
                        .is_some_and(|n| !state.inputs.contains(n))
                    {
                        input = None;
                        state.input = None;
                        state.input_epoch = state.input_epoch.wrapping_add(1);
                        state.message = "MIDI input disconnected; waiting for named port".into();
                    }
                    if state
                        .output
                        .as_ref()
                        .is_some_and(|n| !state.outputs.contains(n))
                    {
                        output = None;
                        state.output = None;
                        state.output_epoch = state.output_epoch.wrapping_add(1);
                    }
                    if input.is_none() {
                        if let Some(name) = &wanted_in {
                            if let Ok(Some(index)) = unique_port(&state.inputs, name) {
                                mi.ignore(midir::Ignore::All);
                                let producer = bridge.with_epoch(state.input_epoch.wrapping_add(1));
                                match mi.connect(
                                    &ip[index],
                                    "DAWJ controls",
                                    move |_, bytes, _| producer.receive(bytes),
                                    (),
                                ) {
                                    Ok(c) => {
                                        input = Some(c);
                                        state.input = Some(name.clone());
                                        state.input_epoch = state.input_epoch.wrapping_add(1);
                                        state.message = "MIDI input connected".into();
                                    }
                                    Err(e) => {
                                        state.message = format!("MIDI input connection failed: {e}")
                                    }
                                }
                            } else {
                                state.message = format!(
                                    "Input '{name}' unavailable or ambiguous; reconnect/rescan"
                                );
                            }
                        }
                    }
                    if output.is_none() {
                        if let Some(name) = &wanted_out {
                            if let Ok(Some(index)) = unique_port(&state.outputs, name) {
                                match mo.connect(&op[index], "DAWJ state") {
                                    Ok(c) => {
                                        output = Some(c);
                                        state.output = Some(name.clone());
                                        state.output_epoch = state.output_epoch.wrapping_add(1);
                                    }
                                    Err(e) => {
                                        state.message =
                                            format!("MIDI output connection failed: {e}")
                                    }
                                }
                            }
                        }
                    }
                }
                (Err(e), _) => state.message = format!("MIDI input unavailable: {e}"),
                (_, Err(e)) => state.message = format!("MIDI output unavailable: {e}"),
            }
        }
        *status.lock().unwrap_or_else(|p| p.into_inner()) = state.clone();
    }
}
