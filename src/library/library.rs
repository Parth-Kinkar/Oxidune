use std::path::Path;

use crate::domain::Track;
use crate::error::Result;

use super::scanner::Scanner;
use super::tags::TagReader;

/// The collection of all tracks found in the music directory.
pub struct Library {
    tracks: Vec<Track>,
}

impl Library {
    pub fn load(root: &Path) -> Result<Self> {
        let paths = Scanner::default().scan(root)?;
        let reader = TagReader;

        let mut tracks: Vec<Track> = paths
            .iter()
            .filter_map(|path| match reader.read(path) {
                Ok(track) => Some(track),
                Err(err) => {
                    eprintln!("skipping: {err}");
                    None
                }
            })
            .collect();

        tracks
            .sort_by(|a, b| (&a.artist, &a.album, &a.title).cmp(&(&b.artist, &b.album, &b.title)));

        Ok(Self { tracks })
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }
    pub fn get(&self, index: usize) -> Option<&Track> {
        self.tracks.get(index)
    }
}
