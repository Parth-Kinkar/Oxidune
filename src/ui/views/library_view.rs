use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, List, ListItem, Paragraph};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let show_search = app.is_searching() || !app.filter_query().is_empty();
    let (list_area, search_area) = if show_search {
        let [list, search] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).areas(area);
        (list, Some(search))
    } else {
        (area, None)
    };

    let current = app.queue().current();
    let tracks = app.library().tracks();

    let items: Vec<ListItem> = app
        .visible()
        .iter()
        .map(|&index| {
            let track = &tracks[index];
            let marker = if Some(index) == current { "▶ " } else { "  " };
            ListItem::new(format!(
                "{marker}{} — {}  [{}]",
                track.artist,
                track.title,
                track.display_duration()
            ))
        })
        .collect();

    let title = if app.filter_query().is_empty() {
        " Library ".to_string()
    } else {
        format!(" Library ({}/{}) ", app.visible().len(), tracks.len())
    };

    let block = Block::bordered().title(title);
    app.regions_mut().library = Some(block.inner(list_area));

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED));

    frame.render_stateful_widget(list, list_area, app.list_state_mut());

    if let Some(search_area) = search_area {
        app.regions_mut().search = Some(search_area);
        render_search(frame, search_area, app);
    }
}

fn render_search(frame: &mut Frame, area: Rect, app: &App) {
    let query = app.filter_query();
    let line = if app.is_searching() {
        Line::from(vec!["/".into(), query.into(), "█".slow_blink()])
    } else {
        Line::from(vec!["/".dark_gray(), query.into()])
    };

    let hint = if app.is_searching() {
        " Enter keep · Esc clear "
    } else {
        " / edit · Esc clear "
    };

    let block = Block::bordered().title(" Search ").title_bottom(hint);
    frame.render_widget(Paragraph::new(line).block(block), area);
}
