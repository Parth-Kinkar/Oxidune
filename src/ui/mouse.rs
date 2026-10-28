use std::time::{Duration, Instant};

use ratatui::layout::{Position, Rect};

/// Two clicks on the same thing within this time count as a double click.
/// Terminals only report single clicks, so we time them ourselves.
pub const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// Where the clickable parts of the screen were drawn in the last frame.
/// The views fill this in while rendering; the app reads it to work out
/// what a click landed on. A panel that wasn't drawn stays `None`.
#[derive(Debug, Default, Clone, Copy)]
pub struct Regions {
    /// The rows of the library list, inside its border.
    pub library: Option<Rect>,
    pub search: Option<Rect>,
    /// The text rows of the lyrics panel, and which lyric line is on the first row.
    pub lyrics: Option<(Rect, usize)>,
    /// The rows of the Up next list, inside its border.
    pub queue: Option<Rect>,
    /// Row of the "shuffled ↓" line in the Up next list (in list rows, not
    /// screen rows), if it's showing.
    pub queue_separator: Option<usize>,
    /// The "Lyrics" and "Up next" tabs in the side panel's title.
    pub lyrics_tab: Option<Rect>,
    pub queue_tab: Option<Rect>,
    /// The "▶ Artist — Title" line.
    pub now_playing: Option<Rect>,
    pub progress: Option<Rect>,
}

/// True if the cell at (column, row) is inside `area`.
pub fn hit(area: Option<Rect>, column: u16, row: u16) -> bool {
    area.is_some_and(|area| area.contains(Position::new(column, row)))
}

/// Which item of a one-line-per-item list is under (column, row),
/// given the index of the item drawn on the list's first row.
pub fn item_at(area: Rect, first: usize, column: u16, row: u16) -> Option<usize> {
    area.contains(Position::new(column, row))
        .then(|| first + usize::from(row - area.y))
}

/// Turns a row of a list that has a non-item line (like "shuffled ↓") at
/// `separator` into the index of the item on that row. None for the line itself.
pub fn skip_separator(row: usize, separator: Option<usize>) -> Option<usize> {
    match separator {
        Some(line) if row == line => None,
        Some(line) if row > line => Some(row - 1),
        _ => Some(row),
    }
}

/// How far along `area` the column is, from 0.0 (left edge) to 1.0 (right
/// edge). Columns outside the area are clamped, so dragging past the end
/// of the progress bar still means "the end".
pub fn fraction_at(area: Rect, column: u16) -> f64 {
    if area.width <= 1 {
        return 0.0;
    }
    let offset = f64::from(column.saturating_sub(area.x));
    (offset / f64::from(area.width - 1)).clamp(0.0, 1.0)
}

/// Spots double clicks. `T` says what was clicked, e.g. which list and row.
#[derive(Debug)]
pub struct DoubleClick<T> {
    last: Option<(Instant, T)>,
}

impl<T> Default for DoubleClick<T> {
    fn default() -> Self {
        Self { last: None }
    }
}

impl<T: Copy + PartialEq> DoubleClick<T> {
    /// Records a click on `item`. Returns true if it completes a double click.
    pub fn click(&mut self, item: T, now: Instant) -> bool {
        let double = matches!(
            self.last,
            Some((at, last)) if last == item && now.duration_since(at) <= DOUBLE_CLICK
        );
        // A third quick click starts a new pair instead of counting again.
        self.last = if double { None } else { Some((now, item)) };
        double
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_at_maps_rows_to_items_including_scroll() {
        let list = Rect::new(1, 1, 20, 5); // rows 1..=5
        assert_eq!(item_at(list, 0, 3, 1), Some(0));
        assert_eq!(item_at(list, 0, 3, 5), Some(4));
        assert_eq!(item_at(list, 10, 3, 2), Some(11)); // scrolled down by 10
        assert_eq!(item_at(list, 0, 3, 0), None); // the border above
        assert_eq!(item_at(list, 0, 3, 6), None); // below
        assert_eq!(item_at(list, 0, 0, 2), None); // left of it
    }

    #[test]
    fn fraction_at_spans_the_bar_and_clamps() {
        let bar = Rect::new(10, 0, 101, 1); // columns 10..=110
        assert_eq!(fraction_at(bar, 10), 0.0);
        assert_eq!(fraction_at(bar, 60), 0.5);
        assert_eq!(fraction_at(bar, 110), 1.0);
        assert_eq!(fraction_at(bar, 2), 0.0);
        assert_eq!(fraction_at(bar, 500), 1.0);
        assert_eq!(fraction_at(Rect::new(0, 0, 1, 1), 0), 0.0);
    }

    #[test]
    fn skip_separator_maps_rows_around_the_line() {
        assert_eq!(skip_separator(0, Some(2)), Some(0));
        assert_eq!(skip_separator(1, Some(2)), Some(1));
        assert_eq!(skip_separator(2, Some(2)), None);
        assert_eq!(skip_separator(3, Some(2)), Some(2));
        assert_eq!(skip_separator(5, None), Some(5));
    }

    #[test]
    fn hit_checks_the_area() {
        let area = Some(Rect::new(5, 5, 2, 2));
        assert!(hit(area, 5, 5));
        assert!(hit(area, 6, 6));
        assert!(!hit(area, 7, 5));
        assert!(!hit(None, 5, 5));
    }

    #[test]
    fn double_click_needs_same_item_within_the_window() {
        let start = Instant::now();
        let mut clicks = DoubleClick::default();

        assert!(!clicks.click(3, start));
        assert!(clicks.click(3, start + Duration::from_millis(200)));

        // a third quick click begins a new pair
        assert!(!clicks.click(3, start + Duration::from_millis(300)));

        // too slow
        assert!(!clicks.click(4, start + Duration::from_secs(2)));
        assert!(!clicks.click(4, start + Duration::from_secs(3)));

        // different item
        assert!(!clicks.click(5, start + Duration::from_secs(4)));
        assert!(!clicks.click(6, start + Duration::from_millis(4100)));
    }
}
