// SPDX-License-Identifier: GPL-3.0-or-later
//! Frontend-neutral, bounded control worker. CPAL is constructed and dropped here.
use crate::{
    Coordinator,
    ipc::*,
    midi::{
        self, Learn, Mapper,
        device::{DeviceStatus, Devices},
        profile::{Profile, default_path},
    },
    output::{self, OutputSession},
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
struct Permit(Arc<AtomicUsize>);
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
struct Envelope {
    request: Request,
    reply: SyncSender<Reply>,
    _permit: Permit,
}
#[derive(Clone)]
pub struct Client {
    tx: SyncSender<Envelope>,
    admitted: Arc<AtomicUsize>,
    closed: Arc<AtomicBool>,
}
pub struct Pending {
    id: u32,
    rx: Receiver<Reply>,
}
impl Pending {
    /// Only call on a non-UI, non-audio worker. Dropped receivers do not cancel musical actions.
    pub fn wait(self) -> Reply {
        self.rx.recv().unwrap_or_else(|_| {
            Reply::new(
                self.id,
                Err(Fault::new(ErrorCode::Closed, "Control worker closed")),
            )
        })
    }
}
impl Client {
    pub fn submit(&self, request: Request) -> Result<Pending, Fault> {
        if request.version != VERSION {
            return Err(Fault::new(
                ErrorCode::UnsupportedVersion,
                "Unsupported frontend protocol version",
            ));
        }
        if self.closed.load(Ordering::Acquire) {
            return Err(Fault::new(
                ErrorCode::Closed,
                "Application is shutting down",
            ));
        }
        self.admitted
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < CAPACITY).then_some(n + 1)
            })
            .map_err(|_| Fault::new(ErrorCode::Busy, "Control admission full; try again"))?;
        let permit = Permit(self.admitted.clone());
        let (tx, rx) = mpsc::sync_channel(1);
        let id = request.id;
        match self.tx.try_send(Envelope {
            request,
            reply: tx,
            _permit: permit,
        }) {
            Ok(()) => Ok(Pending { id, rx }),
            Err(TrySendError::Full(_)) => Err(Fault::new(ErrorCode::Busy, "Control queue full")),
            Err(TrySendError::Disconnected(_)) => {
                Err(Fault::new(ErrorCode::Closed, "Control worker closed"))
            }
        }
    }
}
pub struct Service {
    client: Client,
    worker: Option<JoinHandle<()>>,
}
impl Service {
    pub fn spawn() -> std::io::Result<Self> {
        Self::spawn_inner(|| {
            let path = default_path();
            let (profile, notice) = midi::runtime::load_profile(&path);
            let mut worker = Worker::new(profile, Some(path));
            worker.notice = notice;
            worker
        })
    }
    fn spawn_inner(initialize: impl FnOnce() -> Worker + Send + 'static) -> std::io::Result<Self> {
        let (tx, rx) = mpsc::sync_channel::<Envelope>(CAPACITY);
        let closed = Arc::new(AtomicBool::new(false));
        let stop = closed.clone();
        let worker = thread::Builder::new()
            .name("dawj-control".into())
            .spawn(move || {
                let mut worker = initialize();
                while !stop.load(Ordering::Acquire) {
                    match rx.recv_timeout(Duration::from_millis(8)) {
                        Ok(envelope) => {
                            if stop.load(Ordering::Acquire) {
                                break;
                            }
                            worker.poll();
                            let result = worker.handle(envelope.request.command);
                            // At most one reply fits. Receiver cancellation cannot block this worker.
                            let _ = envelope
                                .reply
                                .try_send(Reply::new(envelope.request.id, result));
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                    worker.poll();
                }
                worker.shutdown();
                // Dropping pending envelopes releases permits and wakes waiting callers as Closed.
            })?;
        Ok(Self {
            client: Client {
                tx,
                closed,
                admitted: Arc::new(AtomicUsize::new(0)),
            },
            worker: Some(worker),
        })
    }
    pub fn client(&self) -> Client {
        self.client.clone()
    }
    pub fn shutdown(&mut self) {
        self.client.closed.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.shutdown();
    }
}
struct Worker {
    output: Option<OutputSession>,
    devices: Vec<audio_backend::OutputDevice>,
    mapper: Mapper,
    midi: Devices,
    midi_state: DeviceStatus,
    dropped: u64,
    feedback_tick: Instant,
    path: Option<PathBuf>,
    notice: String,
}
impl Worker {
    fn new(profile: Profile, path: Option<PathBuf>) -> Self {
        let midi = Devices::new(profile.input.clone(), profile.output.clone());
        Self {
            output: None,
            devices: Vec::new(),
            mapper: Mapper::new(profile).expect("validated profile"),
            midi,
            midi_state: DeviceStatus::default(),
            dropped: 0,
            feedback_tick: Instant::now(),
            path,
            notice: String::new(),
        }
    }
    fn co(&mut self) -> Option<&mut Coordinator> {
        self.output.as_mut().map(|o| &mut o.coordinator)
    }
    fn poll(&mut self) {
        if let Some(o) = &mut self.output {
            o.coordinator.poll();
        }
        midi::runtime::poll(
            &mut self.mapper,
            &self.midi,
            &mut self.midi_state,
            &mut self.dropped,
            &mut self.feedback_tick,
            self.output.as_mut().map(|o| &mut o.coordinator),
        );
    }
    fn snapshot(&self) -> Snapshot {
        let mut s = self.output.as_ref().map_or_else(Snapshot::default, |o| {
            Snapshot::project(&o.coordinator, o.stream.monitor().status().failed)
        });
        s.midi = Some(self.midi_state.clone());
        s.midi_notice = format!("{} {}", self.notice, self.mapper.status);
        s
    }
    fn handle(&mut self, command: Command) -> Result<Response, Fault> {
        use ErrorCode::*;
        let revision = self.mapper.revision;
        match command {
            Command::Devices {} => {
                self.devices = output::discover().map_err(|e| Fault::new(Failed, e))?;
                return Ok(Response::Devices(
                    self.devices
                        .iter()
                        .map(|d| Device {
                            id: d.ordinal,
                            name: d.name.clone().unwrap_or_default(),
                        })
                        .collect(),
                ));
            }
            Command::Start {
                device,
                cue,
                phrase_bars,
            } => {
                if self.output.is_some() {
                    return Err(Fault::new(Invalid, "Stop existing output before starting"));
                }
                let device = self
                    .devices
                    .iter()
                    .find(|d| d.ordinal == device)
                    .ok_or_else(|| Fault::new(Invalid, "Rescan and select an output device"))?;
                self.output = Some(
                    OutputSession::open(device, cue, phrase_bars)
                        .map_err(|e| Fault::new(Failed, e))?,
                );
                self.mapper
                    .reset(self.output.as_mut().map(|o| &mut o.coordinator));
            }
            Command::Stop {} => {
                if let Some(mut o) = self.output.take() {
                    self.mapper.reset(Some(&mut o.coordinator));
                    o.stop().map_err(|e| Fault::new(Failed, e))?;
                }
            }
            Command::Snapshot {} => {}
            Command::Perform { action, source } => {
                if self
                    .output
                    .as_ref()
                    .is_some_and(|o| o.stream.monitor().status().failed)
                {
                    return Err(Fault::new(
                        Unavailable,
                        "Audio stream failed; stop output and start a fresh session",
                    ));
                }
                self.co()
                    .ok_or_else(|| Fault::new(Unavailable, "Output offline"))?
                    .dispatch(action.into(), source)
                    .map_err(|e| Fault::new(Invalid, e))?;
            }
            Command::ReleaseModes { source } => {
                if let Some(co) = self.co() {
                    co.release_modes(source);
                }
            }
            Command::MidiProfile {} => {
                return Ok(Response::MidiProfile(self.mapper.profile.clone()));
            }
            Command::MidiReplace { profile } => {
                self.mapper
                    .replace_profile(profile, self.output.as_mut().map(|o| &mut o.coordinator))
                    .map_err(|e| Fault::new(Invalid, e))?;
                self.mapper.dirty = true;
            }
            Command::MidiConnect { input, output } => {
                if input
                    .as_ref()
                    .is_some_and(|s| s.is_empty() || s.len() > 256)
                    || output
                        .as_ref()
                        .is_some_and(|s| s.is_empty() || s.len() > 256)
                {
                    return Err(Fault::new(Invalid, "MIDI port name exceeds limit"));
                }
                self.mapper
                    .reset(self.output.as_mut().map(|o| &mut o.coordinator));
                self.midi
                    .connect(input.clone(), output.clone())
                    .map_err(|e| Fault::new(Busy, e))?;
                self.mapper.profile.input = input;
                self.mapper.profile.output = output;
            }
            Command::MidiSave {} => {
                self.mapper
                    .profile
                    .save(
                        self.path
                            .as_ref()
                            .ok_or_else(|| Fault::new(Unavailable, "No profile path"))?,
                    )
                    .map_err(|e| Fault::new(Failed, e))?;
                self.mapper.dirty = false;
            }
            Command::MidiLoad {} => {
                let p = Profile::load(
                    self.path
                        .as_ref()
                        .ok_or_else(|| Fault::new(Unavailable, "No profile path"))?,
                )
                .map_err(|e| Fault::new(Failed, e))?;
                self.mapper
                    .replace_profile(p, self.output.as_mut().map(|o| &mut o.coordinator))
                    .map_err(|e| Fault::new(Invalid, e))?;
            }
            Command::MidiRescan {} => self.midi.rescan().map_err(|e| Fault::new(Busy, e))?,
            Command::MidiLearn { mapping, replace } => {
                // Validate the template and replacement before starting learn, with the existing profile rules.
                let mut p = self.mapper.profile.clone();
                if let Some(i) = replace {
                    let row = p
                        .mappings
                        .get_mut(i)
                        .ok_or_else(|| Fault::new(Invalid, "Invalid mapping index"))?;
                    *row = mapping.clone();
                } else {
                    p.mappings.push(mapping.clone());
                }
                p.validate().map_err(|e| Fault::new(Invalid, e))?;
                self.mapper.learn = Some(Learn { mapping, replace });
            }
            Command::MidiCancelLearn {} => self.mapper.learn = None,
        }
        if self.mapper.revision != revision {
            // Match the native settings panel: never reinterpret old queued packets under a new map.
            for _ in self
                .midi
                .events
                .try_iter()
                .take(midi::device::EVENT_CAPACITY)
            {}
        }
        self.poll();
        Ok(Response::Snapshot(Box::new(self.snapshot())))
    }
    fn shutdown(&mut self) {
        if let Some(o) = &mut self.output {
            self.mapper.reset(Some(&mut o.coordinator));
            let _ = o.stop();
        }
        self.output = None;
        midi::runtime::shutdown(&self.mapper, &mut self.midi);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(id: u32) -> Request {
        Request {
            version: VERSION,
            id,
            command: Command::Snapshot {},
        }
    }
    #[test]
    fn worker_is_independent_bounded_and_shutdown_bypasses_a_full_queue() {
        let caller = thread::current().id();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let release = barrier.clone();
        let mut service = Service::spawn_inner(move || {
            assert_ne!(thread::current().id(), caller);
            release.wait();
            Worker::new(Profile::default(), None)
        })
        .unwrap();
        let client = service.client();
        let pending: Vec<_> = (0..CAPACITY)
            .map(|id| client.submit(request(id as u32)).unwrap())
            .collect();
        assert_eq!(
            client.submit(request(99)).err().unwrap().code,
            ErrorCode::Busy
        );
        let mut wrong = request(100);
        wrong.version = VERSION + 1;
        assert_eq!(
            client.submit(wrong).err().unwrap().code,
            ErrorCode::UnsupportedVersion
        );
        service.client.closed.store(true, Ordering::Release);
        barrier.wait();
        service.shutdown();
        for p in pending {
            assert_eq!(p.wait().result.unwrap_err().code, ErrorCode::Closed);
        }
        assert_eq!(client.admitted.load(Ordering::Acquire), 0);
        assert_eq!(
            client.submit(request(101)).err().unwrap().code,
            ErrorCode::Closed
        );
    }
    #[test]
    fn offline_worker_responds_and_client_disconnect_does_not_strand_permits() {
        let mut service = Service::spawn_inner(|| Worker::new(Profile::default(), None)).unwrap();
        let client = service.client();
        drop(client.submit(request(1)).unwrap());
        let reply = client.submit(request(2)).unwrap().wait();
        assert_eq!(reply.id, 2);
        assert_eq!(reply.version, VERSION);
        let Response::Snapshot(s) = reply.result.unwrap() else {
            panic!("expected snapshot")
        };
        assert!(!s.online);
        assert!(s.decks.is_none());
        service.shutdown();
        assert_eq!(client.admitted.load(Ordering::Acquire), 0);
    }
}
