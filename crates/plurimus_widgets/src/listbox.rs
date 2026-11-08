//! The list box on `bevy_ui_widgets`' own [`ListBox`] and [`ListItem`].
//!
//! Rows are entities carrying [`ListItem`], not strings, so an app builds a
//! list the same way it builds anything else and each row can hold its own
//! components. The keys, the cursor's focus rules and the selection updater
//! are upstream's; what is here is what a cell list needs beyond them - the
//! pointer, since a row has no area of its own to be picked by, and the
//! copy upstream does not have. Making it scrollable is a separate
//! decision, and adding a [`ScrollArea`](plurimus_ui::ScrollArea) is all it
//! takes for the generic scroll machinery to window the rows.

use bevy_ecs::bundle::Bundle;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{
    Changed, Children, Commands, Component, Has, On, Query, Res, With, Without,
};
use bevy_ecs::system::SystemParam;
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::FocusedInput;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::layout::{Position, Rect};
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::Line;
use plurimus_term::HeldModifiers;

use crate::rows::{
    ActiveDescendant, ContentDirty, ListItemText, copied_text, point_cursor, row_height,
};
use plurimus_core::CellSize;
use plurimus_core::UiWidget;
use plurimus_term::Clipboard;
use plurimus_ui::StylistCache;
use plurimus_ui::UiLabel;
use plurimus_ui::bevy_picking::events::{PointerClick, PointerDrag, PointerPress};
use plurimus_ui::bevy_ui_widgets::{ListBox, ListBoxMultiSelect, ListItem, SetSelected};
use plurimus_ui::{
    ComputedDisabled, ComputedWidgetArea, Hovered, KeyBinding, Selected, click_cell, first_bound,
    pointer_cell,
};
use plurimus_ui::{ScrollIntoView, ScrollOffset, content_cell};

/// The one key a list binds beyond upstream's, which has no copy: the
/// cursor row, or every [`Selected`] row of a [`ListBoxMultiSelect`] list.
fn copy_key() -> KeyBinding {
    KeyBinding::new(Key::Character("c".into())).with_ctrl()
}

/// Draws a [`ListBox`]'s marker column, telling [`Selected`] and
/// [`Marked`](crate::Marked) rows apart from the row under the cursor.
/// Costs two cells of width, so it is asked for rather than assumed.
///
/// Applied when the list is spawned; a marker removed from a live list
/// does not repaint until something else about the list changes.
#[derive(Component, Debug, Clone, Copy)]
pub struct ListBoxSelectionMarker;

