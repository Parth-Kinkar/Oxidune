use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::error::{Error, Result};

use super::{AudioOutput, CpalOutput, Decoder, StreamSpec, SymphoniaDecoder};

const TICK: Duration = Duration::from_millis(5);
const POSITION_INTERVAL: Duration = Duration::from_millis(100);

/// What the UI can ask the player to do.
#[derive(Debug)]
pub enum Command {
    Play(PathBuf),
    /// The track that follows the current one (None if nothing does).
    /// Lets the player prepare it so the switch is gapless.
    SetNext(Option<PathBuf>),
    Pause,
    Resume,
    Seek(Duration),
    Stop,
    Quit,
}

/// What the player tells the UI.
#[derive(Debug)]
pub enum Event {
    Started(StreamSpec),
    Position(Duration),
    /// Playback moved on to the preloaded next track without a gap.
    Advanced(PathBuf),
    /// The track played to its end and nothing was ready to follow it.
    Finished,
    /// Playback was stopped on request.
    Stopped,
    Error(String),
}

/// Handle to the playback thread. This is all the UI ever sees.
pub struct Player {
    commands: Sender<Command>,
    events: Receiver<Event>,
    worker: Option<JoinHandle<()>>,
}

impl Player {
    pub fn start() -> Self {
        let (commands_tx, commands_rx) = unbounded();
        let (events_tx, events_rx) = unbounded();

        // The engine is created inside the thread because the audio
        // stream it owns can't be moved between threads.
        let worker = thread::spawn(move || {
            Engine {
                commands: commands_rx,
                events: events_tx,
                session: None,
                last_position_event: Instant::now(),
            }
            .run();
        });

        Self {
            commands: commands_tx,
            events: events_rx,
            worker: Some(worker),
        }
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    pub fn events(&self) -> &Receiver<Event> {
        &self.events
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Quit);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// A track that is opened and ready, waiting for the current one to end.
struct Upcoming {
    path: PathBuf,
    decoder: SymphoniaDecoder,
}

/// The point in the output stream where the next track begins.
struct Boundary {
    at: Duration,
    path: PathBuf,
}

/// What one pump of the session achieved.
enum Step {
    Playing,
    Advanced(PathBuf),
    Finished,
}

/// One continuous audio stream: a decoder feeding an output.
/// Several tracks can pass through the same session when they share a format.
struct Session {
    decoder: Box<dyn Decoder>,
    output: Box<dyn AudioOutput>,
    pending: Option<Vec<f32>>, // decoded but not yet accepted by the output
    decoder_done: bool,
    paused: bool,

    next: Option<Upcoming>,
    boundary: Option<Boundary>, // set once the decoder has moved on to `next`

    fed: u64,              // samples handed to the output since it was opened
    track_start: Duration, // output position where the current track began
    offset: Duration,      // where in the track the output started (after a seek)
}

impl Session {
    fn open(path: &Path) -> Result<Self> {
        let decoder = SymphoniaDecoder::open(path)?;
        let output = CpalOutput::open(decoder.spec())?;
        Ok(Self {
            decoder: Box::new(decoder),
            output: Box::new(output),
            pending: None,
            decoder_done: false,
            paused: false,
            next: None,
            boundary: None,
            fed: 0,
            track_start: Duration::ZERO,
            offset: Duration::ZERO,
        })
    }

    fn spec(&self) -> StreamSpec {
        self.decoder.spec()
    }

    /// Position within the current track.
    fn position(&self) -> Duration {
        self.offset + self.output.position().saturating_sub(self.track_start)
    }

    /// Output position corresponding to everything fed so far.
    /// Uses the same formula as the output, so the two compare exactly.
    fn fed_duration(&self) -> Duration {
        let spec = self.decoder.spec();
        let per_second = u64::from(spec.sample_rate) * u64::from(spec.channels);
        Duration::from_secs_f64(self.fed as f64 / per_second as f64)
    }

    fn pump(&mut self) -> Result<Step> {
        self.fill()?;

        // Has playback actually reached the start of the next track?
        let crossed = self
            .boundary
            .as_ref()
            .is_some_and(|b| self.output.position() >= b.at);
        if crossed {
            if let Some(boundary) = self.boundary.take() {
                self.track_start = boundary.at;
                self.offset = Duration::ZERO;
                return Ok(Step::Advanced(boundary.path));
            }
        }

        if self.decoder_done && self.output.is_finished() {
            Ok(Step::Finished)
        } else {
            Ok(Step::Playing)
        }
    }

    /// Feeds decoded audio to the output until its buffer is full.
    fn fill(&mut self) -> Result<()> {
        loop {
            if self.pending.is_none() && !self.decoder_done {
                match self.decoder.next_chunk()? {
                    Some(chunk) => self.pending = Some(chunk),
                    None => {
                        if self.switch_to_next() {
                            continue; // keep feeding, now from the next track
                        }
                        self.decoder_done = true;
                        self.output.finish();
                    }
                }
            }

            let Some(chunk) = self.pending.take() else {
                return Ok(());
            };
            let len = chunk.len() as u64;

            match self.output.try_write(chunk)? {
                Some(rejected) => {
                    self.pending = Some(rejected); // buffer full, retry next tick
                    return Ok(());
                }
                None => self.fed += len,
            }
        }
    }

    /// Moves on to the preloaded track using the same output stream.
    /// Returns false if that isn't possible.
    fn switch_to_next(&mut self) -> bool {
        let Some(upcoming) = self.next.take() else {
            return false;
        };

        // One stream can't change sample rate or channel count midway,
        // and only one switch can be pending at a time.
        if upcoming.decoder.spec() != self.decoder.spec() || self.boundary.is_some() {
            return false;
        }

        self.boundary = Some(Boundary {
            at: self.fed_duration(),
            path: upcoming.path,
        });
        self.decoder = Box::new(upcoming.decoder);
        true
    }

    fn set_next(&mut self, path: Option<&Path>) {
        if self.boundary.is_some() {
            return; // already past the end of this track; the UI will send a fresh one
        }
        if self.next.as_ref().map(|n| n.path.as_path()) == path {
            return; // already prepared
        }
        self.next = path.and_then(|p| {
            let decoder = SymphoniaDecoder::open(p).ok()?;
            Some(Upcoming {
                path: p.to_path_buf(),
                decoder,
            })
        });
    }

    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        self.output.set_paused(paused);
    }

    fn seek(&mut self, to: Duration) -> Result<()> {
        if self.boundary.is_some() {
            return Ok(()); // already decoding the next track; the UI hasn't caught up yet
        }
        self.decoder.seek(to)?;

        // Reopen the output so audio buffered before the seek is discarded.
        self.output = Box::new(CpalOutput::open(self.decoder.spec())?);
        self.output.set_paused(self.paused);
        self.pending = None;
        self.decoder_done = false;
        self.fed = 0;
        self.track_start = Duration::ZERO;
        self.offset = to;
        Ok(())
    }
}

/// The loop that runs on the playback thread.
struct Engine {
    commands: Receiver<Command>,
    events: Sender<Event>,
    session: Option<Session>,
    last_position_event: Instant,
}

impl Engine {
    fn run(mut self) {
        loop {
            while let Ok(command) = self.commands.try_recv() {
                if matches!(command, Command::Quit) {
                    return;
                }
                self.handle(command);
            }
            self.tick();
            thread::sleep(TICK);
        }
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::Play(path) => match Session::open(&path) {
                Ok(session) => {
                    self.emit(Event::Started(session.spec()));
                    self.session = Some(session);
                }
                Err(err) => self.fail(err),
            },
            Command::SetNext(path) => {
                if let Some(session) = self.session.as_mut() {
                    session.set_next(path.as_deref());
                }
            }
            Command::Pause => {
                if let Some(session) = self.session.as_mut() {
                    session.set_paused(true);
                }
            }
            Command::Resume => {
                if let Some(session) = self.session.as_mut() {
                    session.set_paused(false);
                }
            }
            Command::Seek(to) => {
                let result = self.session.as_mut().map(|session| session.seek(to));
                if let Some(Err(err)) = result {
                    self.fail(err);
                }
            }
            Command::Stop => {
                if self.session.take().is_some() {
                    self.emit(Event::Stopped);
                }
            }
            Command::Quit => {}
        }
    }

    fn tick(&mut self) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let outcome = session.pump();
        let position = session.position();

        match outcome {
            Ok(Step::Playing) => {
                if self.last_position_event.elapsed() >= POSITION_INTERVAL {
                    self.last_position_event = Instant::now();
                    self.emit(Event::Position(position));
                }
            }
            Ok(Step::Advanced(path)) => self.emit(Event::Advanced(path)),
            Ok(Step::Finished) => {
                self.session = None;
                self.emit(Event::Finished);
            }
            Err(err) => self.fail(err),
        }
    }

    fn fail(&mut self, err: Error) {
        self.session = None;
        self.emit(Event::Error(err.to_string()));
    }

    fn emit(&self, event: Event) {
        let _ = self.events.send(event);
    }
}
