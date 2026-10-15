use std::path::Path;

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;

use crate::domain::Track;
use crate::error::{Error, Result};

/// Reads tags and audio properties from a file and builds a Track.
pub struct TagReader;

impl TagReader {
    pub fn read(&self, path: &Path) -> Result<Track> {
        let tagged = lofty::read_from_path(path).map_err(|err| Error::Tags {
            path: path.to_path_buf(),
            message: err.to_string(),
        })?;

        let duration = tagged.properties().duration();
        let tag = tagged.primary_tag().or_else(|| tagged.first_tag());

        let title = tag
            .and_then(|t| t.title())
            .map(|s| s.into_owned())
            .unwrap_or_else(|| Self::file_stem(path));
        let artist = tag
            .and_then(|t| t.artist())
            .map(|s| s.into_owned())
            .unwrap_or_else(|| "Unknown Artist".to_string());
        let album = tag
            .and_then(|t| t.album())
            .map(|s| s.into_owned())
            .unwrap_or_else(|| "Unknown Album".to_string());

        Ok(Track {
            path: path.to_path_buf(),
            title,
            artist,
            album,
            duration,
        })
    }

    fn file_stem(path: &Path) -> String {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Unknown Title".to_string())
    }
}
