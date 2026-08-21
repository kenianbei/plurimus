//! Generic machinery for a container drawn from row children.
//!
//! Two widgets are built this way - the list box and the table - and both
//! need the same things a container cannot work out for itself: how tall its
//! content is, whether a row changed, and what to draw beside the row under
//! the cursor. Rows are children, and a child's change never marks its
//! parent, so no query filter on the container can see one.
//!
//! Both systems run in [`WidgetSystems::Layout`](crate::WidgetSystems),
//! before the stylists read what they leave behind.

use std::marker::PhantomData;

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::{ChildOf, Children};
use bevy_ecs::prelude::{Changed, Component, Or, Query, RemovedComponents, With};
use bevy_ecs::query::QueryFilter;
use bevy_ecs::system::SystemParam;
use plurimus_core::ratatui_core::layout::Size;
use plurimus_core::ratatui_core::text::{Line, Text};

use crate::listbox::ListItemTrailing;
use plurimus_ui::{Checked, ComputedWidgetArea, ScrollArea, UiStyle};

/// Marks a container whose content changed: a row added, edited, restyled,
/// or checked.
///
/// [`mark_dirty_content`] sets it, and the container's stylist reads it
/// beside its `StylistCache` rather than hashing every row to find out
/// whether it need redraw.
#[derive(Component)]
pub(crate) struct ContentDirty<M: Send + Sync + 'static>(PhantomData<M>);

impl<M: Send + Sync + 'static> Default for ContentDirty<M> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

/// The row a container's keys act on, whoever holds keyboard focus.
///
/// A container is driven either by holding focus itself or by another
/// widget writing this - a search field stepping the list beneath it - so
/// the cursor row is styled as the active one in both cases.
///
/// Kept pointing at a live row: when a container's children change, a value
/// naming a row that is gone re-points to the first surviving one, and to
/// `None` when none survives. A deliberately empty cursor stays empty.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ActiveDescendant(pub Option<Entity>);

/// Lights a row's marker gutter without claiming it is selected.
///
/// [`Checked`] is the selection channel, written by
/// [`listbox_self_update`](crate::listbox_self_update) and read as "the user
/// picked this". A row can also be marked for a reason of the app's own -
/// a command already in force, a file with unsaved edits - and saying that
/// with `Checked` means any app-wide selection updater silently redefines
/// it. Nothing in this crate ever writes this one.
///
/// Drawn by [`ListBoxSelectionMarker`](crate::ListBoxSelectionMarker),
/// which lights the gutter for a row that is checked, marked, or both.
#[derive(Component, Debug, Clone, Copy)]
pub struct Marked;

/// Draws a [`ListItem`](crate::ListItem) as more than one terminal row, in place of its
/// [`UiLabel`](plurimus_ui::UiLabel).
///
/// Explicit line breaks only: the list truncates a line rather than
/// wrapping it, so a row is exactly as tall as the [`Text`] has lines. An
/// empty one still takes a row. Only the list box reads this - every other
/// widget draws the single-line label, which stays the row's label for
/// anything that asks.
#[derive(Component, Debug, Clone)]
pub struct ListItemText(pub Text<'static>);

// A row is at least one line tall, so an empty `ListItemText` cannot make
// two rows share a top and swallow the clicks meant for one of them.
pub(crate) fn row_height(text: Option<&ListItemText>) -> u16 {
    text.map_or(1, |text| {
        u16::try_from(text.0.height()).unwrap_or(u16::MAX).max(1)
    })
}

// A row clearing one of these reaches its container by no other route:
// `Changed` never fires for a component that goes, and the row itself keeps
// no record that it had one.
#[derive(SystemParam)]
pub(crate) struct ClearedRows<'w, 's> {
    checked: RemovedComponents<'w, 's, Checked>,
    marked: RemovedComponents<'w, 's, Marked>,
    trailing: RemovedComponents<'w, 's, ListItemTrailing>,
    styled: RemovedComponents<'w, 's, UiStyle>,
    parents: Query<'w, 's, &'static ChildOf>,
}

impl ClearedRows<'_, '_> {
    // A despawned row resolves to nothing, which is right: its container
    // hears about it through `Changed<Children>` instead.
    fn parents(&mut self) -> impl Iterator<Item = Entity> + '_ {
        let Self {
            checked,
            marked,
            trailing,
            styled,
            parents,
        } = self;
        checked
            .read()
            .chain(marked.read())
            .chain(trailing.read())
            .chain(styled.read())
            .filter_map(|row| parents.get(row).ok())
            .map(ChildOf::parent)
    }
}

