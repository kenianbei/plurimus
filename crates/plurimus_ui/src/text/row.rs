//! Single-row rendering for a [`TextInput`].
//!
//! The value is drawn into one row through a window that keeps the cursor
//! visible: the window's right edge is pinned to the cursor's trailing
//! column, so typing past the edge scrolls the text instead of letting the
//! cursor leave the field. The caret is a style patched over the cluster it
//! sits on, so a terminal drawing no cursor still shows where typing goes;
//! a host publishes the same cell through `WidgetCursor` for the
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
use crate::UiTheme;

/// The windowed single row: what it draws, the fill style the row is
/// painted with, the caret's style - `None` for a field without focus,
/// which draws no caret at all rather than one more block competing with
/// whichever field the keys actually reach, and for one with a selection -
/// and the selection's.
///
/// Drawn into a one-row area, which it clears and fills before stamping the
/// window of the value that keeps the caret visible.
#[derive(Debug, Clone)]
pub struct TextRow {
    /// What the row draws.
    pub row: DrawnRow,
    /// The fill the whole row is painted with.
    pub style: Style,
    /// The caret's patch, or `None` to draw none.
    pub caret: Option<Style>,
    /// The patch over every selected cluster.
    pub selection_style: Style,
}

/// The text a row draws, with the cursor and selection as char indices
/// into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawnRow {
    /// The drawn text: the value, or its mask.
    pub value: String,
    /// The cursor, a char index into `value`.
    pub cursor: usize,
    /// The selection, char indices into `value`.
    pub selection: Option<Range<usize>>,
}

impl TextRow {
    /// `row` filled with `style`, its selection patched with the theme's and
    /// its caret with the theme's only while `is_focused` and nothing is
    /// selected: a patch adds a modifier rather than toggling one, so a caret
    /// over a selection would vanish, and the terminal's own cursor still
    /// marks the end that moves.
    #[must_use]
    pub fn new(row: DrawnRow, style: Style, is_focused: bool, theme: &UiTheme) -> Self {
        let has_caret = is_focused && row.selection.is_none();
        Self {
            row,
            style,
            caret: has_caret.then_some(theme.caret),
            selection_style: theme.selection,
        }
    }
}

impl DrawnRow {
    /// The caret's cell in a row `width` cells wide, windowed as the row is
    /// drawn: the cell a host publishes for the terminal's own cursor.
    #[must_use]
    pub fn caret_column(&self, width: u16) -> u16 {
        place_caret(&self.value, self.cursor, width).column
    }
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

impl Widget for &TextRow {
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
struct CaretSpan {
    start: u16,
    column: u16,
    width: u16,
}

// The window's right edge is pinned to the caret's trailing column, so the
// caret never leaves the row.
fn place_caret(value: &str, cursor: usize, row_width: u16) -> CaretSpan {
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

impl TextInput {
    /// The row this draws: its value, or one `mask` per grapheme cluster.
    ///
    /// A multi-scalar cluster masks to one char, so under a mask a char index
    /// maps to the count of clusters before it.
    #[must_use]
    pub fn row(&self, mask: Option<char>) -> DrawnRow {
        let value = self.value();
        let Some(mask) = mask else {
            return DrawnRow {
                value: value.to_owned(),
                cursor: self.cursor(),
                selection: self.selection(),
            };
        };
        let masked = |index| value[..char_to_byte(value, index)].graphemes(true).count();
        DrawnRow {
            value: value.graphemes(true).map(|_| mask).collect(),
            cursor: masked(self.cursor()),
            selection: self
                .selection()
                .map(|range| masked(range.start)..masked(range.end)),
        }
    }

    /// The value's char index for the cluster drawn `cell` columns into a row
    /// `row_width` wide, or the value's end past its last cluster:
    /// [`row`](Self::row) run backwards, through the same window the row is
    /// drawn in.
    ///
    /// A drawn cluster, masked or not, is one of the value's, which is what
    /// maps a column back through a mask.
    #[must_use]
    pub fn char_at_cell(&self, mask: Option<char>, row_width: u16, cell: u16) -> usize {
        let row = self.row(mask);
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
        cluster_spans(self.value())
            .nth(before)
            .map_or_else(|| self.value().chars().count(), |(start, _)| start)
    }
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
