mod app;
mod audio;
mod domain;
mod error;
mod library;
mod ui;

use std::env;
use std::path::PathBuf;

use app::App;
use library::Library;

fn main() -> error::Result<()> {
    let dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let library = Library::load(&dir)?;

    let mut terminal = ratatui::init();
    let result = App::new(library).run(&mut terminal);
    ratatui::restore();
    result
}