/// Forwards a row's change to the container that draws it.
///
/// `RowsChanged` says what counts as a row edit and `SelfChanged` what
/// counts as a change to the container itself; the markers `Container` and
/// `Row` are matched here, so neither filter carries its own `With`.
///
/// Independently of both, a row that *loses* [`Checked`] or [`UiStyle`]
/// marks its container too, which no `Changed` filter can report.
pub(crate) fn mark_dirty_content<Container, Row, RowsChanged, SelfChanged>(
    rows: Query<&ChildOf, (With<Row>, RowsChanged)>,
    changed: Query<Entity, (With<Container>, SelfChanged)>,
    mut cleared: ClearedRows,
    mut content: Query<&mut ContentDirty<Container>>,
) where
    Container: Component,
    Row: Component,
    RowsChanged: QueryFilter + 'static,
    SelfChanged: QueryFilter + 'static,
{
    let touched = rows
        .iter()
        .map(ChildOf::parent)
        .chain(changed.iter())
        .chain(cleared.parents());
    for container in touched {
        if let Ok(mut dirty) = content.get_mut(container) {
            dirty.set_changed();
        }
    }
}

/// Keeps a container's [`ActiveDescendant`] pointing at a live row.
///
/// A row it named can be despawned or reparented - filtering a list is
/// despawning its children and spawning new ones - and nothing about the
/// container says so afterwards, leaving a cursor that highlights nothing
/// and moves from nowhere. The first surviving row is where a container
/// with no cursor already sends its first key press, so it is where a lost
/// one lands.
///
/// A cursor an app deliberately emptied stays empty: only a value naming a
/// row that is gone is repaired.
pub(crate) fn repair_active_descendants<Container, Row>(
    mut containers: Query<(Option<&Children>, &mut ActiveDescendant), With<Container>>,
    refilled: Query<Entity, (With<Container>, Changed<Children>)>,
    // Losing the last child removes `Children` outright, so the container
    // an emptied list leaves behind matches no `Changed<Children>` filter.
    mut emptied: RemovedComponents<Children>,
    rows: Query<(), Row>,
) where
    Container: Component,
    Row: QueryFilter + 'static,
{
    for container in refilled.iter().chain(emptied.read()) {
        let Ok((children, mut active)) = containers.get_mut(container) else {
            continue;
        };
        let Some(current) = active.0 else {
            continue;
        };
        let surviving = |row| children.is_some_and(|kept: &Children| kept.contains(&row));
        if surviving(current) && rows.get(current).is_ok() {
            continue;
        }
        let first =
            children.and_then(|kept| kept.iter().copied().find(|&child| rows.get(child).is_ok()));
        active.set_if_neq(ActiveDescendant(first));
    }
}

/// Keeps a scrollable container's content size at (content width, summed
/// row heights), so the generic scroll machinery windows it correctly.
///
/// A scroll area windows a widget whole, so the content is as tall as every
/// row the container draws - which is why a scrolled table's header band
/// scrolls with its body.
///
/// Only a list box's rows can be taller than one line, through
/// [`ListItemText`], so the height query and the [`ContentDirty`] term of
/// the filter are both inert for a table: its rows read `None` and its
/// extent can only move when a row is added or removed. Carrying them here
/// is what keeps one sync for both containers.
pub(crate) fn sync_row_scroll<Container: Component, Row: Component>(
    mut containers: Query<
        (&ComputedWidgetArea, &Children, &mut ScrollArea),
        (
            With<Container>,
            Or<(
                Changed<Children>,
                Changed<ComputedWidgetArea>,
                Changed<ScrollArea>,
                Changed<ContentDirty<Container>>,
            )>,
        ),
    >,
    rows: Query<Option<&ListItemText>, With<Row>>,
) {
    for (area, children, mut scroll) in &mut containers {
        let lines: u16 = children
            .iter()
            .filter_map(|&child| rows.get(child).ok())
            .map(row_height)
            .fold(0, u16::saturating_add);
        let content = Size::new(scroll.content_width(area.0.width), lines);
        if scroll.content_size != content {
            scroll.content_size = content;
        }
    }
}

/// Shared, so the crate's cursor cannot differ between two containers.
pub(crate) const CURSOR_SYMBOL: &str = "> ";

/// The cursor a container draws, `over` replacing the default when a
/// widget carries one of its own.
pub(crate) fn cursor_symbol(over: Option<&Line<'static>>) -> Line<'static> {
    over.cloned().unwrap_or_else(|| Line::from(CURSOR_SYMBOL))
}
