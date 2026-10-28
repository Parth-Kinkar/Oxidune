use std::thread;
use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::domain::Track;
use crate::error::{Error, Result};

const API: &str = "https://lrclib.net/api";
const USER_AGENT: &str = concat!("TuneTerm/", env!("CARGO_PKG_VERSION"));

/// How far (in seconds) a result's length may differ from our track's.
const DURATION_TOLERANCE: f64 = 3.0;

/// LRCLIB sometimes answers "busy" (503); one short wait usually fixes it.
const RETRY_DELAY: Duration = Duration::from_millis(1500);

/// Title extras that LRCLIB entries usually leave out, e.g.
/// "Tum Hi Ho (From \"Aashiqui 2\")" or "Closer (feat. Halsey)".
const DROPPED_EXTRAS: [&str; 6] = ["feat.", "feat ", "ft.", "featuring ", "from ", "with "];

/// Separators between artists in one tag, e.g. "Atif Aslam, Tulsi Kumar & Rochak Kohli".
const ARTIST_SEPARATORS: [&str; 6] = [",", "&", ";", " x ", " feat.", " ft."];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    duration: Option<f64>,
    synced_lyrics: Option<String>,
    plain_lyrics: Option<String>,
}

/// One way of asking LRCLIB for a track, from strictest to loosest.
enum Query {
    /// Artist, title, album, and duration must all match.
    Exact,
    Search {
        artist: String,
        title: String,
    },
    Keywords(String),
}

/// Why a request failed.
enum Failure {
    /// Couldn't reach the server at all (offline, DNS, timeout).
    /// Every other query would fail the same way, so stop.
    Network(String),
    /// The server answered with an error. A different query might still work.
    Server(String),
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
    /// Ok(None) means LRCLIB doesn't have it; Err means we couldn't find out.
    pub fn fetch(&self, track: &Track) -> Result<Option<String>> {
        let mut server_error = None;

        for query in queries(track) {
            match self.run(&query, track) {
                Ok(Some(text)) => return Ok(Some(text)),
                Ok(None) => {}
                Err(Failure::Network(message)) => return Err(Error::Lyrics(message)),
                Err(Failure::Server(message)) => server_error = Some(message),
            }
        }

        match server_error {
            Some(message) => Err(Error::Lyrics(message)),
            None => Ok(None),
        }
    }

    fn run(&self, query: &Query, track: &Track) -> std::result::Result<Option<String>, Failure> {
        let wanted = track.duration.as_secs_f64();
        match query {
            Query::Exact => {
                let duration = track.duration.as_secs().to_string();
                let entry: Option<Entry> = self.get(
                    "get",
                    &[
                        ("artist_name", &track.artist),
                        ("track_name", &track.title),
                        ("album_name", &track.album),
                        ("duration", &duration),
                    ],
                )?;
                Ok(entry.and_then(|entry| lyrics_text(entry, wanted).map(|(_, _, text)| text)))
            }
            Query::Search { artist, title } => {
                let entries: Option<Vec<Entry>> =
                    self.get("search", &[("artist_name", artist), ("track_name", title)])?;
                Ok(best_match(entries.unwrap_or_default(), wanted))
            }
            Query::Keywords(keywords) => {
                let entries: Option<Vec<Entry>> = self.get("search", &[("q", keywords)])?;
                Ok(best_match(entries.unwrap_or_default(), wanted))
            }
        }
    }

    /// GETs an endpoint and decodes the JSON. Ok(None) on 404.
    /// Retries once when the server is busy.
    fn get<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        params: &[(&str, &str)],
    ) -> std::result::Result<Option<T>, Failure> {
        let mut last = Failure::Server("no attempt made".into());

        for attempt in 0..2 {
            if attempt > 0 {
                thread::sleep(RETRY_DELAY);
            }

            let mut request = self
                .agent
                .get(format!("{API}/{endpoint}"))
                .header("User-Agent", USER_AGENT);
            for (key, value) in params {
                request = request.query(*key, *value);
            }

            match request.call() {
                Ok(mut response) => {
                    return response
                        .body_mut()
                        .read_json()
                        .map(Some)
                        .map_err(|err| Failure::Server(format!("bad response: {err}")));
                }
                Err(ureq::Error::StatusCode(404)) => return Ok(None),
                Err(ureq::Error::StatusCode(code)) if code == 429 || code >= 500 => {
                    last = Failure::Server(format!("LRCLIB is busy (HTTP {code})"));
                }
                Err(ureq::Error::StatusCode(code)) => {
                    return Err(Failure::Server(format!("LRCLIB returned HTTP {code}")));
                }
                Err(err) => return Err(Failure::Network(err.to_string())),
            }
        }

        Err(last)
    }
}

