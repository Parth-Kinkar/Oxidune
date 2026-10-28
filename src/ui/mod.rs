pub mod mouse;
mod views;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::{App, Focus, SideTab};

const HELP: &str = " ↑↓ select · / search · Enter play · a play next · Space pause · n/p next/prev · ←→ seek · r repeat · z shuffle · l lyrics · u up next · s stop · q quit";

const QUEUE_HELP: &str = " ↑↓ select · Enter play now · J/K move down/up · d remove · Tab back to library · u hide · Space pause · q quit";

const SEARCH_HELP: &str =
    " type to filter · ↑↓ select · Backspace delete · Enter keep filter · Esc clear";

const LYRICS_TAB: &str = " Lyrics ";
const QUEUE_TAB: &str = " Up next ";

pub fn render(frame: &mut Frame, app: &mut App) {
    // Each view records where it draws; panels not drawn this frame stay empty.
    *app.regions_mut() = mouse::Regions::default();

    let [main, now_playing, help] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(4),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    if let Some(tab) = app.side_tab() {
        let [library, side] =
            Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
                .areas(main);
        views::library_view::render(frame, library, app);
        render_side_panel(frame, side, app, tab);
    } else {
        views::library_view::render(frame, main, app);
    }

    views::now_playing::render(frame, now_playing, app);
    render_help(frame, help, app);
}

/// The right-hand panel: a border with "Lyrics | Up next" tabs, and the
/// active tab's content inside.
fn render_side_panel(frame: &mut Frame, area: Rect, app: &mut App, active: SideTab) {
    let tab_style = |tab: SideTab| {
        if tab == active {
            Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        } else {
            Style::new().fg(Color::DarkGray)
        }
    };
    let title = Line::from(vec![
        Span::styled(LYRICS_TAB, tab_style(SideTab::Lyrics)),
        Span::raw("│"),
        Span::styled(QUEUE_TAB, tab_style(SideTab::Queue)),
    ]);

    let focused = active == SideTab::Queue && app.focus() == Focus::Queue;
    let block = Block::bordered()
        .title(title)
        .border_style(focus_style(focused));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // The title starts one cell in, after the corner.
    let lyrics_width = LYRICS_TAB.chars().count() as u16;
    let queue_width = QUEUE_TAB.chars().count() as u16;
    let regions = app.regions_mut();
    regions.lyrics_tab = Some(Rect::new(area.x + 1, area.y, lyrics_width, 1));
    regions.queue_tab = Some(Rect::new(area.x + 2 + lyrics_width, area.y, queue_width, 1));

    match active {
        SideTab::Lyrics => views::lyrics_view::render(frame, inner, app),
        SideTab::Queue => views::queue_view::render(frame, inner, app),
    }
}

/// Border style for a panel: highlighted when it has keyboard focus.
pub fn focus_style(focused: bool) -> Style {
    if focused {
        Style::new().fg(Color::Cyan)
    } else {
        Style::new()
    }
}

fn render_help(frame: &mut Frame, area: Rect, app: &App) {
    if let Some(notice) = app.notice() {
        frame.render_widget(Paragraph::new(format!(" {notice}")).cyan(), area);
        return;
    }
    let text = if app.is_searching() {
        SEARCH_HELP
    } else if app.focus() == Focus::Queue {
        QUEUE_HELP
    } else {
        HELP
    };
    frame.render_widget(Paragraph::new(text).dark_gray(), area);
}
