//! The pointer on a single-line field: a press places the caret, and a
//! drag, a shifted press or a run of presses selects.

use bevy_ecs::prelude::{On, Query, With, Without};
use plurimus_core::ratatui_core::buffer::CellWidth;
use plurimus_core::ratatui_core::layout::{Position, Rect};
use plurimus_ui::{ComputedDisabled, ComputedWidgetArea, PointerDrag, PointerPress, content_cell};
use unicode_segmentation::UnicodeSegmentation;

use super::field::{drawn_row, place_caret};
use super::input::{EditableText, TextMask};
use super::state::TextInput;
use super::word::word_around;

type Pointed<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut TextInput,
        Option<&'static TextMask>,
        &'static ComputedWidgetArea,
    ),
    (With<EditableText>, Without<ComputedDisabled>),
>;

pub(crate) fn text_input_press(event: On<PointerPress>, mut fields: Pointed) {
    let Ok((mut text, mask, area)) = fields.get_mut(event.entity) else {
        return;
    };
    let Some(target) = char_at(&text, mask, area.0, event.position) else {
        return;
    };
    match event.count {
        0 | 1 if event.modifiers.shift => text.select_to(target),
        0 | 1 => text.move_to(target),
        // A masked field selects all rather than a word, which would show
        // where the hidden value's words break.
        2 if mask.is_none() => {
            let word = word_around(text.value(), target);
            text.move_to(word.start);
            text.select_to(word.end);
        }
        _ => text.select_all(),
    }
}

/// Extends from wherever the press left the selection, a double click's
/// word included, one cluster at a time.
pub(crate) fn text_input_drag(event: On<PointerDrag>, mut fields: Pointed) {
    let Ok((mut text, mask, area)) = fields.get_mut(event.entity) else {
        return;
    };
    if let Some(target) = char_at(&text, mask, area.0, event.position) {
        text.select_to(target);
    }
}

/// The char index of the cluster drawn under `position`, or the end of the
/// value past its last cluster.
///
/// The row is windowed from the cursor, so the window's first column comes
/// from the same `place_caret` the stylist draws with rather than being
/// stored. A drawn cluster, masked or not, is one of the value's, which is
/// what maps a column back through a mask.
fn char_at(
    text: &TextInput,
    mask: Option<&TextMask>,
    area: Rect,
    position: Position,
) -> Option<usize> {
    let cell = content_cell(position, area, Position::ORIGIN)?;
    let row = drawn_row(text, mask.map(|TextMask(glyph)| *glyph));
    let column = place_caret(&row.value, row.cursor, area.width)
        .start
        .saturating_add(cell.x);
    let mut end = 0;
    let before = row
        .value
        .graphemes(true)
        .take_while(|cluster| {
            end += cluster.cell_width();
            end <= column
        })
        .count();
    Some(
        text.value()
            .graphemes(true)
            .take(before)
            .map(|cluster| cluster.chars().count())
            .sum(),
    )
}
