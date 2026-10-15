use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::error::{Error, Result};

/// Walks a directory and returns the audio files inside it.
pub struct Scanner {
    extensions: Vec<&'static str>,
}

impl Default for Scanner {
    fn default() -> Self {
        Self {
            extensions: vec!["flac", "wav", "m4a", "mp3", "ogg"],
        }
    }
}

impl Scanner {
    pub fn scan(&self, root: &Path) -> Result<Vec<PathBuf>> {
        if !root.is_dir() {
            return Err(Error::NotADirectory(root.to_path_buf()));
        }

        Ok(WalkDir::new(root)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.into_path())
            .filter(|path| self.is_supported(path))
            .collect())
    }

    fn is_supported(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| self.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
            .unwrap_or(false)
    }
}
