mod views;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::widgets::Paragraph;

use crate::app::App;

const HELP: &str =
    " ↑↓ select · Enter play · Space pause · n/p next/prev · ←→ seek · r repeat · s stop · q quit";

pub fn render(frame: &mut Frame, app: &mut App) {
    let [library, now_playing, help] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(4),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    views::library_view::render(frame, library, app);
    views::now_playing::render(frame, now_playing, app);
    frame.render_widget(Paragraph::new(HELP).dark_gray(), help);
}
