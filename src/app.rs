use std::time::{Duration, Instant};

use crate::audio::{Command, Event as PlayerEvent, Player, StreamSpec};
use crate::domain::{Filter, Lyrics, Queue, Track};
use crate::error::Result;
use crate::library::Library;
use crate::lyrics::{LyricsEvent, LyricsService};
use crate::ui;
use crate::ui::mouse::{self, DoubleClick, Regions};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{
    self, Event as TermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton,
    MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use ratatui::widgets::ListState;
use std::path::Path;

const SEEK_STEP: Duration = Duration::from_secs(5);
const INPUT_POLL: Duration = Duration::from_millis(50);
/// Rows the list moves per scroll-wheel notch.
const SCROLL_STEP: u16 = 3;

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
    /// LRCLIB has no lyrics for this track.
    Missing,
    /// No local .lrc file and online lookups are off.
    Offline,
    /// The lookup failed (network or service error), with the reason.
    Failed(String),
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
    filter: Filter,
    /// Library indices currently shown in the list. The list selection
    /// is a position in here, not a library index.
    visible: Vec<usize>,
    searching: bool,
    /// Where each panel was drawn last frame, for mouse clicks.
    regions: Regions,
    double_click: DoubleClick,
    /// While dragging on the progress bar: the bar, and where the drag points.
    /// The seek is sent once, on release.
    seek_drag: Option<(Rect, Duration)>,
}

