use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Paragraph, Wrap};

use crate::app::{App, LyricsState};
use crate::domain::Lyrics;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::bordered().title(" Lyrics ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    match app.lyrics() {
        LyricsState::None => {}
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
        LyricsState::Ready(lyrics) => render_lines(frame, inner, lyrics, app.position()),
    }
}

fn render_message<'a>(frame: &mut Frame, area: Rect, text: impl Into<Text<'a>>) {
    let paragraph = Paragraph::new(text)
        .dark_gray()
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_lines(frame: &mut Frame, area: Rect, lyrics: &Lyrics, position: Duration) {
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
}
