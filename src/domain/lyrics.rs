use std::time::Duration;

#[derive(Debug, Clone)]
pub struct LyricLine {
    /// None for plain (unsynced) lyrics.
    pub time: Option<Duration>,
    pub text: String,
}

/// Lyrics for one track, parsed from LRC text (or plain text).
#[derive(Debug, Clone)]
pub struct Lyrics {
    lines: Vec<LyricLine>,
    synced: bool,
}

impl Lyrics {
    pub fn parse(source: &str) -> Self {
        let mut lines = Vec::new();
        let mut offset_ms: i64 = 0;

        for raw in source.trim_start_matches('\u{feff}').lines() {
            let mut rest = raw.trim();
            let mut times = Vec::new();

            // Consume leading [..] tags: timestamps, offset, or metadata like [ar:Name].
            while let Some(inner) = rest.strip_prefix('[') {
                let Some(end) = inner.find(']') else { break };
                let tag = &inner[..end];

                if let Some(time) = parse_timestamp(tag) {
                    times.push(time);
                } else if let Some(value) = tag.strip_prefix("offset:") {
                    offset_ms = value.trim().parse().unwrap_or(0);
                } else if !tag.contains(':') {
                    break; // text in brackets, like "[Chorus]": keep it as text
                }
                rest = inner[end + 1..].trim_start();
            }

            let text = strip_word_timings(rest);
            if times.is_empty() {
                if !text.is_empty() {
                    lines.push(LyricLine { time: None, text });
                }
            } else {
                for time in times {
                    lines.push(LyricLine {
                        time: Some(time),
                        text: text.clone(),
                    });
                }
            }
        }

        if offset_ms != 0 {
            for line in &mut lines {
                line.time = line.time.map(|t| shift(t, offset_ms));
            }
        }

        let synced = !lines.is_empty() && lines.iter().all(|l| l.time.is_some());
        if synced {
            lines.sort_by_key(|l| l.time);
        }

        Self { lines, synced }
    }

    pub fn lines(&self) -> &[LyricLine] {
        &self.lines
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn is_synced(&self) -> bool {
        self.synced
    }

    /// Index of the line being sung at `position`, if any has started yet.
    pub fn current_index(&self, position: Duration) -> Option<usize> {
        if !self.synced {
            return None;
        }
        let started = self
            .lines
            .partition_point(|line| line.time.is_some_and(|t| t <= position));
        started.checked_sub(1)
    }
}

/// "mm:ss.xx" or "mm:ss" -> Duration. Anything else (like "ar:Name") -> None.
fn parse_timestamp(tag: &str) -> Option<Duration> {
    let (minutes, seconds) = tag.split_once(':')?;
    let minutes: u64 = minutes.trim().parse().ok()?;
    let seconds: f64 = seconds.trim().parse().ok()?;
    Some(Duration::from_secs_f64(minutes as f64 * 60.0 + seconds))
}

/// LRC offsets are in ms; positive means lyrics should appear earlier.
fn shift(time: Duration, offset_ms: i64) -> Duration {
    let ms = (time.as_millis() as i64 - offset_ms).max(0);
    Duration::from_millis(ms as u64)
}

/// Removes word-level timestamps like <00:12.50> but leaves things like "<3" alone.
fn strip_word_timings(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        match rest[start..].find('>') {
            Some(len) if parse_timestamp(&rest[start + 1..start + len]).is_some() => {
                rest = &rest[start + len + 1..];
            }
            _ => {
                out.push('<');
                rest = &rest[start + 1..];
            }
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_timestamps_and_skips_metadata() {
        let lyrics = Lyrics::parse("[ar:Someone]\n[00:12.50]Hello\n[00:15.00]World");
        assert!(lyrics.is_synced());
        assert_eq!(lyrics.lines().len(), 2);
        assert_eq!(lyrics.lines()[0].time, Some(Duration::from_millis(12_500)));
    }

    #[test]
    fn repeated_timestamps_expand_and_sort() {
        let lyrics = Lyrics::parse("[00:30.00][00:10.00]La");
        let times: Vec<_> = lyrics.lines().iter().map(|l| l.time.unwrap()).collect();
        assert_eq!(times, [Duration::from_secs(10), Duration::from_secs(30)]);
    }

    #[test]
    fn current_line_follows_position() {
        let lyrics = Lyrics::parse("[00:10.00]a\n[00:20.00]b");
        assert_eq!(lyrics.current_index(Duration::from_secs(5)), None);
        assert_eq!(lyrics.current_index(Duration::from_secs(10)), Some(0));
        assert_eq!(lyrics.current_index(Duration::from_secs(25)), Some(1));
    }

    #[test]
    fn plain_lyrics_are_unsynced() {
        let lyrics = Lyrics::parse("one\ntwo");
        assert!(!lyrics.is_synced());
        assert_eq!(lyrics.lines().len(), 2);
        assert_eq!(lyrics.current_index(Duration::from_secs(5)), None);
    }
}
