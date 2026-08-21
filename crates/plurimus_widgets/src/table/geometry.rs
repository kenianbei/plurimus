//! Where a table's rows and columns landed, and how big its drawing is.
//!
//! Ratatui resolves a table's columns privately, so a click cannot ask it
//! where a cell is. [`clicked_column`] rebuilds that layout from the same
//! inputs - the gutter split off first, then the constraints with their
//! flex and spacing - which makes this file the one place the widget
//! restates a dependency's arithmetic, and the boundary tests the thing
//! that catches ratatui changing it.

use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::Children;
use bevy_ecs::prelude::{Has, Query, With, Without};
use plurimus_core::ratatui_core::layout::{Constraint, Layout, Position, Rect};

use super::{TableColumns, TableCursor, TableFooter, TableHeader, TableLayout, TableRow};
use plurimus_ui::{ComputedWidgetArea, ScrollArea, ScrollOffset, content_cell};

pub(super) type Rows<'w, 's> =
    Query<'w, 's, (&'static TableRow, Has<TableHeader>, Has<TableFooter>)>;

/// Everything about where a table's cells landed on screen.
pub(super) type Placement<'a> = (
    &'a ComputedWidgetArea,
    Option<&'a TableLayout>,
    Option<&'a TableCursor>,
    Option<&'a ScrollArea>,
    Option<&'a ScrollOffset>,
);

// The column geometry a click is resolved against: ratatui's own layout
// call, plus the gutter it splits off first.
pub(super) struct Columns {
    widths: Vec<Constraint>,
    layout: TableLayout,
    gutter: u16,
    width: u16,
}

pub(super) struct Placed<'a> {
    pub(super) area: &'a ComputedWidgetArea,
    pub(super) layout: Option<&'a TableLayout>,
    pub(super) scroll: Option<&'a ScrollArea>,
    pub(super) offset: Option<&'a ScrollOffset>,
}

impl Placed<'_> {
    pub(super) fn width(&self) -> u16 {
        solve_width(*self.area, self.scroll)
    }

    // Both axes: a scrolled table's columns are laid out against its
    // content width, not the area they show through.
    pub(super) fn content_cell(&self, at: Position) -> Option<Position> {
        let offset = self.offset.map_or(Position::ORIGIN, |offset| offset.0);
        content_cell(at, self.area.0, offset)
    }

    // Only an unscrolled table needs bounding; see `clicked_row`.
    pub(super) fn band(&self, bands: (bool, bool)) -> Option<u16> {
        self.scroll
            .is_none()
            .then(|| body_height(*self.area, bands))
    }

    pub(super) fn columns(&self, widths: Vec<Constraint>, gutter: u16) -> Columns {
        Columns {
            widths,
            layout: self.layout.copied().unwrap_or_default(),
            gutter,
            width: self.width(),
        }
    }
}

// `line` is content-space, so a scrolled table's offset is already in it.
// `band` bounds it only when unscrolled: ratatui pins that footer to the
// bottom of the area rather than to the end of the body, so a click on it
// lands at a line a long enough body would also reach.
pub(super) fn clicked_row(line: u16, header: bool, band: Option<u16>) -> Option<usize> {
    let index = line.saturating_sub(u16::from(header));
    band.is_none_or(|band| index < band)
        .then(|| usize::from(index))
}

pub(super) fn body_height(area: ComputedWidgetArea, (header, footer): (bool, bool)) -> u16 {
    area.0
        .height
        .saturating_sub(u16::from(header))
        .saturating_sub(u16::from(footer))
}

pub(super) fn clicked_column(x: u16, geometry: &Columns) -> Option<usize> {
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

// A scrolled table is drawn into a buffer as wide as its content, which is
// a scrollbar's width narrower than the area it shows through.
pub(super) fn solve_width(area: ComputedWidgetArea, scroll: Option<&ScrollArea>) -> u16 {
    scroll.map_or(area.0.width, |scroll| scroll.content_size.width)
}

// An empty width set is expanded here rather than by ratatui, which reads
// one as "never called" and applies this same rule to its own render area.
// The stylist and the click path both take the result, so a table's columns
// cannot be solved from two different widths. `widest` is ignored when the
// app stated its widths, and is what each caller's own pass over the rows
// already found.
pub(super) fn resolved_widths(
    columns: &TableColumns,
    widest: usize,
    width: u16,
) -> Vec<Constraint> {
    if !columns.0.is_empty() {
        return columns.0.clone();
    }
    let each = width / u16::try_from(widest.max(1)).unwrap_or(u16::MAX);
    vec![Constraint::Length(each); widest]
}

pub(super) const fn column_count(columns: &TableColumns, widest: usize) -> usize {
    if columns.0.is_empty() {
        widest
    } else {
        columns.0.len()
    }
}

pub(super) fn widest_row(children: &Children, rows: &Rows) -> usize {
    children
        .iter()
        .filter_map(|&child| rows.get(child).ok())
        .map(|(cells, ..)| cells.0.len())
        .max()
        .unwrap_or_default()
}

/// The rows a cursor may land on, as a query filter - the same predicate
/// [`body_rows`] applies, so a new band cannot be a row to one and not the
/// other.
pub(crate) type BodyRow = (With<TableRow>, Without<TableHeader>, Without<TableFooter>);

pub(super) fn body_rows<'a>(
    children: &'a Children,
    rows: &'a Rows,
) -> impl Iterator<Item = Entity> + 'a {
    children
        .iter()
        .copied()
        .filter(|&child| matches!(rows.get(child), Ok((_, false, false))))
}

// Whether the table has a header row and a footer row.
pub(super) fn bands(children: &Children, rows: &Rows) -> (bool, bool) {
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
