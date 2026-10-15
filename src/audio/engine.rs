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
    /// The track played to its end (the UI should advance the queue).
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

    /// A cloneable sender, for input threads.
    pub fn commands(&self) -> Sender<Command> {
        self.commands.clone()
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

/// One track being played: its decoder, its output, and the state between them.
struct Session {
    decoder: Box<dyn Decoder>,
    output: Box<dyn AudioOutput>,
    pending: Option<Vec<f32>>, // decoded but not yet accepted by the output
    decoder_done: bool,
    offset: Duration, // where in the track this output started (non-zero after a seek)
    paused: bool,
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
            offset: Duration::ZERO,
            paused: false,
        })
    }

    fn spec(&self) -> StreamSpec {
        self.decoder.spec()
    }

    fn position(&self) -> Duration {
        self.offset + self.output.position()
    }

    /// Feeds decoded audio to the output until its buffer is full.
    /// Returns true once the track has completely finished playing.
    fn pump(&mut self) -> Result<bool> {
        loop {
            if self.pending.is_none() && !self.decoder_done {
                match self.decoder.next_chunk()? {
                    Some(chunk) => self.pending = Some(chunk),
                    None => {
                        self.decoder_done = true;
                        self.output.finish();
                    }
                }
            }

            match self.pending.take() {
                Some(chunk) => {
                    if let Some(rejected) = self.output.try_write(chunk)? {
                        self.pending = Some(rejected); // buffer full, retry next tick
                        return Ok(false);
                    }
                }
                None => return Ok(self.decoder_done && self.output.is_finished()),
            }
        }
    }

    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        self.output.set_paused(paused);
    }

    fn seek(&mut self, to: Duration) -> Result<()> {
        self.decoder.seek(to)?;
        // Reopen the output so audio buffered before the seek is discarded.
        self.output = Box::new(CpalOutput::open(self.decoder.spec())?);
        self.output.set_paused(self.paused);
        self.pending = None;
        self.decoder_done = false;
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
            Ok(true) => {
                self.session = None;
                self.emit(Event::Finished);
            }
            Ok(false) => {
                if self.last_position_event.elapsed() >= POSITION_INTERVAL {
                    self.last_position_event = Instant::now();
                    self.emit(Event::Position(position));
                }
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
