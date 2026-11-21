//! A hit map for mouse support: what render laid out, read back by the click handler.
//!
//! Hit-testing should never recompute layout. Render records each rect it actually drew, with a
//! key naming what is there, into a [`HitMap`] the app keeps (in a `RefCell`, since render takes
//! `&self`); the mouse handler asks [`HitMap::at`] what is under the pointer. [`HitMap::clear`] at
//! the top of every frame keeps a pane that wasn't drawn from being hit. Extracted from yaks'
//! `src/tui/mouse.rs` (yaks-97a2), as a pattern any ratatui app can use.

use ratatui::layout::{Position, Rect};

/// The rects the last frame drew, each with what it holds. Later records win, so record an
/// overlay after what it covers.
#[derive(Debug, Clone)]
pub struct HitMap<K> {
    hits: Vec<(Rect, K)>,
}

impl<K> Default for HitMap<K> {
    fn default() -> Self {
        Self { hits: Vec::new() }
    }
}

impl<K: Clone> HitMap<K> {
    /// Forget the last frame: call at the top of render.
    pub fn clear(&mut self) {
        self.hits.clear();
    }

    /// `area` shows `key`.
    pub fn record(&mut self, area: Rect, key: K) {
        if area.width > 0 && area.height > 0 {
            self.hits.push((area, key));
        }
    }

    /// One `key` per row of `area`, from `first` on: `key(first + row)` for each drawn row, as a
    /// scrolled list's rows map back to its items.
    pub fn record_rows(&mut self, area: Rect, rows: usize, first: usize, key: impl Fn(usize) -> K) {
        for row in 0..rows.min(usize::from(area.height)) {
            #[allow(clippy::cast_possible_truncation)]
            let y = area.y + row as u16;
            self.record(
                Rect {
                    y,
                    height: 1,
                    ..area
                },
                key(first + row),
            );
        }
    }

    /// What is under (`column`, `row`), and the rect it was drawn in: the last record holding it.
    #[must_use]
    pub fn at(&self, column: u16, row: u16) -> Option<(Rect, K)> {
        self.hits
            .iter()
            .rev()
            .find(|(area, _)| area.contains(Position::new(column, row)))
            .map(|(area, key)| (*area, key.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_record_under_the_pointer_wins_and_clear_forgets() {
        let mut hits = HitMap::default();
        hits.record(Rect::new(0, 0, 20, 10), "list");
        hits.record(Rect::new(5, 2, 4, 2), "popup");
        assert_eq!(hits.at(6, 3).map(|h| h.1), Some("popup"));
        assert_eq!(hits.at(1, 1).map(|h| h.1), Some("list"));
        assert_eq!(hits.at(30, 1), None);
        hits.clear();
        assert_eq!(hits.at(1, 1), None);
    }

    #[test]
    fn rows_map_back_to_items_past_the_scroll() {
        let mut hits = HitMap::default();
        hits.record_rows(Rect::new(0, 2, 10, 3), 10, 40, |i| i);
        assert_eq!(hits.at(0, 2).map(|h| h.1), Some(40));
        assert_eq!(hits.at(9, 4).map(|h| h.1), Some(42));
        assert_eq!(hits.at(0, 5), None, "only as many rows as the area holds");
    }
}
