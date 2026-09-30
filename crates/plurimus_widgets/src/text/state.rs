//! The single-line field's editing state.
//!
//! [`TextInput`] owns its value and cursor rather than deferring to an
//! engine, which is what separates the field from [`TextEditor`] and its
//! ratatui-textarea backing. Keeping the state here means every edit passes
//! through one type that snaps to cluster boundaries, so callers may propose
//! any target - a word rule, a click, an arrow key - without each of them
//! having to know about grapheme clusters.

use std::ops::Range;

use bevy_ecs::prelude::Component;

use super::grapheme::{char_to_byte, snap_backward, snap_forward};

/// The field's editing state: value plus cursor as a char index resting
/// on grapheme-cluster boundaries, and the selection's other end.
///
/// [`move_to`](Self::move_to), [`select_to`](Self::select_to) and
/// [`delete_to`](Self::delete_to) snap a mid-cluster target to the boundary
/// away from the cursor, so an edit covers whole clusters no matter which
/// rule proposed the target.
///
/// Moving the cursor ends the selection, and every edit replaces it.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct TextInput {
    value: String,
    cursor: usize,
    // Never equal to `cursor`, so an empty selection is unrepresentable.
    anchor: Option<usize>,
}

impl TextInput {
    /// A field pre-filled with `value`, cursor at the end.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.chars().count();
        Self {
            value,
            cursor,
            anchor: None,
        }
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

    /// The selected chars, ordered, or `None` when nothing is selected.
    #[must_use]
    pub fn selection(&self) -> Option<Range<usize>> {
        let anchor = self.anchor?;
        Some(anchor.min(self.cursor)..anchor.max(self.cursor))
    }

    /// The selected text, or `None` when nothing is selected.
    #[must_use]
    pub fn selected_text(&self) -> Option<&str> {
        self.selection()
            .map(|range| &self.value[self.byte_range(range)])
    }

    /// Moves the cursor to `target`, snapped and clamped, extending the
    /// selection from where it started - or from the cursor, if nothing was
    /// selected. A move that lands back on the anchor selects nothing.
    pub fn select_to(&mut self, target: usize) {
        let anchor = self.anchor.unwrap_or(self.cursor);
        self.cursor = self.snapped(target);
        self.anchor = (anchor != self.cursor).then_some(anchor);
    }

    /// Selects the whole value, cursor at the end.
    pub fn select_all(&mut self) {
        self.cursor = self.value.chars().count();
        self.anchor = (self.cursor != 0).then_some(0);
    }

    /// Ends the selection, leaving the cursor where it is.
    pub const fn clear_selection(&mut self) {
        self.anchor = None;
    }

    /// Deletes the selection, leaving the cursor where it started, and
    /// reports whether there was one.
    pub fn delete_selection(&mut self) -> bool {
        let Some(range) = self.selection() else {
            return false;
        };
        self.value.replace_range(self.byte_range(range.clone()), "");
        self.cursor = range.start;
        self.anchor = None;
        true
    }

    /// Inserts `character` at the cursor, in place of the selection.
    pub fn insert(&mut self, character: char) {
        self.delete_selection();
        let byte = char_to_byte(&self.value, self.cursor);
        self.value.insert(byte, character);
        self.cursor += 1;
    }

    /// Inserts `text` at the cursor in one shift, in place of the selection,
    /// leaving the cursor after it. Kept off the public API because it admits
    /// the control characters [`paste`](Self::paste) exists to strip.
    pub(super) fn insert_str(&mut self, text: &str) {
        self.delete_selection();
        let byte = char_to_byte(&self.value, self.cursor);
        self.value.insert_str(byte, text);
        self.cursor += text.chars().count();
    }

    /// Moves the cursor to `target`, snapped and clamped, ending the
    /// selection.
    pub fn move_to(&mut self, target: usize) {
        self.cursor = self.snapped(target);
        self.anchor = None;
    }

    /// Moves the cursor to the start of the value, ending the selection.
    pub const fn move_start(&mut self) {
        self.cursor = 0;
        self.anchor = None;
    }

    /// Moves the cursor to the end of the value, ending the selection.
    pub fn move_end(&mut self) {
        self.cursor = self.value.chars().count();
        self.anchor = None;
    }

    /// Deletes the selection if there is one, and otherwise between the
    /// cursor and `target` (snapped and clamped), leaving the cursor at the
    /// start of the removed range.
    pub fn delete_to(&mut self, target: usize) {
        if self.delete_selection() {
            return;
        }
        let target = self.snapped(target);
        let (low, high) = (self.cursor.min(target), self.cursor.max(target));
        self.value.replace_range(self.byte_range(low..high), "");
        self.cursor = low;
    }

    fn byte_range(&self, chars: Range<usize>) -> Range<usize> {
        char_to_byte(&self.value, chars.start)..char_to_byte(&self.value, chars.end)
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
    fn selecting_back_orders_the_range_and_snaps_to_clusters() {
        let mut text = TextInput::new(format!("a{FAMILY}b"));
        text.select_to(4);
        assert_eq!(text.selection(), Some(1..7));
        assert_eq!(text.selected_text(), Some(&*format!("{FAMILY}b")));
    }

    #[test]
    fn selecting_back_to_the_anchor_selects_nothing() {
        let mut text = TextInput::new("ab");
        text.select_to(1);
        text.select_to(2);
        assert_eq!(text.selection(), None);
        text.select_all();
        text.move_end();
        assert_eq!(text.selection(), None, "a plain move ends it");
    }

    #[test]
    fn edits_replace_the_selection() {
        let mut text = TextInput::new("abcd");
        text.move_to(1);
        text.select_to(3);
        text.insert('X');
        assert_eq!((text.value(), text.cursor()), ("aXd", 2));

        text.select_all();
        text.delete_to(0);
        assert_eq!(
            text.value(),
            "",
            "a deletion takes the selection, not its own range"
        );
    }

    #[test]
    fn nothing_to_select_in_an_empty_value() {
        let mut text = TextInput::new("");
        text.select_all();
        assert_eq!(text.selection(), None);
        assert!(!text.delete_selection());
    }

    #[test]
    fn insert_lands_after_the_inserted_char() {
        let mut text = TextInput::new("ac");
        text.move_to(1);
        text.insert('b');
        assert_eq!((text.value(), text.cursor()), ("abc", 2));
    }
}
