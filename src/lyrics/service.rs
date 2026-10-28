use std::path::PathBuf;
use std::thread;

use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::domain::{Lyrics, Track};
use crate::error::Error;

use super::lrclib::LrclibClient;
use super::store::LyricsStore;

pub enum LyricsEvent {
    Loaded {
        path: PathBuf,
        lyrics: Lyrics,
    },
    /// LRCLIB answered, but has no lyrics for this track.
    Missing {
        path: PathBuf,
    },
    /// No local .lrc file, and online lookups are turned off.
    Offline {
        path: PathBuf,
    },
    /// Couldn't ask LRCLIB (no network, or the service is down).
    Failed {
        path: PathBuf,
        reason: String,
    },
}

/// Finds lyrics for tracks on a background thread.
pub struct LyricsService {
    requests: Sender<Track>,
    events: Receiver<LyricsEvent>,
}

impl LyricsService {
    pub fn start(online: bool) -> Self {
        let (requests_tx, requests_rx) = unbounded::<Track>();
        let (events_tx, events_rx) = unbounded();

        thread::spawn(move || {
            let client = LrclibClient::new();
            // Exits when the service (and its sender) is dropped.
            while let Ok(first) = requests_rx.recv() {
                // If the user skipped ahead, only the newest request matters.
                let track = requests_rx.try_iter().last().unwrap_or(first);
                let _ = events_tx.send(resolve(&track, online, &client));
            }
        });

        Self {
            requests: requests_tx,
            events: events_rx,
        }
    }

    pub fn request(&self, track: Track) {
        let _ = self.requests.send(track);
    }

    pub fn events(&self) -> &Receiver<LyricsEvent> {
        &self.events
    }
}

fn resolve(track: &Track, online: bool, client: &LrclibClient) -> LyricsEvent {
    let path = track.path.clone();

    if let Some(lyrics) = LyricsStore::load(&path) {
        return LyricsEvent::Loaded { path, lyrics };
    }

    if !online {
        return LyricsEvent::Offline { path };
    }

    // Errors go to the lyrics panel, never to stderr: printing would draw
    // over the TUI.
    match client.fetch(track) {
        Ok(Some(text)) => {
            let lyrics = Lyrics::parse(&text);
            if lyrics.is_empty() {
                return LyricsEvent::Missing { path };
            }
            LyricsStore::save(&path, &text);
            LyricsEvent::Loaded { path, lyrics }
        }
        Ok(None) => LyricsEvent::Missing { path },
        Err(err) => LyricsEvent::Failed {
            path,
            reason: match err {
                Error::Lyrics(message) => message,
                other => other.to_string(),
            },
        },
    }
}
