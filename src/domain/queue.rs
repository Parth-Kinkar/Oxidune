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
    shuffle: bool,
    rng: Rng,
}

impl Queue {
    pub fn new(track_count: usize) -> Self {
        Self::with_rng(track_count, Rng::from_entropy())
    }

    fn with_rng(track_count: usize, rng: Rng) -> Self {
        Self {
            order: (0..track_count).collect(),
            cursor: None,
            repeat: RepeatMode::Off,
            shuffle: false,
            rng,
        }
    }

    /// Library index of the current track.
    pub fn current(&self) -> Option<usize> {
        self.cursor.map(|c| self.order[c])
    }

    /// Makes `track` the current one. In shuffle mode the rest of the
    /// queue is reshuffled after it, so every other track still comes up.
    pub fn jump_to(&mut self, track: usize) {
        if self.shuffle {
            self.reshuffle(Some(track));
        } else {
            self.cursor = self.order.iter().position(|&t| t == track);
        }
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

    /// The track that will follow the current one, without moving to it.
    pub fn peek_next(&self) -> Option<usize> {
        let cursor = self.cursor?;
        if self.repeat == RepeatMode::One {
            return Some(self.order[cursor]);
        }
        match self.order.get(cursor + 1) {
            Some(&track) => Some(track),
            None if self.repeat == RepeatMode::All => self.order.first().copied(),
            None => None,
        }
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    /// Turns shuffle on or off. The current track keeps playing either way:
    /// on, it becomes the first of a new random order; off, the queue goes
    /// back to library order from where that track sits.
    pub fn toggle_shuffle(&mut self) {
        self.shuffle = !self.shuffle;
        let current = self.current();
        if self.shuffle {
            self.reshuffle(current);
        } else {
            self.order.sort_unstable();
            self.cursor = current.and_then(|t| self.order.iter().position(|&o| o == t));
        }
    }

    /// Random order with `first` (if any) at the front, cursor on it.
    fn reshuffle(&mut self, first: Option<usize>) {
        self.order.sort_unstable();
        self.rng.shuffle(&mut self.order);
        if let Some(track) = first {
            if let Some(position) = self.order.iter().position(|&t| t == track) {
                self.order.swap(0, position);
                self.cursor = Some(0);
                return;
            }
        }
        self.cursor = None;
    }

    pub fn cycle_repeat(&mut self) {
        self.repeat = match self.repeat {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        };
    }
}

/// A tiny xorshift random generator. Good enough for shuffling songs,
/// and saves pulling in a crate for one function.
struct Rng(u64);

impl Rng {
    fn from_entropy() -> Self {
        use std::collections::hash_map::RandomState;
        use std::hash::BuildHasher;
        Self::seeded(RandomState::new().hash_one(std::time::SystemTime::now()))
    }

    fn seeded(seed: u64) -> Self {
        Self(seed | 1) // xorshift must never be zero
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Fisher-Yates shuffle.
    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = (self.next() % (i as u64 + 1)) as usize;
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shuffled_queue(track_count: usize) -> Queue {
        Queue::with_rng(track_count, Rng::seeded(42))
    }

    /// Plays from the current track to the end, returning every track visited.
    fn play_through(queue: &mut Queue) -> Vec<usize> {
        let mut played: Vec<usize> = queue.current().into_iter().collect();
        while let Some(track) = queue.next() {
            played.push(track);
        }
        played
    }

    #[test]
    fn shuffle_keeps_the_current_track_and_plays_everything_once() {
        let mut queue = shuffled_queue(20);
        queue.jump_to(7);
        queue.toggle_shuffle();
        assert_eq!(queue.current(), Some(7));

        let mut played = play_through(&mut queue);
        assert_eq!(played[0], 7);
        assert_ne!(played, (7..20).collect::<Vec<_>>(), "order should change");
        played.sort();
        assert_eq!(played, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn jumping_while_shuffled_still_reaches_every_track() {
        let mut queue = shuffled_queue(20);
        queue.toggle_shuffle();
        queue.jump_to(19); // last in library order
        let mut played = play_through(&mut queue);
        played.sort();
        assert_eq!(played, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn turning_shuffle_off_resumes_library_order_from_current_track() {
        let mut queue = shuffled_queue(10);
        queue.toggle_shuffle();
        queue.jump_to(4);
        queue.next();
        let current = queue.current().unwrap();

        queue.toggle_shuffle();
        assert!(!queue.shuffle());
        assert_eq!(queue.current(), Some(current));
        assert_eq!(play_through(&mut queue), (current..10).collect::<Vec<_>>());
    }

    #[test]
    fn shuffle_with_nothing_playing_has_no_current_track() {
        let mut queue = shuffled_queue(5);
        queue.toggle_shuffle();
        assert_eq!(queue.current(), None);
        queue.jump_to(3);
        assert_eq!(queue.current(), Some(3));
    }

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

    #[test]
    fn peek_next_looks_ahead_without_moving() {
        let mut queue = Queue::new(3);
        queue.jump_to(0);
        assert_eq!(queue.peek_next(), Some(1));
        assert_eq!(queue.current(), Some(0));

        queue.jump_to(2);
        assert_eq!(queue.peek_next(), None);
        queue.cycle_repeat(); // All
        assert_eq!(queue.peek_next(), Some(0));
        queue.cycle_repeat(); // One
        assert_eq!(queue.peek_next(), Some(2));
    }
}
