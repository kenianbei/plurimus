//! Grapheme-cluster boundaries over char-indexed cursors.
//!
//! A reader sees one character where Rust sees several - a flag, a family
//! emoji, a letter with a combining accent - so a cursor stepping by char
//! lands inside what looks like a single glyph. These helpers snap to
//! cluster boundaries so editing never splits one.
//!
//! Both text widgets address text by char index - `TextInput::cursor` and
//! `TextArea`'s `DataCursor` column - so the helpers speak that unit:
//! snapped targets for the field's direct edits, scalar step counts for the
//! editor's repeated engine inputs.

use unicode_segmentation::UnicodeSegmentation;

/// Scalars in the grapheme cluster ending at `position`, or 0 at the start
/// of the value. A `position` inside a cluster yields the distance back to
/// that cluster's start.
pub(crate) fn cluster_len_before(value: &str, position: usize) -> usize {
    cluster_spans(value)
        .find(|(start, len)| position > *start && position <= start + len)
        .map_or(0, |(start, _)| position - start)
}

/// Scalars in the grapheme cluster starting at `position`, or 0 at the end
/// of the value. A `position` inside a cluster yields the distance forward
/// to that cluster's end.
pub(crate) fn cluster_len_after(value: &str, position: usize) -> usize {
    cluster_spans(value)
        .find(|(start, len)| position >= *start && position < start + len)
        .map_or(0, |(start, len)| start + len - position)
}

/// `position` unchanged if it already sits on a cluster boundary, else the
/// end of the cluster containing it.
///
/// Word boundaries are character-class based and cluster boundaries are
/// not, so a word target can land inside a cluster - `a#️⃣b` breaks between
/// `#` and the variation selector. Snapping keeps the cursor on boundaries
/// no matter which rule proposed the target.
pub(crate) fn snap_forward(value: &str, position: usize) -> usize {
    enclosing_cluster(value, position).map_or(position, |(start, len)| start + len)
}

/// `position` unchanged if it already sits on a cluster boundary, else the
/// start of the cluster containing it.
pub(crate) fn snap_backward(value: &str, position: usize) -> usize {
    enclosing_cluster(value, position).map_or(position, |(start, _)| start)
}

/// The `(start, char count)` of the cluster `position` falls strictly
/// inside, or `None` when it already sits on a boundary.
fn enclosing_cluster(value: &str, position: usize) -> Option<(usize, usize)> {
    cluster_spans(value).find(|(start, len)| position > *start && position < start + len)
}

/// Byte offset of the `index`th char, or the value's length past the end.
pub(crate) fn char_to_byte(value: &str, index: usize) -> usize {
    value
        .char_indices()
        .nth(index)
        .map_or(value.len(), |(byte, _)| byte)
}

/// Every cluster as `(start char index, char count)`, in order.
fn cluster_spans(value: &str) -> impl Iterator<Item = (usize, usize)> + '_ {
    let mut start = 0;
    value.graphemes(true).map(move |cluster| {
        let span = (start, cluster.chars().count());
        start += span.1;
        span
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCENT: &str = "e\u{301}";
    const FAMILY: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    const FLAG: &str = "\u{1F1EB}\u{1F1F7}";

    #[test]
    fn boundaries_report_zero() {
        assert_eq!(cluster_len_before(ACCENT, 0), 0);
        assert_eq!(cluster_len_after(ACCENT, 2), 0);
        assert_eq!(cluster_len_before("", 0), 0);
        assert_eq!(cluster_len_after("", 0), 0);
    }

    #[test]
    fn whole_clusters_are_spanned() {
        assert_eq!(cluster_len_before(ACCENT, 2), 2);
        assert_eq!(cluster_len_after(ACCENT, 0), 2);
        assert_eq!(cluster_len_before(FAMILY, 5), 5);
        assert_eq!(cluster_len_after(FAMILY, 0), 5);
        assert_eq!(cluster_len_before(FLAG, 2), 2);
        assert_eq!(cluster_len_after(FLAG, 0), 2);
    }

    #[test]
    fn mid_cluster_resolves_to_the_containing_boundary() {
        assert_eq!(cluster_len_before(FAMILY, 3), 3);
        assert_eq!(cluster_len_after(FAMILY, 3), 2);
    }

    #[test]
    fn ascii_steps_one_at_a_time() {
        assert_eq!(cluster_len_before("abc", 2), 1);
        assert_eq!(cluster_len_after("abc", 2), 1);
    }
}
