use std::time::Duration;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event as TermEvent, KeyCode, KeyEventKind};
use ratatui::widgets::ListState;

use crate::audio::{Command, Event as PlayerEvent, Player, StreamSpec};
use crate::domain::{Queue, Track};
use crate::error::Result;
use crate::library::Library;
use crate::ui;

const SEEK_STEP: Duration = Duration::from_secs(5);
const INPUT_POLL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

/// Application state and input handling. Draws nothing, decodes nothing.
pub struct App {
    library: Library,
    queue: Queue,
    player: Player,
    state: PlaybackState,
    position: Duration,
    spec: Option<StreamSpec>,
    list_state: ListState,
    message: Option<String>,
    should_quit: bool,
}

impl App {
    pub fn new(library: Library) -> Self {
        let queue = Queue::new(library.len());
        let mut list_state = ListState::default();
        if library.len() > 0 {
            list_state.select_first();
        }

        Self {
            library,
            queue,
            player: Player::start(),
            state: PlaybackState::Stopped,
            position: Duration::ZERO,
            spec: None,
            list_state,
            message: None,
            should_quit: false,
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| ui::render(frame, self))?;
            self.handle_input()?;
            self.handle_player_events();
        }
        Ok(())
    }

    // ---- read access for the UI ----

    pub fn library(&self) -> &Library {
        &self.library
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    pub fn state(&self) -> PlaybackState {
        self.state
    }

    pub fn position(&self) -> Duration {
        self.position
    }

    pub fn spec(&self) -> Option<StreamSpec> {
        self.spec
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn list_state_mut(&mut self) -> &mut ListState {
        &mut self.list_state
    }

    pub fn now_playing(&self) -> Option<&Track> {
        self.queue.current().and_then(|i| self.library.get(i))
    }

    // ---- input ----

    fn handle_input(&mut self) -> Result<()> {
        if event::poll(INPUT_POLL)? {
            if let TermEvent::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    self.on_key(key.code);
                }
            }
        }
        Ok(())
    }

    fn on_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => self.list_state.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.list_state.select_previous(),
            KeyCode::Enter => self.play_selected(),
            KeyCode::Char(' ') => self.toggle_pause(),
            KeyCode::Char('n') => self.play_next(),
            KeyCode::Char('p') => self.play_previous(),
            KeyCode::Char('s') => self.player.send(Command::Stop),
            KeyCode::Char('r') => self.queue.cycle_repeat(),
            KeyCode::Right => self.seek_forward(),
            KeyCode::Left => self.seek_backward(),
            _ => {}
        }
    }

    // ---- playback actions ----

    fn play_selected(&mut self) {
        if let Some(index) = self.list_state.selected() {
            self.queue.jump_to(index);
            self.start_current();
        }
    }

    fn play_next(&mut self) {
        if self.queue.next().is_some() {
            self.start_current();
        }
    }

    fn play_previous(&mut self) {
        if self.queue.previous().is_some() {
            self.start_current();
        }
    }

    fn start_current(&mut self) {
        let Some(track) = self.now_playing() else {
            return;
        };
        self.player.send(Command::Play(track.path.clone()));
        self.state = PlaybackState::Playing;
        self.position = Duration::ZERO;
        self.message = None;
    }

    fn toggle_pause(&mut self) {
        match self.state {
            PlaybackState::Playing => {
                self.player.send(Command::Pause);
                self.state = PlaybackState::Paused;
            }
            PlaybackState::Paused => {
                self.player.send(Command::Resume);
                self.state = PlaybackState::Playing;
            }
            PlaybackState::Stopped => self.play_selected(),
        }
    }

    fn seek_forward(&mut self) {
        let Some(track) = self.now_playing() else {
            return;
        };
        let limit = track.duration.saturating_sub(Duration::from_secs(1));
        let target = (self.position + SEEK_STEP).min(limit);
        self.seek_to(target);
    }

    fn seek_backward(&mut self) {
        let target = self.position.saturating_sub(SEEK_STEP);
        self.seek_to(target);
    }

    fn seek_to(&mut self, target: Duration) {
        if self.state == PlaybackState::Stopped {
            return;
        }
        self.player.send(Command::Seek(target));
        self.position = target;
    }

    fn stop(&mut self) {
        self.state = PlaybackState::Stopped;
        self.position = Duration::ZERO;
        self.spec = None;
    }

    // ---- events from the player thread ----

    fn handle_player_events(&mut self) {
        while let Ok(event) = self.player.events().try_recv() {
            match event {
                PlayerEvent::Started(spec) => self.spec = Some(spec),
                PlayerEvent::Position(position) => self.position = position,
                PlayerEvent::Finished => self.on_track_finished(),
                PlayerEvent::Stopped => self.stop(),
                PlayerEvent::Error(message) => {
                    self.stop();
                    self.message = Some(message);
                }
            }
        }
    }

    fn on_track_finished(&mut self) {
        if self.queue.on_track_end().is_some() {
            self.start_current();
        } else {
            self.stop();
        }
    }
}