/// The lookups to try, strictest first, skipping ones that would repeat.
fn queries(track: &Track) -> Vec<Query> {
    let title = clean_title(&track.title);
    let artist = primary_artist(&track.artist).to_string();

    let mut queries = vec![
        Query::Exact,
        Query::Search {
            artist: track.artist.clone(),
            title: track.title.clone(),
        },
    ];
    if title != track.title || artist != track.artist {
        queries.push(Query::Search {
            artist: artist.clone(),
            title: title.clone(),
        });
    }
    queries.push(Query::Keywords(format!("{title} {artist}")));
    queries
}

/// Picks the best candidate: synced beats plain, a closer length beats a
/// farther one, and anything off by more than the tolerance is ignored.
fn best_match(entries: Vec<Entry>, wanted: f64) -> Option<String> {
    entries
        .into_iter()
        .filter_map(|entry| lyrics_text(entry, wanted))
        .min_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)))
        .map(|(_, _, text)| text)
}

/// (is_plain, duration difference, text) for a usable entry.
fn lyrics_text(entry: Entry, wanted: f64) -> Option<(bool, f64, String)> {
    let diff = entry.duration.map_or(0.0, |d| (d - wanted).abs());
    if diff > DURATION_TOLERANCE {
        return None;
    }
    let synced = non_empty(entry.synced_lyrics);
    let is_plain = synced.is_none();
    let text = synced.or(non_empty(entry.plain_lyrics))?;
    Some((is_plain, diff, text))
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.filter(|t| !t.trim().is_empty())
}

/// Drops bracketed extras like "(feat. X)" or "(From \"Movie\")" from a title.
/// Other brackets, like "(Title Track)", are kept.
fn clean_title(title: &str) -> String {
    let mut out = String::new();
    let mut rest = title;

    while let Some(start) = rest.find(['(', '[']) {
        let close = if rest[start..].starts_with('(') {
            ')'
        } else {
            ']'
        };
        let Some(length) = rest[start..].find(close) else {
            break;
        };
        let end = start + length; // index of the closing bracket
        let inside = rest[start + 1..end].trim_start().to_lowercase();

        out.push_str(&rest[..start]);
        if !DROPPED_EXTRAS.iter().any(|extra| inside.starts_with(extra)) {
            out.push_str(&rest[start..=end]);
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);

    let cleaned = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        title.trim().to_string()
    } else {
        cleaned
    }
}

/// The first artist named in a tag that lists several.
fn primary_artist(artist: &str) -> &str {
    let end = ARTIST_SEPARATORS
        .iter()
        .filter_map(|separator| artist.find(separator))
        .min()
        .unwrap_or(artist.len());
    match artist[..end].trim() {
        "" => artist.trim(),
        first => first,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_title_drops_feat_and_from() {
        assert_eq!(clean_title(r#"Tum Hi Ho (From "Aashiqui 2")"#), "Tum Hi Ho");
        assert_eq!(clean_title("Closer (feat. Halsey)"), "Closer");
        assert_eq!(clean_title("rockstar (Feat. 21 Savage)"), "rockstar");
        assert_eq!(clean_title("Song [ft. Someone] (with Another)"), "Song");
    }

    #[test]
    fn clean_title_keeps_other_brackets() {
        assert_eq!(
            clean_title("Ae Dil Hai Mushkil (Title Track)"),
            "Ae Dil Hai Mushkil (Title Track)"
        );
        assert_eq!(
            clean_title("death bed (feat. beabadoobee) [coffee for your head]"),
            "death bed [coffee for your head]"
        );
        assert_eq!(clean_title("Señorita"), "Señorita");
        assert_eq!(clean_title("Unclosed (feat. X"), "Unclosed (feat. X");
        assert_eq!(clean_title("(feat. X)"), "(feat. X)");
    }

    #[test]
    fn primary_artist_takes_the_first_name() {
        assert_eq!(
            primary_artist("Atif Aslam, Tulsi Kumar & Rochak Kohli"),
            "Atif Aslam"
        );
        assert_eq!(primary_artist("Pritam & Arijit Singh"), "Pritam");
        assert_eq!(
            primary_artist("Alan Walker, Au/Ra & Tomine Harket"),
            "Alan Walker"
        );
        assert_eq!(primary_artist("Arijit Singh"), "Arijit Singh");
    }

    fn entry(duration: f64, synced: Option<&str>, plain: Option<&str>) -> Entry {
        Entry {
            duration: Some(duration),
            synced_lyrics: synced.map(String::from),
            plain_lyrics: plain.map(String::from),
        }
    }

    #[test]
    fn best_match_prefers_synced_then_closest_length() {
        let entries = vec![
            entry(200.0, None, Some("plain, exact length")),
            entry(202.0, Some("synced, 2s off"), None),
            entry(201.0, Some("synced, 1s off"), None),
            entry(210.0, Some("synced, too far"), None),
        ];
        assert_eq!(
            best_match(entries, 200.0).as_deref(),
            Some("synced, 1s off")
        );
    }

    #[test]
    fn best_match_ignores_wrong_lengths_and_empty_lyrics() {
        let entries = vec![
            entry(250.0, Some("too long"), None),
            entry(200.0, Some("  "), None),
        ];
        assert_eq!(best_match(entries, 200.0), None);
    }
}
