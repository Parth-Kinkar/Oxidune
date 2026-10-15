use std::path::PathBuf;
use std::thread;

use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::domain::{Lyrics, Track};

use super::lrclib::LrclibClient;
use super::store::LyricsStore;

pub enum LyricsEvent {
    Loaded { path: PathBuf, lyrics: Lyrics },
    Missing { path: PathBuf },
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

    if online {
        match client.fetch(track) {
            Ok(Some(text)) => {
                let lyrics = Lyrics::parse(&text);
                if !lyrics.is_empty() {
                    LyricsStore::save(&path, &text);
                    return LyricsEvent::Loaded { path, lyrics };
                }
            }
            Ok(None) => {}
            Err(err) => eprintln!("lyrics lookup failed: {err}"),
        }
    }

    LyricsEvent::Missing { path }
}