/// Replaces the symbol drawn beside a [`ListBox`]'s cursor row, which
/// shifts row content right by its width. An empty line frees the gutter
/// entirely, leaving the cursor shown by its highlight style alone.
///
/// Carries the same caveat as [`ListBoxSelectionMarker`]: removing it
/// restores the default symbol only once something else repaints.
#[derive(Component, Debug, Clone)]
pub struct ListBoxCursor(pub Line<'static>);

/// Bands a [`ListBox`], patched over every second row counting from the
/// second, so the first row is unstriped.
///
/// Counted in child order rather than from the scroll offset, so stripes do
/// not crawl as a list scrolls, and a multi-line [`ListItemText`] row counts
/// once. A row's own [`UiStyle`](plurimus_ui::UiStyle) patches on top,
/// keeping the banding beneath a color the app chose, and the cursor's
/// highlight patches over both.
///
/// Carries the same caveat as [`ListBoxSelectionMarker`]: a stripe removed
/// from a live list stays drawn until something else repaints it.
#[derive(Component, Debug, Clone, Copy)]
pub struct ListBoxStripe(pub Style);

/// Spawn bundle for a list box; parent [`list_item`]s to it.
///
/// Upstream's [`ListBox`] requires none of what a cell list draws or is
/// picked with, so the bundle carries it.
#[must_use]
pub fn listbox() -> impl Bundle {
    (
        ListBox,
        Hovered::default(),
        StylistCache::default(),
        ContentDirty::<ListBox>::default(),
        ComputedWidgetArea::default(),
        TabIndex(0),
        UiWidget::default(),
    )
}

/// Spawn bundle for one list row.
pub fn list_item(label: impl Into<Line<'static>>) -> impl Bundle {
    (ListItem, UiLabel(label.into()))
}

/// Copies what [`copied_text`] picks on [`copy_key`], each row as it reads -
/// its [`ListItemText`], else its label. With nothing to copy the key goes
/// on to the list's ancestors.
pub(crate) fn listbox_copy(
    mut input: On<FocusedInput<KeyboardInput>>,
    held: HeldModifiers,
    boxes: Query<
        (&Children, &ActiveDescendant, Has<ListBoxMultiSelect>),
        (
            With<ListBox>,
            With<ComputedWidgetArea>,
            Without<ComputedDisabled>,
        ),
    >,
    rows: Query<(Option<&ListItemText>, Option<&UiLabel>, Has<Selected>), With<ListItem>>,
    mut clipboard: Clipboard,
) {
    let Ok((children, active, is_multi_select)) = boxes.get(input.focused_entity) else {
        return;
    };
    if first_bound(&[(copy_key(), ())], &input.input, held.get()).is_none() {
        return;
    }
    let listed = children
        .iter()
        .filter_map(|&child| Some((child, rows.get(child).ok()?.2)));
    let copied = copied_text(
        listed,
        is_multi_select,
        |child| active.0 == Some(child),
        |child| rows.get(child).map_or_else(|_| String::new(), row_text),
    );
    if clipboard.offer(&copied) {
        input.propagate(false);
    }
}

fn row_text((text, label, _): (Option<&ListItemText>, Option<&UiLabel>, bool)) -> String {
    text.map(|text| text.0.to_string())
        .or_else(|| label.map(|label| label.0.to_string()))
        .unwrap_or_default()
}

/// Scrolls whichever row [`ActiveDescendant`] names into view, whoever set
/// it - a key, a click, a repair, or an app driving the list from a search
/// field beside it. The reveal belongs to the cursor rather than to the
/// thing that moved it, so no writer has to remember it.
pub(crate) fn reveal_listbox_cursor(
    boxes: Query<
        (Entity, &Children, &ActiveDescendant),
        (
            With<ListBox>,
            With<ComputedWidgetArea>,
            Changed<ActiveDescendant>,
        ),
    >,
    items: RowTexts,
    mut commands: Commands,
) {
    for (listbox, children, active) in &boxes {
        let Some(item) = active.0 else {
            continue;
        };
        let Some(span) = row_spans(children, &items).find(|span| span.entity == item) else {
            continue;
        };
        commands.trigger(ScrollIntoView {
            entity: listbox,
            target: Rect::new(0, span.top, 1, span.height),
        });
    }
}

/// What the list's pointer routing reads: the list's geometry and cursor,
/// and its rows' heights and state.
#[derive(SystemParam)]
pub(crate) struct ListPointer<'w, 's> {
    lists: Query<
        'w,
        's,
        (
            &'static ComputedWidgetArea,
            Option<&'static ScrollOffset>,
            &'static Children,
            &'static ActiveDescendant,
        ),
        (With<ListBox>, Without<ComputedDisabled>),
    >,
    items: RowTexts<'w, 's>,
}

impl ListPointer<'_, '_> {
    /// The row under `position` in `list`, with the list's cursor.
    fn row_at(&self, list: Entity, position: Position) -> Option<(Entity, &ActiveDescendant)> {
        let (area, offset, children, active) = self.lists.get(list).ok()?;
        let cell = content_cell(position, area.0, offset)?;
        let row = row_spans(children, &self.items)
            .find(|span| span.contains(cell.y))?
            .entity;
        Some((row, active))
    }
}

/// Moves the cursor to the row under the pointer, so a press shows what it
/// is on. Selecting is the release's, in [`listbox_click`].
pub(crate) fn listbox_press(
    mut event: On<PointerPress>,
    cell: Res<CellSize>,
    lists: ListPointer,
    mut commands: Commands,
) {
    if !lists.lists.contains(event.entity) {
        return;
    }
    event.propagate(false);
    let position = pointer_cell(&event.pointer, *cell);
    if let Some((row, active)) = lists.row_at(event.entity, position) {
        point_cursor(event.entity, active, row, &mut commands);
    }
}

/// Keeps the cursor under a held pointer, so the highlight never disagrees
/// with the row a release would select.
pub(crate) fn listbox_drag(
    mut event: On<PointerDrag>,
    cell: Res<CellSize>,
    lists: ListPointer,
    mut commands: Commands,
) {
    if !lists.lists.contains(event.entity) {
        return;
    }
    event.propagate(false);
    let position = pointer_cell(&event.pointer, *cell);
    if let Some((row, active)) = lists.row_at(event.entity, position) {
        point_cursor(event.entity, active, row, &mut commands);
    }
}

/// Selects the row the pointer was released on through upstream's
/// [`SetSelected`], whose rule a key follows too: a disabled row moves the
/// cursor and selects nothing, the [`Selected`] row is reselected, any other
/// is a [`ValueChange`](crate::ValueChange).
///
/// The release edge rather than the press: a list selection usually closes
/// the thing it was made in, and closing on the way down despawns the entity
/// `bevy_picking` is still holding a gesture for. A release outside the list
/// or any row selects nothing, and a drag across rows names the one it ended
/// on. Upstream's own row click finds no row here, a cell row being no pick.
pub(crate) fn listbox_click(
    mut event: On<PointerClick>,
    cell: Res<CellSize>,
    lists: ListPointer,
    mut commands: Commands,
) {
    let list = event.entity;
    let Ok((area, ..)) = lists.lists.get(list) else {
        return;
    };
    event.propagate(false);
    let Some(position) = click_cell(&event, area, *cell) else {
        return;
    };
    let Some((row, active)) = lists.row_at(list, position) else {
        return;
    };
    point_cursor(list, active, row, &mut commands);
    commands.trigger(SetSelected { entity: list, row });
}

/// The rows a list's geometry is measured from: every [`ListItem`] child,
/// with the text that decides how tall it is.
pub(crate) type RowTexts<'w, 's> = Query<'w, 's, Option<&'static ListItemText>, With<ListItem>>;

/// Where one row sits in the list's own content space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RowSpan {
    pub(crate) entity: Entity,
    pub(crate) top: u16,
    pub(crate) height: u16,
}

impl RowSpan {
    const fn contains(self, line: u16) -> bool {
        line >= self.top && line < self.top.saturating_add(self.height)
    }
}

pub(crate) fn row_spans<'a>(
    children: &'a Children,
    items: &'a RowTexts,
) -> impl Iterator<Item = RowSpan> + 'a {
    let mut top = 0;
    children.iter().copied().filter_map(move |child| {
        let height = row_height(items.get(child).ok()?);
        let span = RowSpan {
            entity: child,
            top,
            height,
        };
        top = top.saturating_add(height);
        Some(span)
    })
}
