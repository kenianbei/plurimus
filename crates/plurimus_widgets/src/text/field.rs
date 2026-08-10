//! Single-row rendering for [`EditableText`](super::EditableText).
//!
//! The value is drawn into one row through a window that keeps the cursor
//! visible: the window's right edge is pinned to the cursor's trailing
//! column, so typing past the edge scrolls the text instead of letting the
//! cursor leave the field. The cursor is a reverse-video span over the
//! cluster it sits on rather than the terminal's own cursor, so the field
//! looks the same whether or not the terminal is drawing a caret.
//!
//! That is now a choice rather than the only option: `WidgetCursor` places
//! the terminal's own caret, which is what a screen reader follows. Moving
//! to it would change how every existing field looks, so it wants deciding
//! on its own rather than riding along with the seam that made it possible.

use plurimus_core::ratatui_core::buffer::{Buffer, CellWidth};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::{Modifier, Style};
use plurimus_core::ratatui_core::widgets::Widget;
use unicode_segmentation::UnicodeSegmentation;

/// The windowed single row: value, cursor as a char index, and the fill
/// style the row is painted with.
pub(super) struct TextField {
    pub(super) value: String,
    pub(super) cursor: usize,
    pub(super) style: Style,
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
        let (cursor_column, cursor_width) = cursor_span(&self.value, self.cursor);
        let window = Window {
            start: cursor_column
                .saturating_add(cursor_width)
                .saturating_sub(area.width),
            area,
        };
        for x in area.left()..area.right() {
            if let Some(cell) = buffer.cell_mut((x, area.y)) {
                cell.set_char(' ');
            }
        }
        buffer.set_style(area, self.style);
        render_window(&self.value, &window, buffer);
        let cursor = Rect::new(
            area.x + (cursor_column - window.start),
            area.y,
            cursor_width,
            1,
        );
        buffer.set_style(
            cursor.intersection(area),
            Style::default().add_modifier(Modifier::REVERSED),
        );
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
