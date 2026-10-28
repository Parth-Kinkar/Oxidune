use super::Track;

/// A search query typed by the user. Plain logic, no UI.
///
/// A track matches when every word of the query appears somewhere in its
/// artist, title, or album, ignoring case. So "pink moon" finds
/// "Nick Drake — Pink Moon", and "drake moon" finds it too.
#[derive(Debug, Default, Clone)]
pub struct Filter {
    query: String,
}

impl Filter {
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn is_empty(&self) -> bool {
        self.query.trim().is_empty()
    }

    pub fn push(&mut self, c: char) {
        self.query.push(c);
    }

    pub fn pop(&mut self) {
        self.query.pop();
    }

    pub fn clear(&mut self) {
        self.query.clear();
    }

    pub fn matches(&self, track: &Track) -> bool {
        let haystack = format!("{} {} {}", track.artist, track.title, track.album).to_lowercase();
        self.query
            .to_lowercase()
            .split_whitespace()
            .all(|word| haystack.contains(word))
    }

    /// Indices of the tracks that match, in their original order.
    pub fn apply(&self, tracks: &[Track]) -> Vec<usize> {
        tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| self.matches(track))
            .map(|(index, _)| index)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn track(artist: &str, title: &str, album: &str) -> Track {
        Track {
            path: format!("{title}.flac").into(),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            duration: Duration::from_secs(60),
        }
    }

    fn filter(query: &str) -> Filter {
        let mut filter = Filter::default();
        query.chars().for_each(|c| filter.push(c));
        filter
    }

    #[test]
    fn empty_query_matches_everything() {
        let tracks = [track("A", "One", "X"), track("B", "Two", "Y")];
        assert_eq!(filter("").apply(&tracks), vec![0, 1]);
        assert_eq!(filter("   ").apply(&tracks), vec![0, 1]);
    }

    #[test]
    fn matches_artist_title_or_album_ignoring_case() {
        let tracks = [
            track("Nick Drake", "Pink Moon", "Pink Moon"),
            track("Radiohead", "Airbag", "OK Computer"),
        ];
        assert_eq!(filter("drake").apply(&tracks), vec![0]);
        assert_eq!(filter("AIRBAG").apply(&tracks), vec![1]);
        assert_eq!(filter("computer").apply(&tracks), vec![1]);
    }

    #[test]
    fn every_word_must_match_in_any_order() {
        let tracks = [
            track("Nick Drake", "Pink Moon", "Pink Moon"),
            track("Nick Drake", "River Man", "Five Leaves Left"),
        ];
        assert_eq!(filter("moon drake").apply(&tracks), vec![0]);
        assert_eq!(filter("drake jazz").apply(&tracks), Vec::<usize>::new());
    }

    #[test]
    fn pop_and_clear_edit_the_query() {
        let mut f = filter("abc");
        f.pop();
        assert_eq!(f.query(), "ab");
        f.clear();
        assert!(f.is_empty());
    }
}
