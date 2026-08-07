//! The table stylist: row entities into a ratatui `Table`.
//!
//! Two change signals meet here, and keeping them apart is what stops an idle
//! frame doing any work. [`StylistCache`] answers "does it draw differently" -
//! hover, focus, disabled, the table's own style override - as it does for
//! every widget. [`TableContent`] answers "does it draw something else", which
//! a query filter on the table cannot: rows are children, and a child's change
//! never marks its parent.

use bevy_ecs::change_detection::{DetectChanges, DetectChangesMut, Ref};
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::{ChildOf, Children};
use bevy_ecs::prelude::{Changed, Has, Or, Query, Res, With};
use bevy_input_focus::InputFocus;
use plurimus_core::ratatui_core::style::Style;
use ratatui_widgets::table::{Cell, Row, Table as RatatuiTable, TableState};

use super::{
    Table, TableColumns, TableContent, TableFooter, TableHeader, TableLayout, TableRow, TableStripe,
};
use crate::stylist::{StateQuery, Stylable, StylistCache, UiStyle, observed};
use crate::theme::UiTheme;
use plurimus_core::UiWidget;

type RowChanged = Or<(
    Changed<TableRow>,
    Changed<UiStyle>,
    Changed<TableHeader>,
    Changed<TableFooter>,
)>;

type ContentChanged = Or<(
    Changed<Children>,
    Changed<TableColumns>,
    Changed<TableStripe>,
    Changed<TableLayout>,
)>;

type TableRows<'w, 's> = Query<
    'w,
    's,
    (
        &'static TableRow,
        Has<TableHeader>,
        Has<TableFooter>,
        Option<&'static UiStyle>,
    ),
>;

type Tables<'w, 's> = Query<
    'w,
    's,
    (
        StateQuery<'static>,
        &'static Children,
        &'static TableColumns,
        Option<&'static TableStripe>,
        Option<&'static TableLayout>,
        Ref<'static, TableContent>,
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
}

// A row's edit has to reach the table it belongs to, and so does a change to
// the table's own content components.
pub(crate) fn mark_changed_tables(
    rows: Query<&ChildOf, RowChanged>,
    changed: Query<Entity, (With<Table>, ContentChanged)>,
    mut content: Query<&mut TableContent>,
) {
    for row in &rows {
        touch(&mut content, row.parent());
    }
    for table in &changed {
        touch(&mut content, table);
    }
}

fn touch(content: &mut Query<&mut TableContent>, table: Entity) {
    if let Ok(mut marker) = content.get_mut(table) {
        marker.set_changed();
    }
}

pub(crate) fn style_tables(
    theme: Res<UiTheme>,
    focus: Res<InputFocus>,
    mut tables: Tables,
    rows: TableRows,
) {
    for (state, children, columns, stripe, layout, content, mut cache, mut widget) in &mut tables {
        let next = observed(state, &focus, 0);
        if !theme.is_changed() && !content.is_changed() && next == *cache {
            continue;
        }
        *cache = next;
        let bands = bands(children, &rows, stripe.map(|stripe| stripe.0));
        *widget = table_widget(
            bands,
            columns,
            layout.copied().unwrap_or_default(),
            next.resting_style(&theme),
        );
    }
}

fn bands(children: &Children, rows: &TableRows, stripe: Option<Style>) -> Bands {
    let mut bands = Bands::default();
    for &child in children {
        let Ok((cells, is_header, is_footer, over)) = rows.get(child) else {
            continue;
        };
        let over = over.map(|style| style.0);
        if is_header {
            bands.header = Some(row_widget(cells, patched(None, over)));
        } else if is_footer {
            bands.footer = Some(row_widget(cells, patched(None, over)));
        } else {
            let banded = stripe.filter(|_| bands.body.len() % 2 == 1);
            bands.body.push(row_widget(cells, patched(banded, over)));
        }
    }
    bands
}

// A `Style` with every field unset patches as the identity, in both
// directions, which is what lets an absent stripe and an absent override
// share one expression.
fn patched(stripe: Option<Style>, over: Option<Style>) -> Style {
    stripe.unwrap_or_default().patch(over.unwrap_or_default())
}

fn row_widget(cells: &TableRow, style: Style) -> Row<'static> {
    Row::new(cells.0.iter().cloned().map(Cell::from)).style(style)
}

fn table_widget(
    bands: Bands,
    columns: &TableColumns,
    layout: TableLayout,
    base: Style,
) -> UiWidget {
    let mut table = RatatuiTable::new(bands.body, columns.0.iter().copied())
        .style(base)
        .column_spacing(layout.column_spacing)
        .flex(layout.flex);
    if let Some(header) = bands.header {
        table = table.header(header);
    }
    if let Some(footer) = bands.footer {
        table = table.footer(footer);
    }
    UiWidget::stateful(table, TableState::default())
}
