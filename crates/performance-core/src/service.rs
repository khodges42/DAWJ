// SPDX-License-Identifier: GPL-3.0-or-later
//! Frontend-neutral, bounded control worker. CPAL is constructed and dropped here.
use crate::{
    Coordinator, InputSource,
    ipc::*,
    midi::{
        self, Mapper,
        device::{DeviceStatus, Devices},
        profile::{Profile, default_path},
        profiles,
    },
    output::{self, OutputSession},
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
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
fn source_index(source: InputSource) -> usize {
    match source {
        InputSource::Gui => 0,
        InputSource::Keyboard => 1,
        InputSource::Midi => 2,
    }
}
#[derive(Default, Debug, serde::Serialize)]
pub struct WorkerTiming {
    pub queue_ms: f64,
    pub poll_ms: f64,
    pub handle_ms: f64,
}
struct Probe {
    admitted: Instant,
    tx: SyncSender<WorkerTiming>,
}
struct Envelope {
    probe: Option<Probe>,
    hold_epoch: Option<(InputSource, u64)>,
    request: Request,
    reply: SyncSender<Reply>,
    _permit: Permit,
}
#[derive(Clone)]
pub struct Client {
    tx: SyncSender<Envelope>,
    admitted: Arc<AtomicUsize>,
    closed: Arc<AtomicBool>,
    releases: Arc<[AtomicU64; 3]>,
}
pub struct Pending {
    id: u32,
    rx: Receiver<Reply>,
    timing: Option<Receiver<WorkerTiming>>,
}
impl Pending {
    /// Only call on a non-UI, non-audio worker. Dropped receivers do not cancel musical actions.
    pub fn wait_profiled(mut self) -> (Reply, Option<WorkerTiming>) {
        let timing = self.timing.take();
        let reply = self.wait();
        (reply, timing.and_then(|rx| rx.try_recv().ok()))
    }
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
        self.submit_inner(request, None, None)
    }
    /// Opt-in non-realtime diagnostic channel. Normal requests allocate no probe.
    pub fn submit_profiled(&self, request: Request) -> Result<Pending, Fault> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.submit_inner(
            request,
            Some(Probe {
                admitted: Instant::now(),
                tx,
            }),
            Some(rx),
        )
    }
    fn submit_inner(
        &self,
        request: Request,
        probe: Option<Probe>,
        timing: Option<Receiver<WorkerTiming>>,
    ) -> Result<Pending, Fault> {
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
        if let Command::ReleaseModes { source } = &request.command {
            self.releases[source_index(*source)].fetch_add(1, Ordering::AcqRel);
        }
        let hold_epoch = if let Command::Perform {
            action: Action::HoldMode { pressed: true, .. },
            source,
        } = &request.command
        {
            Some((
                *source,
                self.releases[source_index(*source)].load(Ordering::Acquire),
            ))
        } else {
            None
        };
        self.admitted
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < CAPACITY).then_some(n + 1)
            })
            .map_err(|_| Fault::new(ErrorCode::Busy, "Control admission full; try again"))?;
        let permit = Permit(self.admitted.clone());
        let (tx, rx) = mpsc::sync_channel(1);
        let id = request.id;
        match self.tx.try_send(Envelope {
            probe,
            hold_epoch,
            request,
            reply: tx,
            _permit: permit,
        }) {
            Ok(()) => Ok(Pending { id, rx, timing }),
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
        Self::spawn_factory(vec![
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/samples"),
        ])
    }
    pub fn spawn_factory(roots: Vec<PathBuf>) -> std::io::Result<Self> {
        Self::spawn_inner(move || {
            let path = default_path();
            let name = profiles::selected(&path);
            let selected = profiles::path(&path, &name).unwrap_or_else(|_| path.clone());
            let (profile, notice) = midi::runtime::load_profile(&selected);
            let missing = name != "default" && !selected.exists();
            let mut worker = Worker::new(profile, Some(path));
            worker.notice = notice;
            if missing {
                worker.notice = format!(
                    "Selected profile '{name}' is missing; factory mapping loaded in memory"
                );
                worker.mapper.dirty = true;
            }
            worker.profile_name = name;
            worker.factory_roots = roots;
            worker.start_index();
            worker
        })
    }
    fn spawn_inner(initialize: impl FnOnce() -> Worker + Send + 'static) -> std::io::Result<Self> {
        let (tx, rx) = mpsc::sync_channel::<Envelope>(CAPACITY);
        let closed = Arc::new(AtomicBool::new(false));
        let stop = closed.clone();
        let releases = Arc::new(std::array::from_fn(|_| AtomicU64::new(0)));
        let release_worker = releases.clone();
        let worker = thread::Builder::new()
            .name("dawj-control".into())
            .spawn(move || {
                let mut worker = initialize();
                while !stop.load(Ordering::Acquire) {
                    worker.apply_releases(&release_worker);
                    match rx.recv_timeout(Duration::from_millis(8)) {
                        Ok(envelope) => {
                            if stop.load(Ordering::Acquire) {
                                break;
                            }
                            let timing_start = envelope.probe.as_ref().map(|_| Instant::now());
                            let queue_ms = envelope
                                .probe
                                .as_ref()
                                .map_or(0.0, |p| p.admitted.elapsed().as_secs_f64() * 1000.0);
                            worker.poll();
                            worker.apply_releases(&release_worker);
                            let stale = envelope.hold_epoch.is_some_and(|(s, e)| {
                                release_worker[source_index(s)].load(Ordering::Acquire) != e
                            });
                            let poll_ms =
                                timing_start.map_or(0.0, |t| t.elapsed().as_secs_f64() * 1000.0);
                            let handling = envelope.probe.as_ref().map(|_| Instant::now());
                            let result = if stale {
                                Err(Fault::new(
                                    ErrorCode::Invalid,
                                    "Stale momentary press discarded after focus/release",
                                ))
                            } else {
                                worker.handle(envelope.request.command)
                            };
                            if let Some(probe) = envelope.probe {
                                let _ = probe.tx.try_send(WorkerTiming {
                                    queue_ms,
                                    poll_ms,
                                    handle_ms: handling
                                        .map_or(0.0, |t| t.elapsed().as_secs_f64() * 1000.0),
                                });
                            }
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
                releases,
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
struct Preparation {
    rx: Receiver<Result<crate::session::Prepared, String>>,
    project: crate::session::Project,
    affected: [bool; 2],
    path: Option<String>,
    opened: bool,
}
struct PreviewJob {
    velocity: u8,
    rx: Receiver<Result<audio_backend::samples::SampleVoice, String>>,
    key: String,
}
struct Worker {
    catalog: crate::factory::Catalog,
    factory_roots: Vec<PathBuf>,
    indexing: Option<Receiver<Result<crate::factory::Catalog, String>>>,
    preview_job: Option<PreviewJob>,
    preview_cache: std::collections::BTreeMap<String, u8>,
    undo: std::collections::VecDeque<crate::session::Project>,
    redo: std::collections::VecDeque<crate::session::Project>,
    history_pending: Option<(crate::session::Project, i8)>,
    jam: crate::session::Workspace,
    preparation: Option<Preparation>,
    control_revision: u64,
    seen_releases: [u64; 3],
    view: crate::timeline::View,
    profile_name: String,
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
            catalog: Default::default(),
            factory_roots: Vec::new(),
            indexing: None,
            preview_job: None,
            preview_cache: Default::default(),
            undo: Default::default(),
            redo: Default::default(),
            history_pending: None,
            jam: crate::session::Workspace::default(),
            preparation: None,
            control_revision: 0,
            seen_releases: [0; 3],
            view: crate::timeline::View::default(),
            profile_name: "default".into(),
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
    fn apply_releases(&mut self, epochs: &[AtomicU64; 3]) {
        for source in [InputSource::Gui, InputSource::Keyboard, InputSource::Midi] {
            let i = source_index(source);
            let now = epochs[i].load(Ordering::Acquire);
            if self.seen_releases[i] != now {
                if let Some(co) = self.co() {
                    co.release_modes(source);
                }
                self.seen_releases[i] = now;
            }
        }
    }
    fn co(&mut self) -> Option<&mut Coordinator> {
        self.output.as_mut().map(|o| &mut o.coordinator)
    }
    fn poll(&mut self) {
        self.poll_index();
        self.poll_preview();
        self.poll_preparation();
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
        if !self.jam.demo
            && !self.jam.dirty
            && self
                .output
                .as_ref()
                .is_some_and(|o| self.jam.changed(&o.coordinator))
        {
            self.jam.dirty = true;
        }
    }
    fn snapshot(&self) -> Snapshot {
        let mut s = self.output.as_ref().map_or_else(Snapshot::default, |o| {
            Snapshot::project(&o.coordinator, o.stream.monitor().status().failed)
        });
        s.timeline = self.output.as_ref().and_then(|o| {
            crate::web_timeline::Projection::from_coordinator(&o.coordinator, &self.view)
        });
        if self.output.is_none() && !self.jam.demo {
            let c = &self.jam.project.settings;
            s.crossfader = c.crossfader;
            s.deck_gain = c.deck_gain;
            s.output_trim_db = c.output_trim_db;
        }
        s.catalog_loading = self.catalog.loading;
        s.undo = self.undo.len();
        s.redo = self.redo.len();
        s.session = Some(self.jam.clone());
        s.midi_revision = self.mapper.revision;
        s.midi_dirty = self.mapper.dirty;
        s.midi_learning = self.mapper.learn.is_some();
        s.midi_dropped = self.dropped;
        s.profile_name = self.profile_name.clone();
        s.profile_path = self
            .path
            .as_ref()
            .and_then(|p| profiles::path(p, &self.profile_name).ok())
            .map_or(String::new(), |p| p.display().to_string());
        s.midi = Some(self.midi_state.clone());
        s.control_revision = self.control_revision;
        s.midi_notice = format!("{} {}", self.notice, self.mapper.status);
        s
    }
    fn handle(&mut self, command: Command) -> Result<Response, Fault> {
        self.control_revision = self.control_revision.wrapping_add(1);
        use ErrorCode::*;
        let revision = self.mapper.revision;
        match command {
            Command::Catalog { rescan } => {
                if rescan {
                    self.start_index();
                }
                return Ok(Response::Catalog(self.catalog.clone()));
            }
            Command::Audition {
                velocity,
                reference,
                deck,
                slot,
                stop,
            } => self
                .audition(reference, deck, slot, stop, velocity)
                .map_err(|e| Fault::new(Invalid, e))?,
            Command::Session { action } => self
                .session_action(action)
                .map_err(|e| Fault::new(Invalid, e))?,
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
                if self.jam.busy {
                    return Err(Fault::new(Busy, "Wait for content preparation"));
                }
                if self.jam.demo {
                    self.output = Some(
                        OutputSession::open(device, cue, phrase_bars)
                            .map_err(|e| Fault::new(Failed, e))?,
                    );
                } else {
                    self.jam.project.settings.phrase_bars = phrase_bars;
                    let (output, media) = OutputSession::open_project_catalog(
                        device,
                        cue,
                        &self.jam.project,
                        Some(&self.catalog),
                    )
                    .map_err(|e| Fault::new(Failed, e))?;
                    self.jam.media = media;
                    self.output = Some(output);
                    self.jam.message =
                        "Output ready. Saved periodic definitions require explicit re-arming."
                            .into();
                }
                self.mapper
                    .reset(self.output.as_mut().map(|o| &mut o.coordinator));
            }
            Command::Stop {} => {
                self.preview_cache.clear();
                if let Some(mut o) = self.output.take() {
                    if !self.jam.demo {
                        self.jam.capture(&o.coordinator);
                    }
                    self.mapper.reset(Some(&mut o.coordinator));
                    o.stop().map_err(|e| Fault::new(Failed, e))?;
                }
            }
            Command::Snapshot {} => {}
            Command::TimelineView {
                zoom_bars,
                follow,
                page_delta,
            } => {
                if ![2, 4, 8].contains(&zoom_bars) || !(-1..=1).contains(&page_delta) {
                    return Err(Fault::new(Invalid, "Invalid timeline view"));
                }
                let s = self
                    .co()
                    .and_then(|c| c.snapshot)
                    .ok_or_else(|| Fault::new(Unavailable, "No musical snapshot"))?;
                self.view.zoom_bars = zoom_bars;
                self.view.set_follow(follow, &s);
                if page_delta != 0 {
                    self.view.page_by(&s, i32::from(page_delta));
                }
            }
            Command::MidiProfiles {} => {
                return Ok(Response::MidiProfiles(
                    profiles::list(
                        self.path
                            .as_ref()
                            .ok_or_else(|| Fault::new(Unavailable, "No profile directory"))?,
                    )
                    .map_err(|e| Fault::new(Failed, e))?,
                ));
            }
            Command::MidiSelectProfile { name } => {
                let default = self
                    .path
                    .as_ref()
                    .ok_or_else(|| Fault::new(Unavailable, "No profile directory"))?;
                let path = profiles::path(default, &name).map_err(|e| Fault::new(Invalid, e))?;
                let profile = Profile::load(&path).map_err(|e| Fault::new(Failed, e))?;
                profiles::remember(default, &name).map_err(|e| Fault::new(Failed, e))?;
                self.mapper
                    .replace_profile(profile, self.output.as_mut().map(|o| &mut o.coordinator))
                    .map_err(|e| Fault::new(Invalid, e))?;
                self.profile_name = name;
            }
            Command::MidiSaveAs { name } => {
                profiles::save(
                    self.path
                        .as_ref()
                        .ok_or_else(|| Fault::new(Unavailable, "No profile directory"))?,
                    &name,
                    &self.mapper.profile,
                )
                .map_err(|e| Fault::new(Invalid, e))?;
                self.profile_name = name;
                self.mapper.dirty = false;
            }
            Command::MidiFactory {} => {
                self.mapper
                    .replace_profile(
                        Profile::default(),
                        self.output.as_mut().map(|o| &mut o.coordinator),
                    )
                    .map_err(|e| Fault::new(Invalid, e))?;
                self.mapper.dirty = true;
            }
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
            Command::MidiReplace {
                profile,
                expected_revision,
            } => {
                if expected_revision.is_some_and(|r| r != self.mapper.revision) {
                    return Err(Fault::new(
                        Invalid,
                        "MIDI profile changed; reload before editing",
                    ));
                }
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
                self.mapper.dirty = true;
                self.mapper.revision = self.mapper.revision.wrapping_add(1);
            }
            Command::MidiSave {} => {
                self.mapper
                    .profile
                    .save(
                        &profiles::path(
                            self.path
                                .as_ref()
                                .ok_or_else(|| Fault::new(Unavailable, "No profile path"))?,
                            &self.profile_name,
                        )
                        .map_err(|e| Fault::new(Invalid, e))?,
                    )
                    .map_err(|e| Fault::new(Failed, e))?;
                self.mapper.dirty = false;
            }
            Command::MidiLoad {} => {
                let p = Profile::load(
                    &profiles::path(
                        self.path
                            .as_ref()
                            .ok_or_else(|| Fault::new(Unavailable, "No profile path"))?,
                        &self.profile_name,
                    )
                    .map_err(|e| Fault::new(Invalid, e))?,
                )
                .map_err(|e| Fault::new(Failed, e))?;
                self.mapper
                    .replace_profile(p, self.output.as_mut().map(|o| &mut o.coordinator))
                    .map_err(|e| Fault::new(Invalid, e))?;
            }
            Command::MidiRescan {} => self.midi.rescan().map_err(|e| Fault::new(Busy, e))?,
            Command::MidiLearn { mapping, replace } => {
                self.mapper
                    .begin_learn(mapping, replace)
                    .map_err(|e| Fault::new(Invalid, e))?;
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
    fn session_action(&mut self, action: crate::session::Action) -> Result<(), String> {
        use crate::session::Action;
        if self.jam.busy {
            return Err("Media preparation in progress".into());
        }
        let direction = match action {
            Action::Undo {} => 1,
            Action::Redo {} => 2,
            Action::New {}
            | Action::Demo {}
            | Action::Open { .. }
            | Action::Save { .. }
            | Action::ExportComposition { .. } => -1,
            _ => 0,
        };
        let before = self.jam.project.clone();
        let revision = self.jam.revision;
        self.session_inner(action)?;
        if direction >= 0 {
            if self.preparation.is_some() {
                self.history_pending = Some((before, direction));
            } else if self.jam.revision != revision {
                self.commit_history(before, direction);
            }
        }
        Ok(())
    }
    fn commit_history(&mut self, before: crate::session::Project, direction: i8) {
        if direction == 1 {
            self.undo.pop_back();
            self.redo.push_back(before);
        } else if direction == 2 {
            self.redo.pop_back();
            self.undo.push_back(before);
        } else {
            self.undo.push_back(before);
            self.redo.clear();
        }
        while self.undo.len() > 32 {
            self.undo.pop_front();
        }
        while self.redo.len() > 32 {
            self.redo.pop_front();
        }
    }
    fn session_inner(&mut self, action: crate::session::Action) -> Result<(), String> {
        use crate::session::{self, Action::*};
        if self.jam.busy {
            return Err(
                "Content preparation in progress; performance controls remain available".into(),
            );
        }
        if let Some(o) = &self.output
            && !self.jam.demo
        {
            self.jam.capture(&o.coordinator);
        }
        match action {
            New {} | Demo {} => {
                if self.output.is_some() {
                    return Err("Stop output before replacing the whole session".into());
                }
                let demo = matches!(action, Demo {});
                self.undo.clear();
                self.redo.clear();
                self.preview_cache.clear();
                self.jam = session::Workspace {
                    demo,
                    message: if demo {
                        "Bundled demonstration selected; Save is for user sessions".into()
                    } else {
                        "Empty session. Create a composition for either deck.".into()
                    },
                    ..Default::default()
                };
                return Ok(());
            }
            Open { path } => {
                if self.output.is_some() {
                    return Err("Stop output before opening a session".into());
                }
                let path = session::absolute(std::path::Path::new(&path))?;
                let project = session::Project::load(&path)?;
                return self.begin_preparation(
                    project,
                    [true; 2],
                    false,
                    Some(path.display().to_string()),
                    true,
                    None,
                );
            }
            Save { path } => {
                if self.jam.demo {
                    return Err("Create a user session to save local compositions".into());
                }
                let path = path
                    .or_else(|| self.jam.path.clone())
                    .ok_or("Use Save As to choose a session path")?;
                let path = session::absolute(std::path::Path::new(&path))?;
                self.jam.project.save(&path)?;
                self.jam.path = Some(path.display().to_string());
                self.jam.dirty = false;
                self.jam.message =
                    "Session saved. Playback position and pending actions are not restored.".into();
                return Ok(());
            }
            _ => {}
        }
        if self.jam.demo {
            return Err("Stop output and choose New session before editing user content".into());
        }
        if let Undo {} | Redo {} = action {
            let candidate = if matches!(action, Undo {}) {
                self.undo.back()
            } else {
                self.redo.back()
            }
            .ok_or("No edit to undo/redo")?
            .clone();
            return self.apply_project(candidate, None);
        }
        if let ExportComposition { composition, path } = action {
            let c = self.jam.project.composition(composition)?.clone();
            let mut p = self.jam.project.clone();
            p.compositions = vec![c];
            p.decks = [Some(composition), None];
            p.save(std::path::Path::new(&path))?;
            self.jam.message = "Composition saved separately".into();
            return Ok(());
        }
        let mut project = self.jam.project.clone();
        let force = if let Import {
            composition, slot, ..
        } = &action
        {
            Some((*composition, *slot))
        } else {
            None
        };
        match action {
            Variation {
                composition,
                slot,
                index,
                store,
            } => {
                project
                    .composition_mut(composition)?
                    .parts
                    .get_mut(slot)
                    .ok_or("Invalid slot")?
                    .variation(index, store)?;
            }
            Pattern {
                composition,
                slot,
                pattern,
            } => {
                let p = project
                    .composition_mut(composition)?
                    .parts
                    .get_mut(slot)
                    .ok_or("Invalid slot")?;
                p.pattern = pattern;
                p.variation = None;
                if pattern.is_some() {
                    p.rule = None;
                }
            }
            Factory {
                composition,
                slot,
                reference,
            } => {
                reference.validate()?;
                let entry = self
                    .catalog
                    .entries
                    .iter()
                    .find(|e| e.reference == reference)
                    .ok_or("Rescan factory catalog")?;
                let hints = project
                    .sample_overrides
                    .get(&reference.key())
                    .unwrap_or(&entry.hints)
                    .clone();
                let p = project
                    .composition_mut(composition)?
                    .parts
                    .get_mut(slot)
                    .ok_or("Invalid slot")?;
                p.factory = Some(reference);
                p.media = None;
                p.looping = hints.kind == "Loop";
                p.name = hints.display_name.clone();
                p.hints = Some(hints);
                p.sample_start = 0;
                p.sample_end = None;
                p.loop_start = 0;
                p.loop_end = None;
                p.pattern = None;
                p.variation = None;
                p.variations = [None; 4];
                p.rule = None;
            }
            Correct { reference, hints } => {
                reference.validate()?;
                hints.validate()?;
                project
                    .sample_overrides
                    .insert(reference.key(), hints.clone());
                for c in &mut project.compositions {
                    for p in &mut c.parts {
                        if p.factory.as_ref() == Some(&reference) {
                            p.hints = Some(hints.clone());
                        }
                    }
                }
            }
            Duplicate {
                composition,
                deck,
                name,
            } => {
                if deck >= 2 || project.compositions.len() >= 8 {
                    return Err("Invalid deck or full session".into());
                }
                let mut c = project.composition(composition)?.clone();
                c.id = session::id();
                c.name = name;
                for p in &mut c.parts {
                    p.id = session::id();
                }
                project.decks[deck] = Some(c.id);
                project.compositions.push(c);
            }
            OpenComposition { deck, path } => {
                if deck >= 2 || project.compositions.len() >= 8 {
                    return Err("Invalid deck or full session".into());
                }
                let imported = session::Project::load(std::path::Path::new(&path))?;
                if imported.compositions.len() != 1 {
                    return Err(
                        "Open a single-composition export here; use Open session for full projects"
                            .into(),
                    );
                }
                let c = imported.compositions[0].clone();
                if project.compositions.iter().any(|old| old.id == c.id) {
                    return Err("Composition is already present; select it or use Duplicate".into());
                }
                project.decks[deck] = Some(c.id);
                project.compositions.push(c);
                for (k, v) in imported.sample_overrides {
                    project.sample_overrides.entry(k).or_insert(v);
                }
            }
            Rename { name } => project.name = name,
            Create { name, deck } => {
                project.create(name, deck)?;
            }
            RenameComposition { composition, name } => {
                project.composition_mut(composition)?.name = name
            }
            Assign { deck, composition } => {
                if deck >= 2 {
                    return Err("Invalid deck".into());
                }
                if let Some(n) = composition {
                    project.composition(n)?;
                }
                project.decks[deck] = composition;
            }
            Import {
                composition,
                slot,
                path,
            } => {
                if !project.decks.contains(&Some(composition)) {
                    return Err("Load the composition on a deck before importing".into());
                }
                let c = project.composition_mut(composition)?;
                let part = c.parts.get_mut(slot).ok_or("Invalid part slot")?;
                let path = session::absolute(std::path::Path::new(&path))?;
                part.name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .chars()
                    .take(80)
                    .collect();
                part.media = Some(path.display().to_string());
                part.factory = None;
                part.pattern = None;
                part.variation = None;
                part.variations = [None; 4];
                part.looping = false;
                part.sample_start = 0;
                part.sample_end = None;
                part.hints = Some(crate::factory::hints(
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .as_ref(),
                    "",
                ));
                part.loop_start = 0;
                part.loop_end = None;
            }
            Remove { composition, slot } => {
                let p = project
                    .composition_mut(composition)?
                    .parts
                    .get_mut(slot)
                    .ok_or("Invalid part slot")?;
                p.media = None;
                p.factory = None;
                p.pattern = None;
                p.variation = None;
                p.variations = [None; 4];
                p.hints = None;
                p.rule = None;
                p.loop_start = 0;
                p.loop_end = None;
            }
            Edit {
                composition,
                slot,
                part,
            } => {
                let old = project
                    .composition_mut(composition)?
                    .parts
                    .get_mut(slot)
                    .ok_or("Invalid part slot")?;
                if old.id != part.id
                    || old.media != part.media
                    || old.factory != part.factory
                    || old.pattern != part.pattern
                {
                    return Err(
                        "Use Import/Remove to change media; identities cannot be edited".into(),
                    );
                }
                // Authoring changes playback metadata. Live shaping/rules retain
                // their current accepted targets through the semantic actions.
                *old = crate::session::Sound {
                    gain: old.gain,
                    quantum_ticks: old.quantum_ticks,
                    cutoff_hz: old.cutoff_hz,
                    rule: old.rule,
                    ..*part
                };
            }
            _ => return Err("Unsupported session operation".into()),
        }
        self.apply_project(project, force)
    }
    fn apply_project(
        &mut self,
        project: crate::session::Project,
        force: Option<(u64, usize)>,
    ) -> Result<(), String> {
        project.validate()?;
        let affected = std::array::from_fn(|d| {
            let before =
                self.jam.project.decks[d].and_then(|n| self.jam.project.composition(n).ok());
            let after = project.decks[d].and_then(|n| project.composition(n).ok());
            force.is_some_and(|(n, _)| project.decks[d] == Some(n))
                || serde_json::to_vec(&before).ok() != serde_json::to_vec(&after).ok()
        });
        if !affected.into_iter().any(|b| b) {
            self.jam.project = project;
            self.jam.dirty = true;
            self.jam.revision += 1;
            return Ok(());
        }
        let voice_changes = crate::session::change_mask(&project, &self.jam.project, affected);
        if force.is_none() && !voice_changes.iter().flatten().any(|b| *b) {
            if let Some(o) = &mut self.output {
                let mut patterns = Vec::new();
                for (d, affected) in affected.into_iter().enumerate() {
                    if affected {
                        let next = project.decks[d].and_then(|n| project.composition(n).ok());
                        let old = self.jam.project.decks[d]
                            .and_then(|n| self.jam.project.composition(n).ok());
                        for s in 0..parts::SLOTS {
                            let p = next.and_then(|c| c.parts[s].pattern);
                            if p != old.and_then(|c| c.parts[s].pattern) {
                                patterns.push((d, s, p));
                            }
                            o.coordinator.names[d][s] = next.map_or_else(
                                || format!("Part {}", s + 1),
                                |c| c.parts[s].name.clone(),
                            );
                        }
                    }
                }
                o.coordinator
                    .set_patterns(&patterns)
                    .map_err(str::to_string)?;
            }
            self.jam.project = project;
            self.jam.dirty = true;
            self.jam.revision += 1;
            self.jam.message =
                "Edit accepted; pattern execution follows the engine's published boundary".into();
            return Ok(());
        }
        self.begin_preparation(project, affected, true, self.jam.path.clone(), false, force)
    }
    fn editable(&self, affected: [bool; 2]) -> bool {
        self.output.as_ref().is_none_or(|o| {
            affected
                .into_iter()
                .enumerate()
                .all(|(d, a)| !a || o.coordinator.deck_editable(d))
        })
    }
    fn begin_preparation(
        &mut self,
        project: crate::session::Project,
        affected: [bool; 2],
        strict: bool,
        path: Option<String>,
        opened: bool,
        force: Option<(u64, usize)>,
    ) -> Result<(), String> {
        if !self.editable(affected) {
            return Err("Stop every part, cancel pending actions and remove periodic rules on the destination deck before changing its content. The other deck can keep playing.".into());
        }
        let (tx, rx) = mpsc::sync_channel(1);
        let candidate = project.clone();
        let mut before = self.jam.project.clone();
        if let Some((n, s)) = force {
            before.composition_mut(n)?.parts[s].media = None;
        }
        let catalog = self.catalog.clone();
        thread::Builder::new()
            .name("dawj-media-prepare".into())
            .spawn(move || {
                let result = if opened {
                    crate::session::prepare_catalog(&candidate, None, affected, strict, &catalog)
                } else {
                    crate::session::prepare_catalog(
                        &candidate,
                        Some(&before),
                        affected,
                        strict,
                        &catalog,
                    )
                };
                let _ = tx.send(result);
            })
            .map_err(|e| e.to_string())?;
        self.preparation = Some(Preparation {
            rx,
            project,
            affected,
            path,
            opened,
        });
        self.jam.busy = true;
        self.jam.message = "Preparing media in background; assignment is not committed yet".into();
        Ok(())
    }
    fn poll_preparation(&mut self) {
        let Some(job) = &self.preparation else { return };
        let result = match job.rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(_) => Err("Media preparation worker closed".into()),
        };
        let job = self.preparation.take().expect("preparation exists");
        self.jam.busy = false;
        if let Err(e) = result.and_then(|prepared| self.commit_preparation(job, prepared)) {
            self.history_pending = None;
            self.jam.message = format!("Content unchanged: {e}");
        }
    }
    fn commit_preparation(
        &mut self,
        job: Preparation,
        prepared: crate::session::Prepared,
    ) -> Result<(), String> {
        if !self.editable(job.affected) {
            return Err("Destination became active while preparing. Stop it and retry.".into());
        }
        let mut project = job.project;
        if let Some(o) = &self.output {
            let mut live = self.jam.clone();
            live.capture(&o.coordinator);
            project.settings = live.project.settings.clone();
            for (d, affected) in job.affected.into_iter().enumerate() {
                if !affected
                    && let Some(n) = project.decks[d]
                    && let (Ok(old), Ok(next)) =
                        (live.project.composition(n), project.composition_mut(n))
                {
                    *next = old.clone();
                }
            }
        }
        for (d, affected) in job.affected.into_iter().enumerate() {
            if affected {
                for s in 0..parts::SLOTS {
                    if project.decks[d]
                        .and_then(|n| project.composition(n).ok())
                        .is_none_or(|c| c.parts[s].media.is_none() && c.parts[s].factory.is_none())
                    {
                        project.settings.part_targets[d][s] = None;
                    }
                }
            }
        }
        let changed = prepared.changed;
        let media = if let Some(o) = &mut self.output {
            crate::session::install(&mut o.coordinator, &project, prepared, job.affected)?
        } else {
            prepared.media
        };
        self.jam.media.retain(|m| !changed[m.deck][m.slot]);
        self.jam.media.extend(media);
        if job.opened {
            self.undo.clear();
            self.redo.clear();
        } else if let Some((before, direction)) = self.history_pending.take() {
            self.commit_history(before, direction);
        }
        self.jam.project = project;
        self.jam.path = job.path;
        self.jam.dirty = !job.opened;
        self.jam.demo = false;
        self.jam.revision += 1;
        let missing = self.jam.media.iter().filter(|m| m.error.is_some()).count();
        self.jam.message = if missing > 0 {
            format!("Session opened with {missing} unavailable sounds. Repair paths using Import.")
        } else {
            "Content ready. Destination parts are stopped.".into()
        };
        Ok(())
    }
    fn start_index(&mut self) {
        if self.indexing.is_some() {
            return;
        }
        let roots = self.factory_roots.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        self.catalog.loading = true;
        self.preview_cache.clear();
        match thread::Builder::new()
            .name("dawj-factory-index".into())
            .spawn(move || {
                let root = roots
                    .iter()
                    .find(|p| p.is_dir())
                    .cloned()
                    .or_else(|| roots.first().cloned())
                    .unwrap_or_default();
                let _ = tx.send(crate::factory::Catalog::discover(&root));
            }) {
            Ok(_) => self.indexing = Some(rx),
            Err(e) => {
                self.catalog.loading = false;
                self.catalog.warnings.push(e.to_string());
            }
        }
    }
    fn poll_index(&mut self) {
        let Some(rx) = &self.indexing else { return };
        match rx.try_recv() {
            Ok(Ok(c)) => {
                self.catalog = c;
                self.indexing = None;
            }
            Ok(Err(e)) => {
                self.catalog = crate::factory::Catalog {
                    warnings: vec![e],
                    ..Default::default()
                };
                self.indexing = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.catalog.loading = false;
                self.catalog.warnings.push("Index worker closed".into());
                self.indexing = None;
            }
            Err(_) => {}
        }
    }
    fn audition(
        &mut self,
        reference: Option<crate::factory::Reference>,
        deck: Option<usize>,
        slot: Option<usize>,
        stop: bool,
        velocity: u8,
    ) -> Result<(), String> {
        if stop {
            return self
                .co()
                .ok_or("Start output for audition")?
                .preview(None)
                .map_err(str::to_string);
        }
        if self.jam.busy {
            return Err("Media preparation in progress".into());
        }
        let mut sound = if let Some(r) = reference {
            r.validate()?;
            let mut s = crate::session::Sound::empty(0);
            s.factory = Some(r);
            s
        } else {
            let (d, s) = (deck.ok_or("Choose a sound")?, slot.ok_or("Choose a part")?);
            if d >= 2 || s >= parts::SLOTS {
                return Err("Invalid audition address".into());
            }
            self.jam
                .project
                .composition(self.jam.project.decks[d].ok_or("Empty deck")?)?
                .parts[s]
                .clone()
        };
        if self.output.is_none() {
            return Err("Select/start an audio output for audition".into());
        }
        let key = format!(
            "{}|{}|{:?}",
            sound.factory.as_ref().map_or_else(
                || sound.media.clone().unwrap_or_default(),
                crate::factory::Reference::key
            ),
            sound.sample_start,
            sound.sample_end
        );
        if let Some(bank) = self.preview_cache.get(&key).copied() {
            return self
                .co()
                .ok_or("Output offline")?
                .preview_velocity(Some(bank), velocity)
                .map_err(str::to_string);
        }
        let catalog = self.catalog.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        sound.id = crate::session::id();
        thread::Builder::new()
            .name("dawj-audition-prepare".into())
            .spawn(move || {
                let result = (|| {
                    let path = if let Some(r) = &sound.factory {
                        catalog.resolve(r)?
                    } else {
                        PathBuf::from(sound.media.as_ref().ok_or("No sound assigned")?)
                    };
                    let asset = parts::AssetId(crate::session::id());
                    let (sample, _) =
                        audio_backend::samples::LoadedSample::prepare_file(asset, &path, 48000)?;
                    let part = parts::Part {
                        id: parts::PartId(sound.id),
                        playback: parts::PlaybackConfiguration {
                            asset,
                            mode: parts::PlaybackMode::OneShot,
                        },
                        timing: parts::TimingPolicy {
                            nominal_length: timebase::BeatTicks(3840),
                            quantum: timebase::BeatTicks(960),
                        },
                    };
                    audio_backend::samples::SampleVoice::prepare_region(
                        part,
                        sample,
                        48000,
                        audio_backend::samples::BoundaryPolicy::Fade { frames: 64 },
                        sound.sample_start,
                        sound.sample_end,
                    )
                    .map_err(str::to_string)
                })();
                let _ = tx.send(result);
            })
            .map_err(|e| e.to_string())?;
        self.preview_job = Some(PreviewJob { rx, key, velocity });
        self.jam.busy = true;
        self.jam.message = "Preparing audition; playback not yet acknowledged".into();
        Ok(())
    }
    fn poll_preview(&mut self) {
        let Some(job) = &self.preview_job else { return };
        let result = match job.rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(_) => Err("Audition preparation closed".into()),
        };
        let job = self.preview_job.take().expect("preview exists");
        self.jam.busy = false;
        let result = result.and_then(|voice| {
            let co = self.co().ok_or("Output closed while preparing audition")?;
            let bank = co.install_voices(vec![voice]).map_err(str::to_string)?[0];
            co.preview_velocity(Some(bank), job.velocity)
                .map_err(str::to_string)?;
            self.preview_cache.insert(job.key, bank);
            Ok(())
        });
        if let Err(e) = result {
            self.jam.message = format!("Audition unavailable: {e}");
        } else {
            self.jam.message =
                "Audition submitted to shared MASTER; no isolated headphone preview".into();
        }
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
    fn settle_content(w: &mut Worker) {
        let start = Instant::now();
        while w.jam.busy {
            w.poll_preparation();
            assert!(start.elapsed() < Duration::from_secs(2));
            thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn jam_authoring_rejects_bad_imports_transactionally_and_rechecks_same_path() {
        use crate::session::Action;
        let root = std::env::temp_dir().join(format!(
            "dawj-jam-worker-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("own.wav");
        let mut w = Worker::new(Profile::default(), None);
        assert!(!w.jam.demo && w.jam.project.compositions.is_empty());
        w.session_action(Action::Create {
            name: "My composition".into(),
            deck: 0,
        })
        .unwrap();
        settle_content(&mut w);
        let n = w.jam.project.decks[0].unwrap();
        let revision = w.jam.revision;
        w.session_action(Action::Import {
            composition: n,
            slot: 0,
            path: path.display().to_string(),
        })
        .unwrap();
        assert!(
            w.session_action(Action::Rename {
                name: "Rejected during preparation".into()
            })
            .is_err()
        );
        assert!(matches!(
            w.handle(Command::Snapshot {}),
            Ok(Response::Snapshot(_))
        ));
        settle_content(&mut w);
        assert_eq!(w.jam.revision, revision);
        assert!(
            w.jam.project.composition(n).unwrap().parts[0]
                .media
                .is_none()
        );
        assert!(w.jam.message.starts_with("Content unchanged:"));
        let mut wav = b"RIFF".to_vec();
        wav.extend(100u32.to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(48000u32.to_le_bytes());
        wav.extend(96000u32.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(64u32.to_le_bytes());
        wav.extend([0u8; 64]);
        std::fs::write(&path, wav).unwrap();
        w.session_action(Action::Import {
            composition: n,
            slot: 0,
            path: path.display().to_string(),
        })
        .unwrap();
        settle_content(&mut w);
        assert_eq!(w.jam.media[0].sample_rate, 48000);
        let revision = w.jam.revision;
        std::fs::write(&path, b"unsupported MP3 or malformed WAV").unwrap();
        w.session_action(Action::Import {
            composition: n,
            slot: 0,
            path: path.display().to_string(),
        })
        .unwrap();
        settle_content(&mut w);
        assert_eq!(w.jam.revision, revision);
        assert!(w.jam.message.starts_with("Content unchanged:"));
        w.shutdown();
        assert!(root.is_absolute() && root.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn stalled_media_job_cannot_block_control_snapshots_or_shutdown() {
        let mut w = Worker::new(Profile::default(), None);
        let (_tx, rx) = mpsc::sync_channel(1);
        w.preparation = Some(Preparation {
            rx,
            project: w.jam.project.clone(),
            affected: [true, false],
            path: None,
            opened: false,
        });
        w.jam.busy = true;
        let start = Instant::now();
        let Response::Snapshot(s) = w.handle(Command::Snapshot {}).unwrap() else {
            panic!("snapshot")
        };
        assert!(s.session.unwrap().busy);
        w.shutdown();
        assert!(start.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn full_queue_release_latches_and_invalidates_an_older_momentary_press() {
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let release = barrier.clone();
        let mut service = Service::spawn_inner(move || {
            release.wait();
            Worker::new(Profile::default(), None)
        })
        .unwrap();
        let client = service.client();
        let hold = client
            .submit(Request {
                version: VERSION,
                id: 1,
                command: Command::Perform {
                    action: Action::HoldMode {
                        token: 1,
                        mode: crate::Mode::Filter,
                        pressed: true,
                    },
                    source: InputSource::Keyboard,
                },
            })
            .unwrap();
        let rest: Vec<_> = (1..CAPACITY)
            .map(|i| client.submit(request(i as u32 + 1)).unwrap())
            .collect();
        let result = client.submit(Request {
            version: VERSION,
            id: 99,
            command: Command::ReleaseModes {
                source: InputSource::Keyboard,
            },
        });
        assert_eq!(result.err().unwrap().code, ErrorCode::Busy);
        assert_eq!(client.releases[1].load(Ordering::Acquire), 1);
        barrier.wait();
        let error = hold.wait().result.unwrap_err();
        assert_eq!(error.code, ErrorCode::Invalid);
        assert!(error.message.contains("Stale momentary"));
        for p in rest {
            p.wait().result.unwrap();
        }
        service.shutdown();
    }
    #[test]
    fn named_profiles_persist_roundtrip_and_stale_editor_changes_reject() {
        let root = std::env::temp_dir().join(format!(
            "dawj-web2-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let default = root.join("midi.json");
        let mut w = Worker::new(Profile::default(), Some(default.clone()));
        w.handle(Command::MidiSaveAs {
            name: "live-set".into(),
        })
        .unwrap();
        assert_eq!(profiles::selected(&default), "live-set");
        let disk = Profile::load(&profiles::path(&default, "live-set").unwrap()).unwrap();
        assert_eq!(disk.mappings.len(), w.mapper.profile.mappings.len());
        assert!(
            profiles::list(&default)
                .unwrap()
                .contains(&"live-set".into())
        );
        for name in ["../escape", "C:\\escape", "", "a/b", "CON", "COM1"] {
            assert!(profiles::path(&default, name).is_err());
        }
        let old = w.mapper.revision;
        w.handle(Command::MidiFactory {}).unwrap();
        let error = w
            .handle(Command::MidiReplace {
                profile: disk,
                expected_revision: Some(old),
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Invalid);
        w.handle(Command::MidiSelectProfile {
            name: "live-set".into(),
        })
        .unwrap();
        assert_eq!(w.profile_name, "live-set");
        w.handle(Command::MidiSave {}).unwrap();
        w.handle(Command::MidiLoad {}).unwrap();
        let Response::Snapshot(s) = w.handle(Command::Snapshot {}).unwrap() else {
            panic!("snapshot")
        };
        assert_eq!(s.profile_name, "live-set");
        assert!(!s.midi_dirty);
        w.shutdown();
        // The test exclusively created this unique directory under the system temp root.
        assert!(root.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(root).unwrap();
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
    #[test]
    fn opt_in_timing_preserves_reply_and_releases_bounded_admission() {
        let mut service = Service::spawn_inner(|| Worker::new(Profile::default(), None)).unwrap();
        let client = service.client();
        let (reply, timing) = client.submit_profiled(request(17)).unwrap().wait_profiled();
        assert_eq!(reply.id, 17);
        assert!(reply.result.is_ok());
        let timing = timing.unwrap();
        assert!(timing.queue_ms >= 0.0 && timing.poll_ms >= 0.0 && timing.handle_ms >= 0.0);
        service.shutdown();
        assert_eq!(client.admitted.load(Ordering::Acquire), 0);
    }
}
