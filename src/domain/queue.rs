#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn label(self) -> &'static str {
        match self {
            RepeatMode::Off => "off",
            RepeatMode::All => "all",
            RepeatMode::One => "one",
        }
    }
}

/// The playback order: which library track plays after which.
/// Stores indices into the library, so it knows nothing about Track itself.
pub struct Queue {
    order: Vec<usize>,
    cursor: Option<usize>, // position within `order`
    repeat: RepeatMode,
}

impl Queue {
    pub fn new(track_count: usize) -> Self {
        Self {
            order: (0..track_count).collect(),
            cursor: None,
            repeat: RepeatMode::Off,
        }
    }

    /// Library index of the current track.
    pub fn current(&self) -> Option<usize> {
        self.cursor.map(|c| self.order[c])
    }

    pub fn jump_to(&mut self, track: usize) {
        self.cursor = self.order.iter().position(|&t| t == track);
    }

    /// Manual skip forward. Returns None if there is nothing after this track.
    pub fn next(&mut self) -> Option<usize> {
        let cursor = self.cursor?;
        let target = if cursor + 1 < self.order.len() {
            cursor + 1
        } else if self.repeat == RepeatMode::All {
            0
        } else {
            return None;
        };
        self.cursor = Some(target);
        self.current()
    }

    pub fn previous(&mut self) -> Option<usize> {
        let target = self.cursor?.checked_sub(1)?;
        self.cursor = Some(target);
        self.current()
    }

    /// Called when a track ends on its own. Honors repeat-one.
    pub fn on_track_end(&mut self) -> Option<usize> {
        if self.repeat == RepeatMode::One {
            return self.current();
        }
        let next = self.next();
        if next.is_none() {
            self.cursor = None;
        }
        next
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    pub fn cycle_repeat(&mut self) {
        self.repeat = match self.repeat {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_stops_at_the_end_without_repeat() {
        let mut queue = Queue::new(2);
        queue.jump_to(1);
        assert_eq!(queue.next(), None);
    }

    #[test]
    fn repeat_all_wraps_around() {
        let mut queue = Queue::new(2);
        queue.cycle_repeat(); // Off -> All
        queue.jump_to(1);
        assert_eq!(queue.next(), Some(0));
    }
}
