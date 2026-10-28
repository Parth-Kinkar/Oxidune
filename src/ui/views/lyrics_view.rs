use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::{App, LyricsState};
use crate::domain::Lyrics;

/// Draws the lyrics inside `inner` (the side panel draws the border).
pub fn render(frame: &mut Frame, inner: Rect, app: &mut App) {
    let top = match app.lyrics() {
        LyricsState::Ready(lyrics) => Some(render_lines(frame, inner, lyrics, app.position())),
        other => {
            render_status(frame, inner, other);
            None
        }
    };
    app.regions_mut().lyrics = top.map(|top| (inner, top));
}

/// Everything except actual lyrics: loading, missing, offline, failed.
fn render_status(frame: &mut Frame, inner: Rect, state: &LyricsState) {
    match state {
        LyricsState::None | LyricsState::Ready(_) => {}
        LyricsState::Loading => render_message(frame, inner, "Loading lyrics…"),
        LyricsState::Missing => render_message(frame, inner, "No lyrics found"),
        LyricsState::Offline => render_message(frame, inner, "No local lyrics (offline mode)"),
        LyricsState::Failed(reason) => {
            let text = Text::from(vec![
                Line::from("Couldn't reach LRCLIB, try again later"),
                Line::from(reason.as_str()).italic(),
            ]);
            render_message(frame, inner, text);
        }
    }
}

fn render_message<'a>(frame: &mut Frame, area: Rect, text: impl Into<Text<'a>>) {
    let paragraph = Paragraph::new(text)
        .dark_gray()
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

/// Draws the lyrics, one line per row. Returns the index of the line on the
/// first row, so a click on a row can be mapped back to its line.
fn render_lines(frame: &mut Frame, area: Rect, lyrics: &Lyrics, position: Duration) -> usize {
    let current = lyrics.current_index(position);
    let height = area.height as usize;

    // Keep the current line in the middle of the panel.
    let top = if lyrics.is_synced() {
        current.unwrap_or(0).saturating_sub(height / 2)
    } else {
        0
    };

    let lines: Vec<Line> = lyrics
        .lines()
        .iter()
        .enumerate()
        .skip(top)
        .take(height)
        .map(|(index, line)| {
            let style = match current {
                Some(c) if index == c => Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                Some(c) if index < c => Style::new().fg(Color::DarkGray),
                _ => Style::new(),
            };
            Line::styled(line.text.clone(), style)
        })
        .collect();

    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
    top
}
