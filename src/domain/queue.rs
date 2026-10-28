use std::collections::HashSet;

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
///
/// Songs removed from the queue are simply left out of `order`; picking one
/// from the library again puts it back.
pub struct Queue {
    order: Vec<usize>,
    cursor: Option<usize>, // position within `order`
    /// Tracks added with "play next" that haven't played yet. They sit right
    /// after the current track and stay there when the order is rebuilt.
    pinned: HashSet<usize>,
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
            pinned: HashSet::new(),
            repeat: RepeatMode::Off,
            shuffle: false,
            rng,
        }
    }

    /// Library index of the current track.
    pub fn current(&self) -> Option<usize> {
        self.cursor.map(|c| self.order[c])
    }

    /// The tracks that will play after the current one, in order.
    /// Empty when nothing is playing.
    pub fn upcoming(&self) -> &[usize] {
        match self.cursor {
            Some(cursor) => &self.order[cursor + 1..],
            None => &[],
        }
    }

    /// True if the track was added with "play next" and hasn't played yet.
    pub fn is_pinned(&self, track: usize) -> bool {
        self.pinned.contains(&track)
    }

    /// Makes `track` the current one. In shuffle mode the rest of the
    /// queue is reshuffled after it, so every other track still comes up.
    /// A track that was removed from the queue rejoins it.
    pub fn jump_to(&mut self, track: usize) {
        if self.shuffle {
            self.reshuffle(Some(track));
            return;
        }
        self.pinned.remove(&track);
        let pinned = self.pinned_in_order(None);
        let position = match self.order.iter().position(|&t| t == track) {
            Some(position) => position,
            None => {
                // Put it back roughly where it sits in the library.
                let position = self
                    .order
                    .iter()
                    .position(|&t| t > track)
                    .unwrap_or(self.order.len());
                self.order.insert(position, track);
                position
            }
        };
        self.cursor = Some(position);
        self.put_after_current(pinned);
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
        self.settle();
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

    // ---- editing what comes next ----
    //
    // `index` below is a position in `upcoming()`: 0 is the next track.

    /// Puts `track` right after the current track, behind any earlier
    /// "play next" picks. Does nothing if nothing is playing or it's
    /// the current track.
    pub fn play_next(&mut self, track: usize) {
        let Some(mut cursor) = self.cursor else {
            return;
        };
        if self.order[cursor] == track {
            return;
        }
        if let Some(position) = self.order.iter().position(|&t| t == track) {
            self.order.remove(position);
            if position < cursor {
                cursor -= 1;
            }
        }
        let mut at = cursor + 1;
        while at < self.order.len() && self.pinned.contains(&self.order[at]) {
            at += 1;
        }
        self.order.insert(at, track);
        self.pinned.insert(track);
        self.cursor = Some(cursor);
    }

    /// Removes an upcoming track from the queue. Returns the removed track.
    pub fn remove_upcoming(&mut self, index: usize) -> Option<usize> {
        let position = self.cursor? + 1 + index;
        if position >= self.order.len() {
            return None;
        }
        let track = self.order.remove(position);
        self.pinned.remove(&track);
        Some(track)
    }

    /// Moves an upcoming track one place earlier or later.
    /// Returns its new index, or None if it can't move that way.
    pub fn move_upcoming(&mut self, index: usize, later: bool) -> Option<usize> {
        let count = self.upcoming().len();
        let target = if later {
            index + 1
        } else {
            index.checked_sub(1)?
        };
        if index >= count || target >= count {
            return None;
        }
        let first = self.cursor? + 1;
        self.order.swap(first + index, first + target);
        Some(target)
    }

    /// Plays an upcoming track now. The ones before it are passed over
    /// (`previous` still goes back to them). Returns the new current track.
    pub fn skip_to_upcoming(&mut self, index: usize) -> Option<usize> {
        let position = self.cursor? + 1 + index;
        if position >= self.order.len() {
            return None;
        }
        self.cursor = Some(position);
        self.settle();
        self.current()
    }

    // ---- modes ----

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    /// Turns shuffle on or off for the songs still in the queue. The current
    /// track keeps playing either way, and "play next" picks stay next.
    /// On: everything else gets a new random order. Off: library order,
    /// carrying on from the current track.
    pub fn toggle_shuffle(&mut self) {
        self.shuffle = !self.shuffle;
        let current = self.current();
        if self.shuffle {
            self.reshuffle(current);
        } else {
            let pinned = self.pinned_in_order(current);
            self.order.sort_unstable();
            self.cursor = current.and_then(|t| self.order.iter().position(|&o| o == t));
            self.put_after_current(pinned);
        }
    }

    pub fn cycle_repeat(&mut self) {
        self.repeat = match self.repeat {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        };
    }

    // ---- internal ----

    /// New order: `first` (if any) as the current track, then the pinned
    /// tracks as they were, then everything else shuffled.
    fn reshuffle(&mut self, first: Option<usize>) {
        let pinned: Vec<usize> = self.pinned_in_order(first);
        let mut rest: Vec<usize> = self
            .order
            .iter()
            .copied()
            .filter(|&t| Some(t) != first && !self.pinned.contains(&t))
            .collect();
        rest.sort_unstable(); // so the result depends only on the rng
        self.rng.shuffle(&mut rest);

        self.order.clear();
        self.cursor = first.map(|track| {
            self.order.push(track);
            0
        });
        self.order.extend(pinned);
        self.order.extend(rest);
        self.settle();
    }

    /// Moves `pinned` (in that order) to just after the current track.
    fn put_after_current(&mut self, pinned: Vec<usize>) {
        let Some(current) = self.current() else {
            return;
        };
        self.order.retain(|t| !pinned.contains(t));
        let cursor = self.order.iter().position(|&t| t == current).unwrap_or(0);
        self.cursor = Some(cursor);
        self.order.splice(cursor + 1..cursor + 1, pinned);
    }

    /// Pinned tracks in queue order, leaving out `except`.
    fn pinned_in_order(&self, except: Option<usize>) -> Vec<usize> {
        self.order
            .iter()
            .copied()
            .filter(|&t| self.pinned.contains(&t) && Some(t) != except)
            .collect()
    }

    /// After the cursor moves forward: anything at or before it has
    /// played or been passed over, so it's no longer a "play next" pick.
    fn settle(&mut self) {
        if let Some(cursor) = self.cursor {
            for track in &self.order[..=cursor] {
                self.pinned.remove(track);
            }
        }
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

    fn playing(track_count: usize, current: usize) -> Queue {
        let mut queue = shuffled_queue(track_count);
        queue.jump_to(current);
        queue
    }

    #[test]
    fn upcoming_lists_what_follows_the_current_track() {
        let queue = playing(5, 2);
        assert_eq!(queue.upcoming(), &[3, 4]);
        assert_eq!(shuffled_queue(5).upcoming(), &[] as &[usize]);
    }

    #[test]
    fn play_next_goes_after_current_in_the_order_picked() {
        let mut queue = playing(10, 2);
        queue.play_next(7);
        queue.play_next(5);
        queue.play_next(0); // already played: moves forward
        assert_eq!(queue.current(), Some(2));
        assert_eq!(&queue.upcoming()[..4], &[7, 5, 0, 3]);
        assert!(queue.is_pinned(7));

        queue.play_next(2); // the current track: nothing happens
        assert_eq!(queue.current(), Some(2));

        queue.next();
        assert_eq!(queue.current(), Some(7));
        assert!(!queue.is_pinned(7), "played tracks are no longer pinned");
    }

    #[test]
    fn play_next_does_nothing_when_nothing_is_playing() {
        let mut queue = shuffled_queue(3);
        queue.play_next(1);
        assert_eq!(queue.current(), None);
        assert!(!queue.is_pinned(1));
    }

    #[test]
    fn removed_tracks_stay_removed_through_shuffle_toggles() {
        let mut queue = playing(10, 0);
        assert_eq!(queue.remove_upcoming(2), Some(3));
        assert_eq!(queue.remove_upcoming(0), Some(1));
        assert_eq!(queue.remove_upcoming(99), None);

        queue.toggle_shuffle();
        let mut played = play_through(&mut queue);
        played.sort();
        assert_eq!(played, vec![0, 2, 4, 5, 6, 7, 8, 9]);

        queue.jump_to(0);
        queue.toggle_shuffle(); // off again
        assert_eq!(play_through(&mut queue), vec![0, 2, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn a_removed_track_rejoins_when_picked() {
        let mut queue = playing(6, 0);
        queue.remove_upcoming(2); // track 3
        queue.jump_to(3);
        assert_eq!(queue.current(), Some(3));
        assert_eq!(queue.upcoming(), &[4, 5]);

        queue.toggle_shuffle();
        queue.remove_upcoming(0);
        let removed: Vec<usize> = (0..6).filter(|t| !queue.order.contains(t)).collect();
        queue.jump_to(removed[0]);
        assert_eq!(queue.current(), Some(removed[0]));
    }

    #[test]
    fn play_next_picks_stay_next_after_shuffle_toggles() {
        let mut queue = playing(20, 4);
        queue.play_next(15);
        queue.play_next(9);

        queue.toggle_shuffle();
        assert_eq!(queue.current(), Some(4));
        assert_eq!(&queue.upcoming()[..2], &[15, 9]);

        queue.toggle_shuffle();
        assert_eq!(queue.current(), Some(4));
        assert_eq!(&queue.upcoming()[..3], &[15, 9, 5]);
    }

    #[test]
    fn picking_another_track_keeps_play_next_picks_next() {
        let mut queue = playing(10, 1);
        queue.play_next(8);
        queue.jump_to(5);
        assert_eq!(&queue.upcoming()[..2], &[8, 6]);
    }

    #[test]
    fn move_upcoming_swaps_and_stops_at_the_ends() {
        let mut queue = playing(5, 0); // upcoming: 1 2 3 4
        assert_eq!(queue.move_upcoming(0, true), Some(1));
        assert_eq!(queue.upcoming(), &[2, 1, 3, 4]);
        assert_eq!(queue.move_upcoming(3, false), Some(2));
        assert_eq!(queue.upcoming(), &[2, 1, 4, 3]);
        assert_eq!(queue.move_upcoming(0, false), None);
        assert_eq!(queue.move_upcoming(3, true), None);
        assert_eq!(queue.move_upcoming(9, true), None);
    }

    #[test]
    fn skip_to_upcoming_jumps_and_previous_goes_back() {
        let mut queue = playing(6, 0);
        queue.play_next(4);
        assert_eq!(queue.skip_to_upcoming(2), Some(2)); // upcoming was 4 1 2 3 5
        assert!(!queue.is_pinned(4), "passed-over picks are unpinned");
        assert_eq!(queue.upcoming(), &[3, 5]);
        assert_eq!(queue.previous(), Some(1));
        assert_eq!(queue.skip_to_upcoming(99), None);
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
