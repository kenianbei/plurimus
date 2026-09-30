//! Single-row rendering for [`EditableText`](super::EditableText).
//!
//! The value is drawn into one row through a window that keeps the cursor
//! visible: the window's right edge is pinned to the cursor's trailing
//! column, so typing past the edge scrolls the text instead of letting the
//! cursor leave the field. The caret is a style patched over the cluster it
//! sits on rather than the terminal's own cursor, so the field looks the
//! same whether or not the terminal is drawing a caret.
//!
//! That is a choice rather than the only option: `WidgetCursor` places the
//! terminal's own caret, which is what a screen reader follows. Moving to it
//! would change how every existing field looks, so it wants deciding on its
//! own rather than riding along with the seam that made it possible.

use plurimus_core::ratatui_core::buffer::{Buffer, CellWidth};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::widgets::Widget;
use unicode_segmentation::UnicodeSegmentation;

use super::grapheme::char_to_byte;

/// The windowed single row: value, cursor as a char index, the fill style
/// the row is painted with, and the caret's style - `None` for a field
/// without focus, which draws no caret at all rather than one more block
/// competing with whichever field the keys actually reach.
pub(super) struct TextField {
    pub(super) value: String,
    pub(super) cursor: usize,
    pub(super) style: Style,
    pub(super) caret: Option<Style>,
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
        let caret_span = place_caret(&self.value, self.cursor, area.width);
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
        render_window(&self.value, &window, buffer);
        let Some(caret) = self.caret else {
            return;
        };
        let cursor = Rect::new(area.x + caret_span.column, area.y, caret_span.width, 1);
        buffer.set_style(cursor.intersection(area), caret);
    }
}

/// Where the caret sits in a row `row_width` cells wide: `column` cells into
/// the row and `width` cells wide, with the value drawn from column `start`.
pub(super) struct CaretSpan {
    pub(super) start: u16,
    pub(super) column: u16,
    pub(super) width: u16,
}

/// Pins the window's right edge to the cursor's trailing column, so typing
/// past the edge scrolls the value rather than losing the caret.
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

/// One `mask` per grapheme cluster. The cursor is a char index, and a
/// multi-scalar cluster masks to one char, so it maps to a cluster count.
pub(super) fn mask_value(value: &str, cursor: usize, mask: char) -> (String, usize) {
    let masked = value.graphemes(true).map(|_| mask).collect();
    let masked_cursor = value[..char_to_byte(value, cursor)].graphemes(true).count();
    (masked, masked_cursor)
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
