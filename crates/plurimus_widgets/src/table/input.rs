//! Table interaction: keys through the app's own map, clicks through the
//! column layout that [`geometry`](super::geometry) resolves.

use bevy_ecs::change_detection::{DetectChangesMut, Mut};
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::Children;
use bevy_ecs::prelude::{Changed, Commands, On, Query, With, Without};
use bevy_input::keyboard::KeyboardInput;
use bevy_input_focus::FocusedInput;
use plurimus_core::ratatui_core::layout::{Position, Rect};

use super::geometry::{
    Placed, Placement, Rows, bands, body_height, body_rows, clicked_column, clicked_row,
    column_count, resolved_widths, widest_row,
};
use super::{
    ActiveColumn, Table, TableAction, TableColumns, TableHeaderClick, TableKeys, TablePosition,
    TableSelection, cursor_gutter,
};
use crate::rows::ActiveDescendant;
use plurimus_ui::{
    Click, ComputedWidgetArea, InteractionDisabled, ScrollIntoView, ValueChange, first_bound,
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

type Clickable<'a> = (
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
    let Some(action) = first_bound(&keys.0, &input.input) else {
        return;
    };
    input.propagate(false);

    if action == TableAction::Select {
        if input.input.repeat {
            return;
        }
        let value = position(*selection, *active, *column);
        commands.trigger(ValueChange::new(table, value, true));
        return;
    }
    if action.moves_column() && selection.tracks_column() {
        let count = column_count(columns, widest_row(children, &rows));
        column.set_if_neq(ActiveColumn(moved_column(action, column.0, count)));
        return;
    }
    move_row(action, (children, &rows, *area), &mut active);
}

/// Scrolls whichever row [`ActiveDescendant`] names into view, whoever set
/// it - the table's own keys, a click, a repair after a rebuild, or an app
/// stepping the cursor itself.
pub(crate) fn reveal_table_cursor(
    tables: Query<(Entity, &Children, &ActiveDescendant), (With<Table>, Changed<ActiveDescendant>)>,
    rows: Rows,
    mut commands: Commands,
) {
    for (table, children, active) in &tables {
        let Some(row) = active.0 else {
            continue;
        };
        let Some(index) = body_rows(children, &rows).position(|candidate| candidate == row) else {
            continue;
        };
        let (header, _) = bands(children, &rows);
        let line = u16::try_from(index)
            .unwrap_or(u16::MAX)
            .saturating_add(u16::from(header));
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
) {
    let body: Vec<Entity> = body_rows(children, rows).collect();
    let Some(last) = body.len().checked_sub(1) else {
        return;
    };
    let current = active
        .0
        .and_then(|row| body.iter().position(|&candidate| candidate == row));
    let (header, footer) = bands(children, rows);
    // The floor covers an area its bands leave nothing of, and a hidden
    // table, which keeps focus but has no area.
    let page = usize::from(body_height(area, (header, footer))).max(1);
    let index = moved_row(action, current, last, page);
    active.set_if_neq(ActiveDescendant(Some(body[index])));
}

/// What a pointer cell landed on, once the column layout has been solved.
enum Hit {
    /// The header band, naming the column under the pointer.
    Header(usize),
    /// A body row, with the column under the pointer.
    Body(Entity, Option<usize>),
}

/// Everything the column solve needs, in one place because seven loose
/// parameters would breach the crate's limit - not because two callers
/// share it. Only the release resolves a cell; see [`table_click`].
struct Geometry<'a> {
    children: &'a Children,
    columns: &'a TableColumns,
    selection: TableSelection,
    placement: Placement<'a>,
    active: Option<Entity>,
}

impl Geometry<'_> {
    fn hit(&self, cell: Position, rows: &Rows) -> Option<Hit> {
        let (area, layout, cursor, scroll, offset) = self.placement;
        let placed = Placed {
            area,
            layout,
            scroll,
            offset,
        };
        let cell = placed.content_cell(cell)?;
        let widths = resolved_widths(
            self.columns,
            widest_row(self.children, rows),
            placed.width(),
        );
        let gutter = cursor_gutter(self.selection, self.active, cursor);
        let column = clicked_column(cell.x, &placed.columns(widths, gutter));
        let (header, footer) = bands(self.children, rows);
        if header && cell.y == 0 {
            return Some(Hit::Header(column?));
        }
        let row = clicked_row(cell.y, header, placed.band((header, footer)))
            .and_then(|index| body_rows(self.children, rows).nth(index))?;
        Some(Hit::Body(row, column))
    }
}

/// Moves the cursor to the row released on, selects it, and reports a
/// header click.
///
/// The release edge rather than the press: selecting usually closes what was
/// clicked, and closing on the way down despawns the entity the pointer
/// router is still holding a gesture for.
///
/// A press moves nothing, which the column geometry requires rather than
/// merely permits: the cursor gutter exists only while a row is current, so
/// a press that set the cursor would shift every column before the release
/// resolved against them - against a layout no frame had drawn yet.
pub(crate) fn table_click(
    event: On<Click>,
    mut tables: Query<Clickable, Interactive>,
    rows: Rows,
    mut commands: Commands,
) {
    let table = event.entity;
    let Ok((children, columns, selection, placement, mut active, mut column)) =
        tables.get_mut(table)
    else {
        return;
    };
    let geometry = Geometry {
        children,
        columns,
        selection: *selection,
        placement,
        active: active.0,
    };
    let Some(hit) = geometry.hit(event.position, &rows) else {
        return;
    };
    let (row, hit_column) = match hit {
        Hit::Header(header_column) => {
            commands.trigger(TableHeaderClick {
                entity: table,
                column: header_column,
            });
            return;
        }
        Hit::Body(row, hit_column) => (row, hit_column),
    };
    active.set_if_neq(ActiveDescendant(Some(row)));
    if selection.tracks_column()
        && let Some(hit_column) = hit_column
    {
        column.set_if_neq(ActiveColumn(Some(hit_column)));
    }
    let value = position(*selection, *active, *column);
    commands.trigger(ValueChange::new(table, value, true));
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
