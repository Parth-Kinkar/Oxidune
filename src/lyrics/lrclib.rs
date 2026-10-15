use std::time::Duration;

use serde::Deserialize;

use crate::domain::Track;
use crate::error::{Error, Result};

const API: &str = "https://lrclib.net/api";
const USER_AGENT: &str = concat!("TuneTerm/", env!("CARGO_PKG_VERSION"));

/// How far (in seconds) a result's length may differ from our track's.
const DURATION_TOLERANCE: f64 = 3.0;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    duration: Option<f64>,
    synced_lyrics: Option<String>,
    plain_lyrics: Option<String>,
}

/// Client for the LRCLIB lyrics API.
pub struct LrclibClient {
    agent: ureq::Agent,
}

impl LrclibClient {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(8)))
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
        }
    }

    /// LRC text for the track: synced if available, otherwise plain.
    /// Ok(None) means LRCLIB doesn't have it.
    pub fn fetch(&self, track: &Track) -> Result<Option<String>> {
        if let Some(text) = self.get_exact(track)? {
            return Ok(Some(text));
        }
        self.search(track)
    }

    /// Exact match on artist, title, album, and duration.
    fn get_exact(&self, track: &Track) -> Result<Option<String>> {
        let result = self
            .agent
            .get(format!("{API}/get"))
            .header("User-Agent", USER_AGENT)
            .query("artist_name", &track.artist)
            .query("track_name", &track.title)
            .query("album_name", &track.album)
            .query("duration", track.duration.as_secs().to_string())
            .call();

        let mut response = match result {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(404)) => return Ok(None),
            Err(err) => return Err(Error::Lyrics(err.to_string())),
        };

        let entry: Entry = response
            .body_mut()
            .read_json()
            .map_err(|err| Error::Lyrics(err.to_string()))?;

        Ok(non_empty(entry.synced_lyrics).or(non_empty(entry.plain_lyrics)))
    }

    /// Looser search by artist and title, then pick the best candidate:
    /// synced beats plain, and a closer duration beats a farther one.
    fn search(&self, track: &Track) -> Result<Option<String>> {
        let mut response = self
            .agent
            .get(format!("{API}/search"))
            .header("User-Agent", USER_AGENT)
            .query("artist_name", &track.artist)
            .query("track_name", &track.title)
            .call()
            .map_err(|err| Error::Lyrics(err.to_string()))?;

        let entries: Vec<Entry> = response
            .body_mut()
            .read_json()
            .map_err(|err| Error::Lyrics(err.to_string()))?;

        let wanted = track.duration.as_secs_f64();

        Ok(entries
            .into_iter()
            .filter_map(|entry| {
                let diff = entry.duration.map_or(0.0, |d| (d - wanted).abs());
                if diff > DURATION_TOLERANCE {
                    return None;
                }
                let synced = non_empty(entry.synced_lyrics);
                let is_plain = synced.is_none();
                let text = synced.or(non_empty(entry.plain_lyrics))?;
                Some((is_plain, diff, text))
            })
            .min_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)))
            .map(|(_, _, text)| text))
    }
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.filter(|t| !t.trim().is_empty())
}
