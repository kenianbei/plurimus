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
//! A cell is therefore not a widget and cannot hold one. Editing one in
//! place means floating a widget over where it is drawn: ask
//! [`TableGeometry::cell_rect`] for the cell's screen rect, turn it into a
//! [`UiArea`](plurimus_core::UiArea) with
//! [`local_area`](plurimus_core::local_area), and give the floated widget a
//! higher [`UiOrder`](plurimus_core::UiOrder) than the table. The table
//! knows nothing about it; what it does is report where its cells are.
//!
//! For cells holding arbitrary content rather than text, a `bevy_ui`
//! `Display::Grid` tree laid out by `plurimus_bui` is the other tool: it
//! sizes cells with taffy, at the cost of an entity and a layout node each,
//! and knows nothing of headers, striping, or a cursor.

mod geometry;
mod input;
mod style;

pub use geometry::TableGeometry;

pub(crate) use geometry::BodyRow as TableBodyRow;
pub(crate) use input::{reveal_table_cursor, table_click, table_key};
pub(crate) use style::{TableRowsChanged, TableSelfChanged, style_tables};

use bevy_ecs::bundle::Bundle;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Component, EntityEvent};
use bevy_input::keyboard::Key;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::layout::{Constraint, Flex};
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::Line;

use crate::rows::ActiveDescendant;
use crate::rows::CURSOR_SYMBOL;
use crate::rows::ContentDirty;
use plurimus_core::UiWidget;
use plurimus_ui::Hovered;
use plurimus_ui::KeyBinding;
use plurimus_ui::StylistCache;

/// A table of [`TableRow`] children, drawn as one ratatui `Table`.
///
/// Presentational on its own. Rows are one terminal row tall.
#[derive(Component, Debug, Clone, Copy)]
#[require(Hovered, StylistCache, ContentDirty<Self>)]
pub struct Table;

/// A [`Table`]'s column widths, resolved by ratatui's layout.
///
/// An empty set divides the table's width equally among as many columns as
/// the widest of its rows has - a scrolled table's content width, being what
/// it is drawn into. This crate expands that default itself rather than
/// leaving it to ratatui, so the columns a click resolves against are the
/// ones that were drawn.
///
/// Not a required component, so that a [`Table`] is styled only once the app
/// has said how wide its columns are; one without it keeps whatever
/// [`UiWidget`](plurimus_core::UiWidget) it carries.
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
/// [`UiStyle`](plurimus_ui::UiStyle) patches on top, keeping the banding
/// beneath a color the app chose.
#[derive(Component, Debug, Clone, Copy)]
pub struct TableStripe(pub Style);

/// A [`Table`]'s column spacing and extra-space distribution. Defaults to
/// ratatui's: one cell between columns, spare width left at the end.
#[derive(Component, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct TableLayout {
    /// Cells between one column and the next.
    pub column_spacing: u16,
    /// Where width the columns did not claim ends up.
    pub flex: Flex,
}

impl TableLayout {
    /// Sets the blank columns drawn between cells.
    #[must_use]
    pub const fn with_column_spacing(mut self, column_spacing: u16) -> Self {
        self.column_spacing = column_spacing;
        self
    }
}

impl Default for TableLayout {
    fn default() -> Self {
        Self {
            column_spacing: 1,
            flex: Flex::Start,
        }
    }
}

/// Styles a [`Checked`](plurimus_ui::Checked) body row, which is where a
/// [`TableMultiSelect`] table's selection shows. Patched over the striping
/// and under the row's own [`UiStyle`](plurimus_ui::UiStyle).
#[derive(Component, Debug, Clone, Copy)]
pub struct TableCheckedStyle(pub Style);

/// Makes a [`Table`] interactive, at the stated granularity.
///
/// Adding it makes the table a tab stop and gives it keys and clicks;
/// without it a table draws and hovers but emits nothing. Selection emits
/// [`ValueChange<TablePosition>`](crate::ValueChange) on the table in every
/// mode.
///
/// Requires [`TabIndex`], which Bevy does not remove when this component
/// goes: removing it leaves a non-interactive table in the tab order.
/// [`InteractionDisabled`](plurimus_ui::InteractionDisabled) is how a table
/// is turned off.
///
/// Closed: row, column and cell are the whole granularity lattice, and an
/// app branching on it has to handle each to be correct.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[require(TabIndex, ActiveDescendant, ActiveColumn, TableKeys)]
pub enum TableSelection {
    /// The cursor is a row.
    #[default]
    Row,
    /// The cursor is a column.
    Column,
    /// The cursor is one cell.
    Cell,
}

