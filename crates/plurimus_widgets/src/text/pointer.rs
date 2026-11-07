//! The pointer on a single-line field: a press places the caret, and a
//! drag, a shifted press or a run of presses selects.

use bevy_ecs::prelude::{On, Query, Res, With, Without};
use plurimus_core::CellSize;
use plurimus_core::ratatui_core::layout::{Position, Rect};
use plurimus_term::PointerModifiers;
use plurimus_ui::bevy_picking::events::{PointerDrag, PointerPress};
use plurimus_ui::{ComputedDisabled, ComputedWidgetArea, content_cell, pointer_cell};

use super::field::char_at_cell;
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

pub(crate) fn text_input_press(
    mut event: On<PointerPress>,
    cell: Res<CellSize>,
    modifiers: Res<PointerModifiers>,
    mut fields: Pointed,
) {
    let Ok((mut text, mask, area)) = fields.get_mut(event.entity) else {
        return;
    };
    event.propagate(false);
    let Some(target) = char_at(&text, mask, area.0, pointer_cell(&event.pointer, *cell)) else {
        return;
    };
    // A shifted press extends whatever its count, since the run it would
    // belong to is the field's and a click anywhere in it counts.
    match event.count {
        _ if modifiers.0.shift => text.select_to(target),
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
pub(crate) fn text_input_drag(
    mut event: On<PointerDrag>,
    cell: Res<CellSize>,
    mut fields: Pointed,
) {
    let Ok((mut text, mask, area)) = fields.get_mut(event.entity) else {
        return;
    };
    event.propagate(false);
    if let Some(target) = char_at(&text, mask, area.0, pointer_cell(&event.pointer, *cell)) {
        text.select_to(target);
    }
}

fn char_at(
    text: &TextInput,
    mask: Option<&TextMask>,
    area: Rect,
    position: Position,
) -> Option<usize> {
    let cell = content_cell(position, area, None)?;
    let mask = mask.map(|TextMask(glyph)| *glyph);
    Some(char_at_cell(text, mask, area.width, cell.x))
}
