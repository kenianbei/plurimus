//! Table interaction: keys through the app's own map, clicks through the
//! column layout.
//!
//! Both handlers resolve a click's column by rebuilding the layout ratatui
//! computes privately. That is the one place this widget restates a
//! dependency's arithmetic, and the boundary tests are what catch ratatui
//! changing it.

use bevy_ecs::change_detection::{DetectChangesMut, Mut};
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::Children;
use bevy_ecs::prelude::{Commands, Has, On, Query, With, Without};
use bevy_input::ButtonState;
use bevy_input::keyboard::KeyboardInput;
use bevy_input_focus::FocusedInput;
use plurimus_core::ratatui_core::layout::{Constraint, Layout, Rect};

use super::{
    ActiveColumn, Table, TableAction, TableColumns, TableCursor, TableFooter, TableHeader,
    TableHeaderClick, TableKeys, TableLayout, TablePosition, TableRow, TableSelection,
    cursor_gutter,
};
use crate::listbox::ActiveDescendant;
use plurimus_ui::{ComputedWidgetArea, InteractionDisabled, PointerPress, ValueChange};

type Rows<'w, 's> = Query<'w, 's, (&'static TableRow, Has<TableHeader>, Has<TableFooter>)>;

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
    &'a ComputedWidgetArea,
    Option<&'a TableLayout>,
    Option<&'a TableCursor>,
    &'a mut ActiveDescendant,
    &'a mut ActiveColumn,
);

type Interactive = (With<Table>, Without<InteractionDisabled>);

// The column geometry a click is resolved against, which is ratatui's own
// layout call plus the gutter it splits off first.
struct Columns {
    widths: Vec<Constraint>,
    layout: TableLayout,
    gutter: u16,
    width: u16,
}

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
    move_row(action, (children, &rows, *area), &mut active);
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
    let index = moved_row(action, current, last, page_rows(children, rows, area));
    active.set_if_neq(ActiveDescendant(Some(body[index])));
}

pub(crate) fn table_press(
    event: On<PointerPress>,
    mut tables: Query<Pressable, Interactive>,
    rows: Rows,
    mut commands: Commands,
) {
    let table = event.entity;
    let Ok((children, columns, selection, area, layout, cursor, mut active, mut column)) =
        tables.get_mut(table)
    else {
        return;
    };
    let geometry = Columns {
        widths: resolved_widths(columns, children, &rows, area.0.width),
        layout: layout.copied().unwrap_or_default(),
        gutter: cursor_gutter(Some(*selection), active.0, cursor),
        width: area.0.width,
    };
    let hit = clicked_column(event.position.x.saturating_sub(area.0.x), &geometry);
    let y = event.position.y.saturating_sub(area.0.y);
    let (header, footer) = bands(children, &rows);

    if header && y == 0 {
        if let Some(column) = hit {
            commands.trigger(TableHeaderClick {
                entity: table,
                column,
            });
        }
        return;
    }
    let Some(row) = clicked_row(y, (header, footer), *area, |index| {
        body_rows(children, &rows).nth(index)
    }) else {
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

// The footer sits at the bottom of the area, so a click on it lands at a
// `y` a long body would also reach. Only the band between the two is a row.
fn clicked_row(
    y: u16,
    (header, footer): (bool, bool),
    area: ComputedWidgetArea,
    row_at: impl FnOnce(usize) -> Option<Entity>,
) -> Option<Entity> {
    let body = area
        .0
        .height
        .saturating_sub(u16::from(header))
        .saturating_sub(u16::from(footer));
    let index = y.saturating_sub(u16::from(header));
    if index >= body {
        return None;
    }
    row_at(usize::from(index))
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

fn body_rows<'a>(children: &'a Children, rows: &'a Rows) -> impl Iterator<Item = Entity> + 'a {
    children
        .iter()
        .copied()
        .filter(|&child| matches!(rows.get(child), Ok((_, false, false))))
}

// Whether the table has a header row and a footer row.
fn bands(children: &Children, rows: &Rows) -> (bool, bool) {
    children
        .iter()
        .filter_map(|&child| rows.get(child).ok())
        .fold(
            (false, false),
            |(seen_header, seen_footer), (_, header, footer)| {
                (seen_header || header, seen_footer || footer)
            },
        )
}

fn page_rows(children: &Children, rows: &Rows, area: ComputedWidgetArea) -> usize {
    let (header, footer) = bands(children, rows);
    usize::from(area.0.height)
        .saturating_sub(usize::from(header) + usize::from(footer))
        .max(1)
}

// Ratatui reads an empty width set as "never called" and divides the area
// equally among as many columns as the widest row has.
fn resolved_widths(
    columns: &TableColumns,
    children: &Children,
    rows: &Rows,
    width: u16,
) -> Vec<Constraint> {
    if !columns.0.is_empty() {
        return columns.0.clone();
    }
    let count = column_count(columns, children, rows);
    let each = width / u16::try_from(count.max(1)).unwrap_or(u16::MAX);
    vec![Constraint::Length(each); count]
}

fn column_count(columns: &TableColumns, children: &Children, rows: &Rows) -> usize {
    if !columns.0.is_empty() {
        return columns.0.len();
    }
    children
        .iter()
        .filter_map(|&child| rows.get(child).ok())
        .map(|(cells, ..)| cells.0.len())
        .max()
        .unwrap_or_default()
}

fn clicked_column(x: u16, geometry: &Columns) -> Option<usize> {
    let [_, columns] =
        Layout::horizontal([Constraint::Length(geometry.gutter), Constraint::Fill(0)])
            .areas(Rect::new(0, 0, geometry.width, 1));
    Layout::horizontal(geometry.widths.iter().copied())
        .flex(geometry.layout.flex)
        .spacing(geometry.layout.column_spacing)
        .split(columns)
        .iter()
        .position(|rect| x >= rect.x && x < rect.x.saturating_add(rect.width))
}