impl TableSelection {
    pub(crate) const fn tracks_row(self) -> bool {
        matches!(self, Self::Row | Self::Cell)
    }

    pub(crate) const fn tracks_column(self) -> bool {
        matches!(self, Self::Column | Self::Cell)
    }
}

/// Allows multiple [`Checked`](plurimus_ui::Checked) rows in a [`Table`].
#[derive(Component, Debug, Clone, Copy)]
pub struct TableMultiSelect;

/// The cursor's column, as [`ActiveDescendant`] is its row.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ActiveColumn(pub Option<usize>);

/// A position in a [`Table`]. A coordinate the table's [`TableSelection`]
/// does not track is `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TablePosition {
    /// The row entity.
    pub row: Option<Entity>,
    /// The column index.
    pub column: Option<usize>,
}

/// Replaces the symbol drawn beside a [`Table`]'s cursor row, which shifts
/// every column right by its width while a row is selected. An empty line
/// frees the gutter, leaving the cursor shown by its highlight alone.
#[derive(Component, Debug, Clone)]
pub struct TableCursor(pub Line<'static>);

/// A click on a [`Table`]'s [`TableHeader`] row, carrying the column under
/// the pointer. Sorting is the app's: reorder the row entities and the
/// table redraws.
#[derive(EntityEvent, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct TableHeaderClick {
    /// The table that was clicked.
    pub entity: Entity,
    /// The column under the pointer.
    pub column: usize,
}

impl TableHeaderClick {
    /// A header click on `entity`'s `column`.
    #[must_use]
    pub const fn new(entity: Entity, column: usize) -> Self {
        Self { entity, column }
    }
}

/// What a key does to a [`Table`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TableAction {
    /// Move the cursor up one row.
    RowPrev,
    /// Move the cursor down one row.
    RowNext,
    /// Move the cursor to the first row.
    RowFirst,
    /// Move the cursor to the last row.
    RowLast,
    /// Move the cursor up by the visible body height.
    PageUp,
    /// Move the cursor down by the visible body height.
    PageDown,
    /// Move the cursor one column left.
    ColumnPrev,
    /// Move the cursor one column right.
    ColumnNext,
    /// Select what the cursor is on.
    Select,
}

impl TableAction {
    pub(crate) const fn moves_column(self) -> bool {
        matches!(self, Self::ColumnPrev | Self::ColumnNext)
    }
}

/// A [`Table`]'s key bindings, scanned in order so the first match wins.
///
/// Replace it to remap: two keys may share an action by appearing twice.
/// Defaults to the arrows, `Home` and `End`, `PageUp` and `PageDown`, and
/// `Enter` and space to select.
#[derive(Component, Debug, Clone)]
pub struct TableKeys(pub Vec<(KeyBinding, TableAction)>);

impl Default for TableKeys {
    fn default() -> Self {
        Self(vec![
            (Key::ArrowUp.into(), TableAction::RowPrev),
            (Key::ArrowDown.into(), TableAction::RowNext),
            (Key::ArrowLeft.into(), TableAction::ColumnPrev),
            (Key::ArrowRight.into(), TableAction::ColumnNext),
            (Key::Home.into(), TableAction::RowFirst),
            (Key::End.into(), TableAction::RowLast),
            (Key::PageUp.into(), TableAction::PageUp),
            (Key::PageDown.into(), TableAction::PageDown),
            (Key::Enter.into(), TableAction::Select),
            (Key::Character(" ".into()).into(), TableAction::Select),
        ])
    }
}

// The cells ratatui reserves left of the first column. It never reports the
// width it settled on, so this restates the rule `WhenSelected` spacing sets.
pub(crate) fn cursor_gutter(
    selection: TableSelection,
    active: Option<Entity>,
    cursor: Option<&TableCursor>,
) -> u16 {
    if !selection.tracks_row() || active.is_none() {
        return 0;
    }
    let width = cursor.map_or(CURSOR_SYMBOL.chars().count(), |cursor| cursor.0.width());
    u16::try_from(width).unwrap_or(u16::MAX)
}

/// Spawn bundle for a table; parent [`table_row`]s to it.
#[must_use]
pub fn table(widths: impl IntoIterator<Item = Constraint>) -> impl Bundle {
    (
        Table,
        TableColumns(widths.into_iter().collect()),
        UiWidget::default(),
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
