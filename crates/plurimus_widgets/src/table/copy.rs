//! What a table's copy key puts on the clipboard.

use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::Children;
use bevy_ecs::prelude::{Has, On, Query, With, Without};
use bevy_input::keyboard::KeyboardInput;
use bevy_input_focus::FocusedInput;
use plurimus_term::bevy_compat::HeldModifiers;
use plurimus_ui::{Checked, ComputedDisabled, first_bound};

use super::{
    ActiveColumn, Table, TableAction, TableFooter, TableHeader, TableKeys, TableMultiSelect,
    TableRow, TableSelection,
};
use crate::clipboard::Clipboard;
use crate::rows::{ActiveDescendant, copied_rows};

type Copyable<'a> = (
    &'a Children,
    &'a TableSelection,
    &'a TableKeys,
    &'a ActiveDescendant,
    &'a ActiveColumn,
    Has<TableMultiSelect>,
);

/// The body rows a copy reads, and whether each is checked. The bands are
/// chrome, so a copy never takes them.
type BodyRows<'w, 's> =
    Query<'w, 's, (&'static TableRow, Has<Checked>), (Without<TableHeader>, Without<TableFooter>)>;

/// Copies what [`copied_rows`] picks when the key is bound to
/// [`TableAction::Copy`]: rows one per line, a row's cells tab-separated,
/// narrowed to the cursor's column when the selection tracks one. With
/// nothing to copy the key goes on to the table's ancestors.
pub(crate) fn table_copy(
    mut input: On<FocusedInput<KeyboardInput>>,
    held: HeldModifiers,
    tables: Query<Copyable, (With<Table>, Without<ComputedDisabled>)>,
    rows: BodyRows,
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
    let cursor = Cursor {
        selection: *selection,
        row: active.0,
        column: column.0,
    };
    let copied = copied_text((children, &rows), cursor, is_multi_select);
    if copied.is_empty() {
        return;
    }
    clipboard.offer(&copied);
    input.propagate(false);
}

/// Where a table's cursor is, in the terms its selection tracks.
struct Cursor {
    selection: TableSelection,
    row: Option<Entity>,
    column: Option<usize>,
}

fn copied_text(
    (children, rows): (&Children, &BodyRows),
    Cursor {
        selection,
        row: active,
        column,
    }: Cursor,
    is_multi_select: bool,
) -> String {
    if selection.tracks_column() && column.is_none() {
        return String::new();
    }
    let column = column.filter(|_| selection.tracks_column());
    let body: Vec<((Entity, &TableRow), bool)> = children
        .iter()
        .filter_map(|&child| {
            let (row, checked) = rows.get(child).ok()?;
            Some(((child, row), checked))
        })
        .collect();
    // A selection that tracks no row is a whole column's worth of them.
    let tracks_row = selection.tracks_row();
    copied_rows(body, is_multi_select, |&(child, _)| {
        !tracks_row || active == Some(child)
    })
    .map(|(_, row)| row_text(row, column))
    .collect::<Vec<_>>()
    .join("\n")
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
