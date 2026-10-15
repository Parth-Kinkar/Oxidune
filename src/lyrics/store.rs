use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::Lyrics;

/// Reads and writes the .lrc file that sits next to an audio file.
pub struct LyricsStore;

impl LyricsStore {
    pub fn path_for(audio: &Path) -> PathBuf {
        audio.with_extension("lrc")
    }

    pub fn load(audio: &Path) -> Option<Lyrics> {
        let text = fs::read_to_string(Self::path_for(audio)).ok()?;
        let lyrics = Lyrics::parse(&text);
        (!lyrics.is_empty()).then_some(lyrics)
    }

    /// Best effort: on a read-only drive we simply don't cache.
    pub fn save(audio: &Path, text: &str) {
        let _ = fs::write(Self::path_for(audio), text);
    }
}
