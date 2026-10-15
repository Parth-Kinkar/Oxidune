mod views;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::widgets::Paragraph;

use crate::app::App;

const HELP: &str = " ↑↓ select · Enter play · Space pause · n/p next/prev · ←→ seek · r repeat · l lyrics · s stop · q quit";

pub fn render(frame: &mut Frame, app: &mut App) {
    let [main, now_playing, help] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(4),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    if app.show_lyrics() {
        let [library, lyrics] =
            Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
                .areas(main);
        views::library_view::render(frame, library, app);
        views::lyrics_view::render(frame, lyrics, app);
    } else {
        views::library_view::render(frame, main, app);
    }

    views::now_playing::render(frame, now_playing, app);
    frame.render_widget(Paragraph::new(HELP).dark_gray(), help);
}
