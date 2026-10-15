use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, List, ListItem};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let current = app.queue().current();

    let items: Vec<ListItem> = app
        .library()
        .tracks()
        .iter()
        .enumerate()
        .map(|(index, track)| {
            let marker = if Some(index) == current { "▶ " } else { "  " };
            ListItem::new(format!(
                "{marker}{} — {}  [{}]",
                track.artist,
                track.title,
                track.display_duration()
            ))
        })
        .collect();

    let list = List::new(items)
        .block(Block::bordered().title(" Library "))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED));

    frame.render_stateful_widget(list, area, app.list_state_mut());
}
