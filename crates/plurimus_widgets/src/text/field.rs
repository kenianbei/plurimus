//! Single-row rendering for [`EditableText`](super::EditableText).
//!
//! The value is drawn into one row through a window that keeps the cursor
//! visible: the window's right edge is pinned to the cursor's trailing
//! column, so typing past the edge scrolls the text instead of letting the
//! cursor leave the field. The caret is a style patched over the cluster it
//! sits on, so a terminal drawing no cursor still shows where typing goes;
//! the stylist publishes the same cell through `WidgetCursor` for the
//! terminal's own, which is what a screen reader follows. A selection is a
//! style patched the same way, over every cluster it covers.

use std::ops::Range;

use plurimus_core::ratatui_core::buffer::{Buffer, CellWidth};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::widgets::Widget;
use unicode_segmentation::UnicodeSegmentation;

use super::grapheme::{char_to_byte, cluster_spans};
use super::state::TextInput;

/// The windowed single row: what it draws, the fill style the row is
/// painted with, the caret's style - `None` for a field without focus,
/// which draws no caret at all rather than one more block competing with
/// whichever field the keys actually reach, and for one with a selection -
/// and the selection's.
pub(super) struct TextField {
    pub(super) row: DrawnRow,
    pub(super) style: Style,
    pub(super) caret: Option<Style>,
    pub(super) selection_style: Style,
}

/// The text a row draws, with the cursor and selection as char indices
/// into it.
pub(super) struct DrawnRow {
    pub(super) value: String,
    pub(super) cursor: usize,
    pub(super) selection: Option<Range<usize>>,
}

struct Window {
    start: u16,
    area: Rect,
}

impl Window {
    const fn end(&self) -> u16 {
        self.start.saturating_add(self.area.width)
    }
}

impl Widget for &TextField {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let row = &self.row;
        let caret_span = place_caret(&row.value, row.cursor, area.width);
        let window = Window {
            start: caret_span.start,
            area,
        };
        for x in area.left()..area.right() {
            if let Some(cell) = buffer.cell_mut((x, area.y)) {
                cell.set_char(' ');
            }
        }
        buffer.set_style(area, self.style);
        render_window(&row.value, &window, buffer);
        if let Some(selected) = selection_rect(row, &window) {
            buffer.set_style(selected, self.selection_style);
        }
        let Some(caret) = self.caret else {
            return;
        };
        let cursor = Rect::new(area.x + caret_span.column, area.y, caret_span.width, 1);
        buffer.set_style(cursor.intersection(area), caret);
    }
}

fn selection_rect(row: &DrawnRow, window: &Window) -> Option<Rect> {
    let selection = row.selection.as_ref()?;
    let [left, right] = [selection.start, selection.end].map(|index| {
        let (column, _) = cursor_span(&row.value, index);
        column.saturating_sub(window.start).min(window.area.width)
    });
    let area = window.area;
    Some(Rect::new(area.x + left, area.y, right - left, 1))
}

/// Where the caret sits in its row: `column` cells in and `width` wide, with
/// the value drawn from its column `start`.
pub(super) struct CaretSpan {
    pub(super) start: u16,
    pub(super) column: u16,
    pub(super) width: u16,
}

pub(super) fn place_caret(value: &str, cursor: usize, row_width: u16) -> CaretSpan {
    let (column, width) = cursor_span(value, cursor);
    let start = column
        .saturating_add(width)
        .saturating_sub(row_width)
        .min(column);
    CaretSpan {
        start,
        column: column - start,
        width,
    }
}

// Per-scalar widths over-count a ZWJ sequence; `cell_width` is the measure
// the buffer itself uses. A cursor past the last cluster gets one blank cell.
fn cursor_span(value: &str, cursor: usize) -> (u16, u16) {
    let mut scalars = 0;
    let mut column: u16 = 0;
    for cluster in value.graphemes(true) {
        if scalars >= cursor {
            return (column, cluster.cell_width().max(1));
        }
        scalars += cluster.chars().count();
        column = column.saturating_add(cluster.cell_width());
    }
    (column, 1)
}

/// The row `text` draws: its value, or one `mask` per grapheme cluster.
///
/// A multi-scalar cluster masks to one char, so under a mask a char index
/// maps to the count of clusters before it.
pub(super) fn drawn_row(text: &TextInput, mask: Option<char>) -> DrawnRow {
    let value = text.value();
    let Some(mask) = mask else {
        return DrawnRow {
            value: value.to_owned(),
            cursor: text.cursor(),
            selection: text.selection(),
        };
    };
    let masked = |index| value[..char_to_byte(value, index)].graphemes(true).count();
    DrawnRow {
        value: value.graphemes(true).map(|_| mask).collect(),
        cursor: masked(text.cursor()),
        selection: text
            .selection()
            .map(|range| masked(range.start)..masked(range.end)),
    }
}

/// The value's char index for the cluster drawn `cell` columns into a row
/// `row_width` wide, or the value's end past its last cluster: `drawn_row`
/// and `cursor_span` run backwards, the window placed by the same
/// `place_caret` the row is drawn with.
///
/// A drawn cluster, masked or not, is one of the value's, which is what
/// maps a column back through a mask.
pub(super) fn char_at_cell(
    text: &TextInput,
    mask: Option<char>,
    row_width: u16,
    cell: u16,
) -> usize {
    let row = drawn_row(text, mask);
    let column = place_caret(&row.value, row.cursor, row_width)
        .start
        .saturating_add(cell);
    let mut end: u16 = 0;
    let before = row
        .value
        .graphemes(true)
        .take_while(|cluster| {
            end = end.saturating_add(cluster.cell_width());
            end <= column
        })
        .count();
    cluster_spans(text.value())
        .nth(before)
        .map_or_else(|| text.value().chars().count(), |(start, _)| start)
}

fn render_window(value: &str, window: &Window, buffer: &mut Buffer) {
    let mut column: u16 = 0;
    for cluster in value.graphemes(true) {
        column = column.saturating_add(stamp(cluster, column, window, buffer));
    }
}

// Not Buffer::set_stringn, which resets continuation cells and would drop
// the field's fill style from the second column of every wide cluster; the
// empty symbol is what marks a continuation for the presenter's diff.
fn stamp(cluster: &str, column: u16, window: &Window, buffer: &mut Buffer) -> u16 {
    let width = cluster.cell_width();
    let end = column.saturating_add(width);
    if width == 0 || column < window.start || end > window.end() {
        return width;
    }
    let x = window.area.x + column - window.start;
    let Some(cell) = buffer.cell_mut((x, window.area.y)) else {
        return width;
    };
    cell.set_symbol(cluster);
    for continuation in 1..width {
        if let Some(hidden) = buffer.cell_mut((x + continuation, window.area.y)) {
            hidden.set_symbol("");
        }
    }
    width
}
