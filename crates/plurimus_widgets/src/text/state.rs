//! The single-line field's editing state.
//!
//! [`TextInput`] owns its value and cursor rather than deferring to an
//! engine, which is what separates the field from [`TextEditor`] and its
//! ratatui-textarea backing. Keeping the state here means every edit passes
//! through one type that snaps to cluster boundaries, so callers may propose
//! any target - a word rule, a click, an arrow key - without each of them
//! having to know about grapheme clusters.

use bevy_ecs::prelude::Component;

use super::grapheme::{char_to_byte, snap_backward, snap_forward};

/// The field's editing state: value plus cursor as a char index resting
/// on grapheme-cluster boundaries.
///
/// [`move_to`](Self::move_to) and [`delete_to`](Self::delete_to) snap a
/// mid-cluster target to the boundary away from the cursor, so an edit
/// covers whole clusters no matter which rule proposed the target.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct TextInput {
    value: String,
    cursor: usize,
}

impl TextInput {
    /// A field pre-filled with `value`, cursor at the end.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.chars().count();
        Self { value, cursor }
    }

    /// The current text.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The cursor as a char index into the value.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Inserts `character` at the cursor.
    pub fn insert(&mut self, character: char) {
        let byte = char_to_byte(&self.value, self.cursor);
        self.value.insert(byte, character);
        self.cursor += 1;
    }

    /// Inserts `text` at the cursor in one shift, leaving the cursor after
    /// it. Kept off the public API because it admits the control characters
    /// [`paste`](Self::paste) exists to strip.
    pub(super) fn insert_str(&mut self, text: &str) {
        let byte = char_to_byte(&self.value, self.cursor);
        self.value.insert_str(byte, text);
        self.cursor += text.chars().count();
    }

    /// Moves the cursor to `target`, snapped and clamped.
    pub fn move_to(&mut self, target: usize) {
        self.cursor = self.snapped(target);
    }

    /// Moves the cursor to the start of the value.
    pub const fn move_start(&mut self) {
        self.cursor = 0;
    }

    /// Moves the cursor to the end of the value.
    pub fn move_end(&mut self) {
        self.cursor = self.value.chars().count();
    }

    /// Deletes between the cursor and `target` (snapped and clamped),
    /// leaving the cursor at the start of the removed range.
    pub fn delete_to(&mut self, target: usize) {
        let target = self.snapped(target);
        let (low, high) = (self.cursor.min(target), self.cursor.max(target));
        let range = char_to_byte(&self.value, low)..char_to_byte(&self.value, high);
        self.value.replace_range(range, "");
        self.cursor = low;
    }

    /// `target` clamped to the value, then moved to the cluster boundary
    /// *away* from the cursor, so an edit always covers whole clusters.
    fn snapped(&self, target: usize) -> usize {
        let target = target.min(self.value.chars().count());
        if target > self.cursor {
            snap_forward(&self.value, target)
        } else {
            snap_backward(&self.value, target)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAMILY: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";

    #[test]
    fn mid_cluster_moves_snap_away_from_the_cursor() {
        let mut text = TextInput::new(format!("a{FAMILY}b"));
        text.move_start();
        text.move_to(3);
        assert_eq!(text.cursor(), 6, "forward snaps to the cluster end");
        text.move_to(3);
        assert_eq!(text.cursor(), 1, "backward snaps to the cluster start");
    }

    #[test]
    fn mid_cluster_deletion_takes_the_whole_cluster() {
        let mut text = TextInput::new(format!("a{FAMILY}b"));
        text.move_start();
        text.move_to(1);
        text.delete_to(4);
        assert_eq!(text.value(), "ab");
        assert_eq!(text.cursor(), 1);
    }

    #[test]
    fn targets_clamp_to_the_value() {
        let mut text = TextInput::new("ab");
        text.move_to(usize::MAX);
        assert_eq!(text.cursor(), 2);
        text.delete_to(usize::MAX);
        assert_eq!(text.value(), "ab", "cursor at end deletes nothing");
    }

    #[test]
    fn insert_lands_after_the_inserted_char() {
        let mut text = TextInput::new("ac");
        text.move_to(1);
        text.insert('b');
        assert_eq!((text.value(), text.cursor()), ("abc", 2));
    }
}
