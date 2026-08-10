//! Table interaction: keys through the app's own map, clicks through the
//! column layout that [`geometry`](super::geometry) resolves.

use bevy_ecs::change_detection::{DetectChangesMut, Mut};
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::Children;
use bevy_ecs::prelude::{Commands, On, Query, With, Without};
use bevy_input::ButtonState;
use bevy_input::keyboard::KeyboardInput;
use bevy_input_focus::FocusedInput;
use plurimus_core::ratatui_core::layout::Rect;

use super::geometry::{
    Placed, Placement, Rows, bands, body_height, body_rows, clicked_column, clicked_row,
    column_count, resolved_widths,
};
use super::{
    ActiveColumn, Table, TableAction, TableColumns, TableHeaderClick, TableKeys, TablePosition,
    TableSelection, cursor_gutter,
};
use crate::listbox::ActiveDescendant;
use plurimus_ui::{
    ComputedWidgetArea, InteractionDisabled, PointerPress, ScrollIntoView, ValueChange,
};

type Navigable<'a> = (
    &'a Children,
    &'a TableColumns,
    &'a TableSelection,
    &'a TableKeys,
    &'a ComputedWidgetArea,
    &'a mut ActiveDescendant,
    &'a mut ActiveColumn,
);

type Pressable<'a> = (
    &'a Children,
    &'a TableColumns,
    &'a TableSelection,
    Placement<'a>,
    &'a mut ActiveDescendant,
    &'a mut ActiveColumn,
);

type Interactive = (With<Table>, Without<InteractionDisabled>);

pub(crate) fn table_key(
    mut input: On<FocusedInput<KeyboardInput>>,
    mut tables: Query<Navigable, Interactive>,
    rows: Rows,
    mut commands: Commands,
) {
    let table = input.focused_entity;
    let Ok((children, columns, selection, keys, area, mut active, mut column)) =
        tables.get_mut(table)
    else {
        return;
    };
    let Some(action) = bound_action(keys, &input.input) else {
        return;
    };
    input.propagate(false);

    if action == TableAction::Select {
        if input.input.repeat {
            return;
        }
        let value = position(*selection, *active, *column);
        commands.trigger(ValueChange {
            source: table,
            value,
            is_final: true,
        });
        return;
    }
    if action.moves_column() && selection.tracks_column() {
        let count = column_count(columns, children, &rows);
        column.set_if_neq(ActiveColumn(moved_column(action, column.0, count)));
        return;
    }
    if let Some(line) = move_row(action, (children, &rows, *area), &mut active) {
        commands.trigger(ScrollIntoView {
            entity: table,
            target: Rect::new(0, line, 1, 1),
        });
    }
}

fn move_row(
    action: TableAction,
    (children, rows, area): (&Children, &Rows, ComputedWidgetArea),
    active: &mut Mut<ActiveDescendant>,
) -> Option<u16> {
    let body: Vec<Entity> = body_rows(children, rows).collect();
    let last = body.len().checked_sub(1)?;
    let current = active
        .0
        .and_then(|row| body.iter().position(|&candidate| candidate == row));
    let (header, footer) = bands(children, rows);
    // The floor covers an area its bands leave nothing of, and a hidden
    // table, which keeps focus but has no area.
    let page = usize::from(body_height(area, (header, footer))).max(1);
    let index = moved_row(action, current, last, page);
    active.set_if_neq(ActiveDescendant(Some(body[index])));
    Some(
        u16::try_from(index)
            .unwrap_or(u16::MAX)
            .saturating_add(u16::from(header)),
    )
}

pub(crate) fn table_press(
    event: On<PointerPress>,
    mut tables: Query<Pressable, Interactive>,
    rows: Rows,
    mut commands: Commands,
) {
    let table = event.entity;
    let Ok((children, columns, selection, placement, mut active, mut column)) =
        tables.get_mut(table)
    else {
        return;
    };
    let (area, layout, cursor, scroll, offset) = placement;
    let placed = Placed {
        area,
        layout,
        scroll,
        offset,
    };
    let widths = resolved_widths(columns, children, &rows, placed.width());
    let gutter = cursor_gutter(*selection, active.0, cursor);
    let hit = clicked_column(
        event.position.x.saturating_sub(area.0.x),
        &placed.columns(widths, gutter),
    );
    let line = placed.line(event.position.y);
    let (header, footer) = bands(children, &rows);

    if header && line == 0 {
        if let Some(column) = hit {
            commands.trigger(TableHeaderClick {
                entity: table,
                column,
            });
        }
        return;
    }
    let Some(row) = clicked_row(line, header, placed.band((header, footer)))
        .and_then(|index| body_rows(children, &rows).nth(index))
    else {
        return;
    };
    active.set_if_neq(ActiveDescendant(Some(row)));
    if selection.tracks_column()
        && let Some(hit) = hit
    {
        column.set_if_neq(ActiveColumn(Some(hit)));
    }
    let value = position(*selection, *active, *column);
    commands.trigger(ValueChange {
        source: table,
        value,
        is_final: true,
    });
}

fn position(
    selection: TableSelection,
    active: ActiveDescendant,
    column: ActiveColumn,
) -> TablePosition {
    TablePosition {
        row: selection.tracks_row().then_some(active.0).flatten(),
        column: selection.tracks_column().then_some(column.0).flatten(),
    }
}

fn bound_action(keys: &TableKeys, input: &KeyboardInput) -> Option<TableAction> {
    if input.state != ButtonState::Pressed {
        return None;
    }
    keys.0
        .iter()
        .find(|(key, _)| *key == input.logical_key)
        .map(|(_, action)| *action)
}

fn moved_row(action: TableAction, current: Option<usize>, last: usize, page: usize) -> usize {
    match (action, current) {
        (TableAction::RowPrev, Some(index)) => index.saturating_sub(1),
        (TableAction::RowNext, Some(index)) => (index + 1).min(last),
        (TableAction::PageUp, Some(index)) => index.saturating_sub(page),
        (TableAction::PageDown, Some(index)) => index.saturating_add(page).min(last),
        (TableAction::RowLast, _) => last,
        _ => 0,
    }
}

fn moved_column(action: TableAction, current: Option<usize>, count: usize) -> Option<usize> {
    let last = count.checked_sub(1)?;
    Some(match (action, current) {
        (TableAction::ColumnPrev, Some(index)) => index.saturating_sub(1),
        (TableAction::ColumnNext, Some(index)) => (index + 1).min(last),
        _ => 0,
    })
}
