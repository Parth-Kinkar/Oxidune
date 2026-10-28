use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{Block, Gauge, Paragraph};

use crate::app::{App, PlaybackState};
use crate::domain::format_duration;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::bordered().title(" Now Playing ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [info, bar] = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(inner);

    if app.now_playing().is_some() {
        let regions = app.regions_mut();
        regions.now_playing = Some(info);
        regions.progress = Some(bar);
    }
    if app.message().is_some() {
        app.regions_mut().progress = None; // the bar shows the error instead
    }

    let modes = format!(
        "repeat: {}  ·  shuffle: {}",
        app.queue().repeat().label(),
        if app.queue().shuffle() { "on" } else { "off" }
    );

    let Some(track) = app.now_playing() else {
        let text = app.message().unwrap_or("Nothing playing");
        frame.render_widget(Paragraph::new(format!("{text}  ·  {modes}")), info);
        return;
    };

    let icon = match app.state() {
        PlaybackState::Playing => "▶",
        PlaybackState::Paused => "⏸",
        PlaybackState::Stopped => "■",
    };
    let spec = app.spec().map(|s| format!("  ·  {s}")).unwrap_or_default();
    let text = format!(
        "{icon} {} — {}{spec}  ·  {modes}",
        track.artist, track.title
    );
    frame.render_widget(Paragraph::new(text), info);

    if let Some(message) = app.message() {
        frame.render_widget(Paragraph::new(message).red(), bar);
        return;
    }

    let ratio = if track.duration.is_zero() {
        0.0
    } else {
        (app.position().as_secs_f64() / track.duration.as_secs_f64()).clamp(0.0, 1.0)
    };
    let label = format!(
        "{} / {}",
        format_duration(app.position()),
        track.display_duration()
    );
    let gauge = Gauge::default()
        .ratio(ratio)
        .label(label)
        .gauge_style(Style::new().fg(Color::Cyan));
    frame.render_widget(gauge, bar);
}
