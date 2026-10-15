use std::path::PathBuf;
use std::time::Duration;

/// A single audio file and the metadata we show for it.
/// Plain data, so the fields are public; behavior lives elsewhere.
#[derive(Debug, Clone)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Duration,
}

pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}

impl Track {
    pub fn display_duration(&self) -> String {
        format_duration(self.duration)
    }
}