impl App {
    pub fn new(library: Library, fetch_lyrics: bool) -> Self {
        let queue = Queue::new(library.len());
        let visible = (0..library.len()).collect();
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
            filter: Filter::default(),
            visible,
            searching: false,
            regions: Regions::default(),
            double_click: DoubleClick::default(),
            seek_drag: None,
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

    /// Playback position, or where the user is dragging the progress bar to.
    pub fn position(&self) -> Duration {
        self.seek_drag.map_or(self.position, |(_, target)| target)
    }

    pub fn spec(&self) -> Option<StreamSpec> {
        self.spec
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn regions_mut(&mut self) -> &mut Regions {
        &mut self.regions
    }

    pub fn list_state_mut(&mut self) -> &mut ListState {
        &mut self.list_state
    }

    /// Library indices of the tracks shown in the list, in display order.
    pub fn visible(&self) -> &[usize] {
        &self.visible
    }

    pub fn filter_query(&self) -> &str {
        self.filter.query()
    }

    /// True while the user is typing a search query.
    pub fn is_searching(&self) -> bool {
        self.searching
    }

    pub fn now_playing(&self) -> Option<&Track> {
        self.queue.current().and_then(|i| self.library.get(i))
    }

    // ---- input ----

    /// Waits briefly for input, then handles everything that's queued.
    /// Mouse movement can produce bursts of events; taking them all before
    /// the next redraw keeps the UI responsive.
    fn handle_input(&mut self) -> Result<()> {
        let mut timeout = INPUT_POLL;
        while event::poll(timeout)? {
            match event::read()? {
                TermEvent::Key(key) if key.kind == KeyEventKind::Press => {
                    if self.searching {
                        self.on_search_key(key);
                    } else {
                        self.on_key(key.code);
                    }
                }
                TermEvent::Mouse(mouse) => self.on_mouse(mouse),
                _ => {}
            }
            timeout = Duration::ZERO;
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
            KeyCode::Char('r') => self.cycle_repeat(),
            KeyCode::Char('z') => self.toggle_shuffle(),
            KeyCode::Right => self.seek_forward(),
            KeyCode::Left => self.seek_backward(),
            KeyCode::Char('l') => self.show_lyrics = !self.show_lyrics,
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Esc if !self.filter.is_empty() => self.clear_filter(),
            _ => {}
        }
    }

    /// Keys while typing a search. Letters go into the query, so 'q' or 'n'
    /// type instead of quitting or skipping.
    fn on_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.searching = false;
                self.clear_filter();
            }
            KeyCode::Enter => self.searching = false, // keep the filter, back to normal keys
            KeyCode::Backspace => {
                self.filter.pop();
                self.refresh_filter();
            }
            KeyCode::Down => self.list_state.select_next(),
            KeyCode::Up => self.list_state.select_previous(),
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.filter.push(c);
                self.refresh_filter();
            }
            _ => {}
        }
    }

    // ---- mouse ----

    fn on_mouse(&mut self, mouse: MouseEvent) {
        let (column, row) = (mouse.column, mouse.row);
        let over_library = mouse::hit(self.regions.library, column, row);

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => self.on_click(column, row),
            MouseEventKind::Drag(MouseButton::Left) => self.drag_seek(column),
            MouseEventKind::Up(MouseButton::Left) => self.finish_seek_drag(),
            MouseEventKind::ScrollDown if over_library => {
                self.list_state.scroll_down_by(SCROLL_STEP)
            }
            MouseEventKind::ScrollUp if over_library => self.list_state.scroll_up_by(SCROLL_STEP),
            _ => {}
        }
    }

    fn on_click(&mut self, column: u16, row: u16) {
        let regions = self.regions;

        if mouse::hit(regions.search, column, row) {
            self.searching = true;
            return;
        }
        // Clicking anywhere else finishes typing a search, like Enter does.
        self.searching = false;

        let list_item = regions
            .library
            .and_then(|area| mouse::item_at(area, self.list_state.offset(), column, row));
        let lyric_line = regions
            .lyrics
            .and_then(|(area, top)| mouse::item_at(area, top, column, row));

        if let Some(bar) = regions
            .progress
            .filter(|&bar| mouse::hit(Some(bar), column, row))
        {
            self.start_seek_drag(bar, column);
        } else if let Some(item) = list_item {
            self.click_list_item(item);
        } else if let Some(line) = lyric_line {
            self.seek_to_lyric(line);
        } else if mouse::hit(regions.now_playing, column, row)
            && self.state != PlaybackState::Stopped
        {
            self.toggle_pause();
        }
    }

    /// Click selects; a double click plays.
    fn click_list_item(&mut self, item: usize) {
        if item >= self.visible.len() {
            return; // empty space below the last track
        }
        self.list_state.select(Some(item));
        if self.double_click.click(item, Instant::now()) {
            self.play_selected();
        }
    }

    /// Jumps to a line of synced lyrics.
    fn seek_to_lyric(&mut self, line: usize) {
        let LyricsState::Ready(lyrics) = &self.lyrics else {
            return;
        };
        if let Some(time) = lyrics.lines().get(line).and_then(|line| line.time) {
            self.seek_to(time);
        }
    }

    fn start_seek_drag(&mut self, bar: Rect, column: u16) {
        if self.state == PlaybackState::Stopped {
            return;
        }
        self.seek_drag = Some((bar, Duration::ZERO));
        self.drag_seek(column);
    }

    fn drag_seek(&mut self, column: u16) {
        let Some((bar, _)) = self.seek_drag else {
            return;
        };
        let Some(track) = self.now_playing() else {
            self.seek_drag = None;
            return;
        };
        let limit = track.duration.saturating_sub(Duration::from_secs(1));
        let target = track
            .duration
            .mul_f64(mouse::fraction_at(bar, column))
            .min(limit);
        self.seek_drag = Some((bar, target));
    }

    fn finish_seek_drag(&mut self) {
        if let Some((_, target)) = self.seek_drag.take() {
            self.seek_to(target);
        }
    }

    // ---- search ----

    fn clear_filter(&mut self) {
        self.filter.clear();
        self.refresh_filter();
    }

    /// Recomputes the visible tracks, keeping the selected track selected
    /// if it still matches, otherwise selecting the first match.
    fn refresh_filter(&mut self) {
        let selected = self.selected_track();
        self.visible = self.filter.apply(self.library.tracks());

        let position = selected
            .and_then(|track| self.visible.iter().position(|&i| i == track))
            .or(if self.visible.is_empty() {
                None
            } else {
                Some(0)
            });
        self.list_state.select(position);
    }

    /// Library index of the track under the cursor.
    fn selected_track(&self) -> Option<usize> {
        self.list_state
            .selected()
            .and_then(|position| self.visible.get(position).copied())
    }

    pub fn lyrics(&self) -> &LyricsState {
        &self.lyrics
    }

    pub fn show_lyrics(&self) -> bool {
        self.show_lyrics
    }

    // ---- playback actions ----

    fn play_selected(&mut self) {
        if let Some(index) = self.selected_track() {
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

    /// Starts the queue's current track from scratch.
    fn start_current(&mut self) {
        let Some(track) = self.now_playing() else {
            return;
        };
        self.player.send(Command::Play(track.path.clone()));
        self.on_track_started();
    }

    /// UI bookkeeping for when the queue's current track starts playing,
    /// whether we asked for it (Play) or the player moved on by itself (gapless).
    fn on_track_started(&mut self) {
        if let Some(track) = self.now_playing() {
            self.lyrics_service.request(track.clone());
        }
        self.state = PlaybackState::Playing;
        self.position = Duration::ZERO;
        self.lyrics = LyricsState::Loading;
        self.message = None;
        self.update_next();
    }

    /// Tells the player which track follows the current one, so it can
    /// open it early and switch over without a gap.
    fn update_next(&self) {
        let next = self
            .queue
            .peek_next()
            .and_then(|index| self.library.get(index))
            .map(|track| track.path.clone());
        self.player.send(Command::SetNext(next));
    }

    fn cycle_repeat(&mut self) {
        self.queue.cycle_repeat();
        self.update_next(); // what comes next may have changed
    }

    fn toggle_shuffle(&mut self) {
        self.queue.toggle_shuffle();
        self.update_next(); // the preloaded next track is now different
    }

    /// The player crossed into the preloaded next track on its own.
    fn on_track_advanced(&mut self, path: &Path) {
        let expected = self
            .queue
            .peek_next()
            .and_then(|index| self.library.get(index))
            .map(|track| track.path.as_path());
        if expected != Some(path) {
            return; // stale: the user has already moved on
        }
        self.queue.on_track_end();
        self.on_track_started();
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
                PlayerEvent::Advanced(path) => self.on_track_advanced(&path),
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
                LyricsEvent::Offline { path } if self.is_current(&path) => {
                    self.lyrics = LyricsState::Offline;
                }
                LyricsEvent::Failed { path, reason } if self.is_current(&path) => {
                    self.lyrics = LyricsState::Failed(reason);
                }
                _ => {} // a result for a track we've already moved past
            }
        }
    }

    fn is_current(&self, path: &Path) -> bool {
        self.now_playing().is_some_and(|track| track.path == path)
    }
}
