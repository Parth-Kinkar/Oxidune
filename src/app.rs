use std::time::Duration;

use crate::audio::{Command, Event as PlayerEvent, Player, StreamSpec};
use crate::domain::{Lyrics, Queue, Track};
use crate::error::Result;
use crate::library::Library;
use crate::lyrics::{LyricsEvent, LyricsService};
use crate::ui;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event as TermEvent, KeyCode, KeyEventKind};
use ratatui::widgets::ListState;
use std::path::Path;

const SEEK_STEP: Duration = Duration::from_secs(5);
const INPUT_POLL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

/// What the lyrics panel should show for the current track.
pub enum LyricsState {
    None,
    Loading,
    Ready(Lyrics),
    Missing,
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
    lyrics_service: LyricsService,
    lyrics: LyricsState,
    show_lyrics: bool,
}

impl App {
    pub fn new(library: Library, fetch_lyrics: bool) -> Self {
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
            lyrics_service: LyricsService::start(fetch_lyrics),
            lyrics: LyricsState::None,
            show_lyrics: true,
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| ui::render(frame, self))?;
            self.handle_input()?;
            self.handle_player_events();
            self.handle_lyrics_events();
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
            KeyCode::Char('l') => self.show_lyrics = !self.show_lyrics,
            _ => {}
        }
    }

    pub fn lyrics(&self) -> &LyricsState {
        &self.lyrics
    }

    pub fn show_lyrics(&self) -> bool {
        self.show_lyrics
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
        self.lyrics_service.request(track.clone());
        self.state = PlaybackState::Playing;
        self.position = Duration::ZERO;
        self.lyrics = LyricsState::Loading;
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
        self.lyrics = LyricsState::None;
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

    fn handle_lyrics_events(&mut self) {
        while let Ok(event) = self.lyrics_service.events().try_recv() {
            match event {
                LyricsEvent::Loaded { path, lyrics } if self.is_current(&path) => {
                    self.lyrics = LyricsState::Ready(lyrics);
                }
                LyricsEvent::Missing { path } if self.is_current(&path) => {
                    self.lyrics = LyricsState::Missing;
                }
                _ => {} // a result for a track we've already moved past
            }
        }
    }

    fn is_current(&self, path: &Path) -> bool {
        self.now_playing().is_some_and(|track| track.path == path)
    }
}
