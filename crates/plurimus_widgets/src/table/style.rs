//! The table stylist: row entities into a ratatui `Table`.
//!
//! Two change signals meet here, and keeping them apart is what stops an idle
//! frame doing any work. [`StylistCache`] answers "does it draw differently" -
//! hover, focus, disabled, the table's own style override - as it does for
//! every widget, and [`ContentDirty`](crate::rows::ContentDirty) answers
//! "does it draw something else".

use bevy_ecs::change_detection::{DetectChanges, Ref};
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::Children;
use bevy_ecs::prelude::{Changed, Has, Or, Query, Res};
use bevy_input_focus::InputFocus;
use plurimus_core::ratatui_core::layout::Constraint;
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::Line;
use ratatui_widgets::table::{Cell, HighlightSpacing, Row, Table as RatatuiTable, TableState};

use super::geometry::{resolved_widths, solve_width};
use super::{
    ActiveColumn, Table, TableCheckedStyle, TableColumns, TableCursor, TableFooter, TableHeader,
    TableLayout, TableRow, TableSelection, TableStripe,
};
use crate::rows::ActiveDescendant;
use crate::rows::ContentDirty;
use crate::rows::{cursor_style, cursor_symbol};
use plurimus_core::UiWidget;
use plurimus_ui::{Checked, ComputedWidgetArea, ScrollArea, UiStyle, UiTheme};
use plurimus_ui::{StateQuery, Stylable, StylistCache, hashed_bits, observed};

pub(crate) type TableRowsChanged = Or<(
    Changed<TableRow>,
    Changed<UiStyle>,
    Changed<Checked>,
    Changed<TableHeader>,
    Changed<TableFooter>,
)>;

// The cursor is absent by design: it reaches the stylist through
// `StylistCache`, which needs no ordering against whatever moved it.
pub(crate) type TableSelfChanged = Or<(
    Changed<Children>,
    Changed<TableColumns>,
    Changed<TableStripe>,
    Changed<TableLayout>,
    Changed<TableCheckedStyle>,
    Changed<TableCursor>,
    Changed<TableSelection>,
)>;

type TableRows<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static TableRow,
        Has<TableHeader>,
        Has<TableFooter>,
        Option<&'static UiStyle>,
        Has<Checked>,
    ),
>;

// How a table's rows are painted, before any interaction state.
type Look<'a> = (
    Option<&'a TableStripe>,
    Option<&'a TableCheckedStyle>,
    Option<&'a TableLayout>,
    Option<&'a TableCursor>,
);

// Where the cursor is, absent on a presentational table.
type Cursor<'a> = (
    Option<&'a TableSelection>,
    Option<&'a ActiveDescendant>,
    Option<&'a ActiveColumn>,
);

// The width the columns are solved against, which is the click path's too.
type Extent<'a> = (&'a ComputedWidgetArea, Option<&'a ScrollArea>);

type Tables<'w, 's> = Query<
    'w,
    's,
    (
        StateQuery<'static>,
        &'static Children,
        &'static TableColumns,
        Look<'static>,
        Cursor<'static>,
        Extent<'static>,
        Ref<'static, ContentDirty<Table>>,
        &'static mut StylistCache,
        &'static mut UiWidget,
    ),
    Stylable<Table>,
>;

#[derive(Default)]
struct Bands {
    header: Option<Row<'static>>,
    footer: Option<Row<'static>>,
    body: Vec<Row<'static>>,
    cursor: Option<usize>,
    // Counted here rather than in a second pass, and over every band, which
    // is the rule an empty `TableColumns` is divided by.
    widest: usize,
}

// The per-row styles a table resolves before its rows' own overrides.
struct RowStyles {
    stripe: Option<Style>,
    checked: Option<Style>,
}

struct Chrome {
    widths: Vec<Constraint>,
    layout: TableLayout,
    base: Style,
}

struct Highlight {
    selection: Option<TableSelection>,
    style: Style,
    symbol: Line<'static>,
    column: Option<usize>,
}

