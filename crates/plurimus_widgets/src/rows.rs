//! Generic machinery for a container drawn from row children.
//!
//! Two widgets are built this way - the list box and the table - and both
//! need the same two things a container cannot work out for itself: how tall
//! its content is, and whether a row changed. Rows are children, and a
//! child's change never marks its parent, so no query filter on the
//! container can see one.
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

use crate::stylist::UiStyle;
use plurimus_ui::{Checked, ComputedWidgetArea, ScrollArea};

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

// A row clearing one of these reaches its container by no other route:
// `Changed` never fires for a component that goes, and the row itself keeps
// no record that it had one.
#[derive(SystemParam)]
pub(crate) struct ClearedRows<'w, 's> {
    checked: RemovedComponents<'w, 's, Checked>,
    styled: RemovedComponents<'w, 's, UiStyle>,
    parents: Query<'w, 's, &'static ChildOf>,
}

impl ClearedRows<'_, '_> {
    // A despawned row resolves to nothing, which is right: its container
    // hears about it through `Changed<Children>` instead.
    fn parents(&mut self) -> impl Iterator<Item = Entity> + '_ {
        let Self {
            checked,
            styled,
            parents,
        } = self;
        checked
            .read()
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

/// Keeps a scrollable container's content size at (content width, row
/// count), so the generic scroll machinery windows it correctly.
///
/// A scroll area windows a widget whole, so the content is as tall as every
/// row the container draws - which is why a scrolled table's header band
/// scrolls with its body.
pub(crate) fn sync_row_scroll<Container: Component, Row: Component>(
    mut containers: Query<
        (&ComputedWidgetArea, &Children, &mut ScrollArea),
        (
            With<Container>,
            Or<(
                Changed<Children>,
                Changed<ComputedWidgetArea>,
                Changed<ScrollArea>,
            )>,
        ),
    >,
    rows: Query<(), With<Row>>,
) {
    for (area, children, mut scroll) in &mut containers {
        let lines = children.iter().filter(|&&row| rows.contains(row)).count();
        let content = Size::new(
            scroll.content_width(area.0.width),
            u16::try_from(lines).unwrap_or(u16::MAX),
        );
        if scroll.content_size != content {
            scroll.content_size = content;
        }
    }
}
