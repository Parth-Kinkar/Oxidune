mod app;
mod audio;
mod domain;
mod error;
mod library;
mod lyrics;
mod ui;

use std::env;
use std::io::stdout;
use std::panic;
use std::path::PathBuf;

use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;

use app::App;
use library::Library;

fn main() -> error::Result<()> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    let offline = args.iter().any(|arg| arg == "--offline");
    let mouse = !args.iter().any(|arg| arg == "--no-mouse");
    args.retain(|arg| arg != "--offline" && arg != "--no-mouse");

    let dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let library = Library::load(&dir)?;

    let mut terminal = ratatui::init();
    if mouse {
        enable_mouse();
    }
    let result = App::new(library, !offline).run(&mut terminal);
    if mouse {
        disable_mouse();
    }
    ratatui::restore();
    result
}

/// Asks the terminal to report clicks and scrolling. Also makes sure that's
/// switched off if we panic; otherwise the terminal would keep printing
/// escape codes whenever the mouse moves.
fn enable_mouse() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        disable_mouse();
        previous(info); // ratatui's hook, which restores the rest of the terminal
    }));
    let _ = execute!(stdout(), EnableMouseCapture);
}

fn disable_mouse() {
    let _ = execute!(stdout(), DisableMouseCapture);
}
