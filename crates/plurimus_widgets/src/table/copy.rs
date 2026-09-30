//! What a table's copy key puts on the clipboard.

use bevy_ecs::hierarchy::Children;
use bevy_ecs::prelude::{Has, On, Query};
use bevy_input::keyboard::KeyboardInput;
use bevy_input_focus::FocusedInput;
use plurimus_term::bevy_compat::HeldModifiers;
use plurimus_ui::{Checked, first_bound};

use super::geometry::BodyRow;
use super::input::Interactive;
use super::{ActiveColumn, TableAction, TableKeys, TableMultiSelect, TableRow, TableSelection};
use crate::clipboard::Clipboard;
use crate::rows::{ActiveDescendant, copied_text};

type Copyable<'a> = (
    &'a Children,
    &'a TableSelection,
    &'a TableKeys,
    &'a ActiveDescendant,
    &'a ActiveColumn,
    Has<TableMultiSelect>,
);

/// Copies what [`copied_text`] picks when the key is bound to
/// [`TableAction::Copy`]: a row's cells tab-separated, or its one cell
/// when the selection tracks a column. The bands are chrome, so a copy
/// never takes them. With nothing to copy the key goes on to the table's
/// ancestors.
pub(crate) fn table_copy(
    mut input: On<FocusedInput<KeyboardInput>>,
    held: HeldModifiers,
    tables: Query<Copyable, Interactive>,
    rows: Query<(&TableRow, Has<Checked>), BodyRow>,
    mut clipboard: Clipboard,
) {
    let Ok((children, selection, keys, active, column, is_multi_select)) =
        tables.get(input.focused_entity)
    else {
        return;
    };
    if first_bound(&keys.0, &input.input, held.get()) != Some(TableAction::Copy) {
        return;
    }
    // A selection tracking a column copies nothing until it has one.
    let column = match (selection.tracks_column(), column.0) {
        (true, None) => return,
        (tracks_column, column) => column.filter(|_| tracks_column),
    };
    // A selection tracking no row copies a whole column's worth of them.
    let tracks_row = selection.tracks_row();
    let body = children
        .iter()
        .filter_map(|&child| Some((child, rows.get(child).ok()?.1)));
    let copied = copied_text(
        body,
        is_multi_select,
        |child| !tracks_row || active.0 == Some(child),
        |child| {
            rows.get(child)
                .map_or_else(|_| String::new(), |(row, _)| row_text(row, column))
        },
    );
    if clipboard.offer(&copied) {
        input.propagate(false);
    }
}

fn row_text(row: &TableRow, column: Option<usize>) -> String {
    match column {
        Some(column) => row
            .0
            .get(column)
            .map(ToString::to_string)
            .unwrap_or_default(),
        None => row
            .0
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\t"),
    }
}