pub(crate) fn style_tables(
    theme: Res<UiTheme>,
    focus: Res<InputFocus>,
    mut tables: Tables,
    rows: TableRows,
) {
    for (state, children, columns, look, cursor, extent, content, mut cache, mut widget) in
        &mut tables
    {
        let (stripe, checked, layout, symbol) = look;
        let (selection, active, column) = cursor;
        let (area, scroll) = extent;
        // `Copy` scalars, compared here so neither the cursor nor a resize
        // needs system ordering against whatever moved it.
        let width = solve_width(*area, scroll);
        let next = observed(
            state,
            &focus,
            hashed_bits((
                active.map(|active| active.0),
                column.map(|column| column.0),
                width,
            )),
        );
        if !cache.redraws(next, theme.is_changed() || content.is_changed()) {
            continue;
        }
        let styles = RowStyles {
            stripe: stripe.map(|stripe| stripe.0),
            checked: checked.map(|checked| checked.0),
        };
        let bands = bands(children, &rows, &styles, active.and_then(|active| active.0));
        let chrome = Chrome {
            widths: resolved_widths(columns, || bands.widest, width),
            layout: layout.copied().unwrap_or_default(),
            base: next.resting_style(&theme),
        };
        let highlight = Highlight {
            selection: selection.copied(),
            style: cursor_style(
                next,
                active.is_some_and(|active| active.0.is_some()),
                &theme,
            ),
            symbol: cursor_symbol(symbol.map(|cursor| &cursor.0)),
            column: column.and_then(|column| column.0),
        };
        *widget = table_widget(bands, chrome, highlight);
    }
}

fn bands(
    children: &Children,
    rows: &TableRows,
    styles: &RowStyles,
    active: Option<Entity>,
) -> Bands {
    let mut bands = Bands::default();
    for &child in children {
        let Ok((row, cells, is_header, is_footer, over, checked)) = rows.get(child) else {
            continue;
        };
        bands.widest = bands.widest.max(cells.0.len());
        let over = over.map(|style| style.0);
        if is_header {
            bands.header = Some(row_widget(cells, over.unwrap_or_default()));
        } else if is_footer {
            bands.footer = Some(row_widget(cells, over.unwrap_or_default()));
        } else {
            if active == Some(row) {
                bands.cursor = Some(bands.body.len());
            }
            let banded = styles.stripe.filter(|_| bands.body.len() % 2 == 1);
            let checked = styles.checked.filter(|_| checked);
            // An unset `Style` patches as the identity, so an absent stripe
            // or override drops out of the chain rather than branching.
            let style = banded
                .unwrap_or_default()
                .patch(checked.unwrap_or_default())
                .patch(over.unwrap_or_default());
            bands.body.push(row_widget(cells, style));
        }
    }
    bands
}

fn row_widget(cells: &TableRow, style: Style) -> Row<'static> {
    Row::new(cells.0.iter().cloned().map(Cell::from)).style(style)
}

fn table_widget(bands: Bands, chrome: Chrome, highlight: Highlight) -> UiWidget {
    let mut table = RatatuiTable::new(bands.body, chrome.widths)
        .style(chrome.base)
        .column_spacing(chrome.layout.column_spacing)
        .flex(chrome.layout.flex)
        .highlight_symbol(highlight.symbol)
        .highlight_spacing(HighlightSpacing::WhenSelected);
    table = match highlight.selection {
        Some(TableSelection::Row) => table.row_highlight_style(highlight.style),
        Some(TableSelection::Column) => table.column_highlight_style(highlight.style),
        Some(TableSelection::Cell) => table.cell_highlight_style(highlight.style),
        None => table,
    };
    if let Some(header) = bands.header {
        table = table.header(header);
    }
    if let Some(footer) = bands.footer {
        table = table.footer(footer);
    }
    UiWidget::stateful(
        table,
        cursor_state(highlight.selection, bands.cursor, highlight.column),
    )
}

// Only the coordinates the mode tracks reach the state: a stray row index
// would reserve the cursor gutter for a cursor that is never drawn.
fn cursor_state(
    selection: Option<TableSelection>,
    row: Option<usize>,
    column: Option<usize>,
) -> TableState {
    let tracked = |axis: fn(TableSelection) -> bool, value: Option<usize>| {
        selection.is_some_and(axis).then_some(value).flatten()
    };
    TableState::new()
        .with_selected(tracked(TableSelection::tracks_row, row))
        .with_selected_column(tracked(TableSelection::tracks_column, column))
}
