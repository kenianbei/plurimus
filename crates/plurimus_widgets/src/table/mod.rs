//! The table widget: columnar rows with a header and footer band.
//!
//! Rows are entities carrying [`TableRow`], and their cells are data on that
//! entity rather than entities of their own - selection, striping, and click
//! routing are all row-level, and a [`Line`] carries its own style and
//! alignment, so a cell needs no identity to be styled.
//!
//! A bare [`Table`] draws and hovers; it is not a tab stop and consumes no
//! keys. Interaction is opt-in, and making it scrollable is a separate
//! decision again - adding a [`ScrollArea`](plurimus_ui::ScrollArea) is all
//! the generic scroll machinery needs. Because a scroll area windows a widget
//! whole, the header scrolls with the body; a table that must keep its header
//! in view is two tables, an unscrolled header above the scrolled rows,
//! sharing one [`TableColumns`].
//!
//! For cells holding arbitrary content rather than text, a `bevy_ui`
//! `Display::Grid` tree laid out by `plurimus_bui` is the other tool: it
//! sizes cells with taffy, at the cost of an entity and a layout node each,
//! and knows nothing of headers, striping, or a cursor.

mod style;

pub(crate) use style::{mark_changed_tables, style_tables};

use bevy_ecs::bundle::Bundle;
use bevy_ecs::prelude::Component;
use plurimus_core::ratatui_core::layout::{Constraint, Flex};
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::Line;

use crate::placeholder;
use crate::stylist::StylistCache;
use plurimus_ui::Hovered;

/// A table of [`TableRow`] children, drawn as one ratatui `Table`.
///
/// Presentational on its own. Rows are one terminal row tall.
#[derive(Component, Debug, Clone, Copy)]
#[require(Hovered, StylistCache, TableContent)]
pub struct Table;

/// A [`Table`]'s column widths, resolved by ratatui's layout.
///
/// An empty set is ratatui's signal to divide the width equally among as
/// many columns as the widest row has. Not a required component, so that a
/// [`Table`] is styled only once the app has said how wide its columns are;
/// one without it keeps whatever [`UiWidget`](plurimus_core::UiWidget) it
/// carries.
#[derive(Component, Debug, Clone)]
pub struct TableColumns(pub Vec<Constraint>);

/// One row of a [`Table`]: a child entity holding its cells.
#[derive(Component, Debug, Clone)]
pub struct TableRow(pub Vec<Line<'static>>);

/// Draws a [`TableRow`] as the header band, above the body and outside the
/// striping. Applied when the row is spawned; a marker removed from a live
/// row does not repaint until something else about the table changes.
#[derive(Component, Debug, Clone, Copy)]
pub struct TableHeader;

/// Draws a [`TableRow`] as the footer band, below the body and outside the
/// striping. Carries the same caveat as [`TableHeader`].
#[derive(Component, Debug, Clone, Copy)]
pub struct TableFooter;

/// Bands a [`Table`]'s body, patched over every second row counting from
/// the second, so the first body row is unstriped.
///
/// Counted in child order rather than from the scroll offset, so stripes do
/// not crawl as a table scrolls. A row's own
/// [`UiStyle`](crate::UiStyle) patches on top, keeping the banding beneath
/// a color the app chose.
#[derive(Component, Debug, Clone, Copy)]
pub struct TableStripe(pub Style);

/// A [`Table`]'s column spacing and extra-space distribution. Defaults to
/// ratatui's: one cell between columns, spare width left at the end.
#[derive(Component, Debug, Clone, Copy)]
pub struct TableLayout {
    /// Cells between one column and the next.
    pub column_spacing: u16,
    /// Where width the columns did not claim ends up.
    pub flex: Flex,
}

impl Default for TableLayout {
    fn default() -> Self {
        Self {
            column_spacing: 1,
            flex: Flex::Start,
        }
    }
}

// Carries one tick: what a table draws has changed. Rows are children, so a
// `Changed` filter on the table cannot see a row's edit; `mark_changed_tables`
// forwards it here.
#[derive(Component, Debug, Clone, Copy, Default)]
pub(crate) struct TableContent;

/// Spawn bundle for a table; parent [`table_row`]s to it.
#[must_use]
pub fn table(widths: impl IntoIterator<Item = Constraint>) -> impl Bundle {
    (
        Table,
        TableColumns(widths.into_iter().collect()),
        placeholder(),
    )
}

/// Spawn bundle for one body row.
#[must_use]
pub fn table_row(cells: impl IntoIterator<Item = impl Into<Line<'static>>>) -> impl Bundle {
    TableRow(cells.into_iter().map(Into::into).collect())
}

/// Spawn bundle for the header row.
#[must_use]
pub fn table_header(cells: impl IntoIterator<Item = impl Into<Line<'static>>>) -> impl Bundle {
    (TableHeader, table_row(cells))
}

/// Spawn bundle for the footer row.
#[must_use]
pub fn table_footer(cells: impl IntoIterator<Item = impl Into<Line<'static>>>) -> impl Bundle {
    (TableFooter, table_row(cells))
}
