use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};

use crate::app::{App, Focus};
use crate::domain::queue::RepeatMode;
use crate::ui::mouse;

/// Draws the Up next list inside `inner` (the side panel draws the border).
pub fn render(frame: &mut Frame, inner: Rect, app: &mut App) {
    if app.queue().current().is_none() {
        render_message(
            frame,
            inner,
            "Nothing playing.\n\nStart a song, then press a on any track to play it next.",
        );
        return;
    }

    let note = match app.queue().repeat() {
        RepeatMode::All => Some("↻ then the queue repeats"),
        RepeatMode::One => Some("↻ repeating the current song"),
        RepeatMode::Off => None,
    };
    let [list_area, note_area] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(u16::from(note.is_some())),
    ])
    .areas(inner);

    let tracks = app.library().tracks();
    let upcoming = app.queue().upcoming();
    let mut items: Vec<ListItem> = app
        .queue()
        .upcoming()
        .iter()
        .enumerate()
        .map(|(n, &index)| {
            let track = &tracks[index];
            // "+" marks songs added with "play next"
            let mark = if app.queue().is_pinned(index) {
                "+"
            } else {
                " "
            };
            ListItem::new(format!(
                "{:>3} {mark} {} — {}  [{}]",
                n + 1,
                track.artist,
                track.title,
                track.display_duration()
            ))
        })
        .collect();

    // With shuffle on, a dim line shows where the shuffled part starts:
    // right after the songs picked with "play next", which keep their order.
    let separator = app.queue().shuffle().then(|| {
        upcoming
            .iter()
            .take_while(|&&track| app.queue().is_pinned(track))
            .count()
    });
    let separator = separator.filter(|&row| row < upcoming.len());
    if let Some(row) = separator {
        let line = Line::from("─── shuffled ↓ ───").centered().dark_gray();
        items.insert(row, ListItem::new(line));
    }

    if items.is_empty() {
        render_message(frame, list_area, "Nothing after this song.");
    } else {
        let highlight = if app.focus() == Focus::Queue {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new().fg(Color::Cyan)
        };
        let list = List::new(items).highlight_style(highlight);

        // The app's selection counts songs only; the list on screen also has
        // the separator line, so shift the selection past it while drawing.
        let state = app.queue_state_mut();
        let selected = state.selected().map(|song| match separator {
            Some(row) if song >= row => song + 1,
            _ => song,
        });
        let mut screen_state = ListState::default()
            .with_offset(state.offset())
            .with_selected(selected);
        frame.render_stateful_widget(list, list_area, &mut screen_state);

        // Keep the scroll position, and the (possibly clamped) selection.
        *state.offset_mut() = screen_state.offset();
        state.select(
            screen_state
                .selected()
                .and_then(|row| mouse::skip_separator(row, separator)),
        );

        let regions = app.regions_mut();
        regions.queue = Some(list_area);
        regions.queue_separator = separator;
    }

    if let Some(note) = note {
        frame.render_widget(
            Paragraph::new(note)
                .dark_gray()
                .alignment(Alignment::Center),
            note_area,
        );
    }
}

fn render_message(frame: &mut Frame, area: Rect, text: &str) {
    let paragraph = Paragraph::new(text)
        .dark_gray()
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}
