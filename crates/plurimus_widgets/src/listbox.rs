//! The list box widget: a focusable list container with entity items.
//!
//! Rows are entities carrying [`ListItem`], not strings, so an app builds a
//! list the same way it builds anything else and each row can hold its own
//! components. The list box owns selection and keyboard movement, its
//! bindings data in [`ListBoxKeys`] rather than a closed match; making it
//! scrollable is a separate decision, and adding a
//! [`ScrollArea`](plurimus_ui::ScrollArea) is all it takes for the generic
//! scroll machinery to window the rows.

use bevy_ecs::bundle::Bundle;
use bevy_ecs::change_detection::{DetectChangesMut, Mut};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Children, Commands, Component, On, Query, With, Without};
use bevy_input::ButtonState;
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::FocusedInput;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::text::Line;

use super::{UiLabel, ValueChange, placeholder};
use crate::rows::ContentDirty;
use crate::stylist::StylistCache;
use plurimus_ui::{ComputedWidgetArea, Hovered, InteractionDisabled, PointerPress};
use plurimus_ui::{ScrollIntoView, ScrollOffset};

/// A focusable list of [`ListItem`] children. Selection emits
/// [`ValueChange<Entity>`] on the list box; attach
/// [`listbox_self_update`](super::listbox_self_update) for uncontrolled
/// behavior.
#[derive(Component, Debug, Clone, Copy)]
#[require(
    Hovered,
    StylistCache,
    ActiveDescendant,
    ContentDirty<Self>,
    ListBoxKeys,
    ComputedWidgetArea
)]
pub struct ListBox;

/// Allows multiple [`Checked`](plurimus_ui::Checked) items in a [`ListBox`].
#[derive(Component, Debug, Clone, Copy)]
pub struct ListBoxMultiSelect;

/// Draws a [`ListBox`]'s marker column, telling
/// [`Checked`](plurimus_ui::Checked) rows apart from the row under the
/// cursor. Costs two cells of width, so it is asked for rather than
/// assumed.
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

/// One row of a [`ListBox`]: a child entity carrying its
/// [`UiLabel`] and [`Checked`](plurimus_ui::Checked) selection state.
#[derive(Component, Debug, Clone, Copy)]
pub struct ListItem;

/// The highlighted item. Keyboard focus stays on the [`ListBox`] itself;
/// this tracks which row its keys act on.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ActiveDescendant(pub Option<Entity>);

/// Spawn bundle for a list box; parent [`list_item`]s to it.
#[must_use]
pub fn listbox() -> impl Bundle {
    (ListBox, TabIndex(0), placeholder())
}

/// Spawn bundle for one list row.
pub fn list_item(label: impl Into<Line<'static>>) -> impl Bundle {
    (ListItem, UiLabel(label.into()))
}

/// What a key does to a [`ListBox`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ListBoxAction {
    /// Move the cursor up one row.
    Up,
    /// Move the cursor down one row.
    Down,
    /// Move the cursor to the first row.
    First,
    /// Move the cursor to the last row.
    Last,
    /// Move the cursor up by the visible height.
    PageUp,
    /// Move the cursor down by the visible height.
    PageDown,
    /// Select the row the cursor is on.
    Select,
}

/// A [`ListBox`]'s key bindings, scanned in order so the first match wins.
///
/// Replace it to remap: two keys may share an action by appearing twice.
/// Defaults to the arrows, `Home` and `End`, `PageUp` and `PageDown`, and
/// `Enter` and space to select.
#[derive(Component, Debug, Clone)]
pub struct ListBoxKeys(pub Vec<(Key, ListBoxAction)>);

impl Default for ListBoxKeys {
    fn default() -> Self {
        Self(vec![
            (Key::ArrowUp, ListBoxAction::Up),
            (Key::ArrowDown, ListBoxAction::Down),
            (Key::Home, ListBoxAction::First),
            (Key::End, ListBoxAction::Last),
            (Key::PageUp, ListBoxAction::PageUp),
            (Key::PageDown, ListBoxAction::PageDown),
            (Key::Enter, ListBoxAction::Select),
            (Key::Character(" ".into()), ListBoxAction::Select),
        ])
    }
}

pub(crate) fn listbox_key(
    mut input: On<FocusedInput<KeyboardInput>>,
    mut boxes: Query<
        (
            &Children,
            &ListBoxKeys,
            &ComputedWidgetArea,
            &mut ActiveDescendant,
        ),
        (With<ListBox>, Without<InteractionDisabled>),
    >,
    items: Query<(), With<ListItem>>,
    mut commands: Commands,
) {
    let listbox = input.focused_entity;
    let Ok((children, keys, area, mut active)) = boxes.get_mut(listbox) else {
        return;
    };
    let Some(action) = bound_action(keys, &input.input) else {
        return;
    };
    input.propagate(false);
    if action == ListBoxAction::Select {
        if !input.input.repeat && list_rows(children, &items).next().is_some() {
            select_active(listbox, *active, &mut commands);
        }
        return;
    }
    let rows: Vec<Entity> = list_rows(children, &items).collect();
    if let Some(index) = move_active(action, &rows, *area, &mut active) {
        reveal_row(listbox, index, &mut commands);
    }
}

// Focus dispatch is unordered against `UiSystems::Areas`, so the area is a
// frame stale and zero on the first, which pages by a single row.
fn move_active(
    action: ListBoxAction,
    rows: &[Entity],
    area: ComputedWidgetArea,
    active: &mut Mut<ActiveDescendant>,
) -> Option<usize> {
    let last = rows.len().checked_sub(1)?;
    let current = active
        .0
        .and_then(|item| rows.iter().position(|&row| row == item));
    let page = usize::from(area.0.height).max(1);
    let index = moved_index(action, current, last, page);
    active.set_if_neq(ActiveDescendant(Some(rows[index])));
    Some(index)
}

fn reveal_row(listbox: Entity, index: usize, commands: &mut Commands) {
    let row = u16::try_from(index).unwrap_or(u16::MAX);
    commands.trigger(ScrollIntoView {
        entity: listbox,
        target: Rect::new(0, row, 1, 1),
    });
}

fn bound_action(keys: &ListBoxKeys, input: &KeyboardInput) -> Option<ListBoxAction> {
    if input.state != ButtonState::Pressed {
        return None;
    }
    keys.0
        .iter()
        .find(|(key, _)| *key == input.logical_key)
        .map(|(_, action)| *action)
}

fn moved_index(action: ListBoxAction, current: Option<usize>, last: usize, page: usize) -> usize {
    match (action, current) {
        (ListBoxAction::Up, Some(index)) => index.saturating_sub(1),
        (ListBoxAction::Down, Some(index)) => (index + 1).min(last),
        (ListBoxAction::PageUp, Some(index)) => index.saturating_sub(page),
        (ListBoxAction::PageDown, Some(index)) => index.saturating_add(page).min(last),
        (ListBoxAction::Last, _) => last,
        _ => 0,
    }
}

fn select_active(listbox: Entity, active: ActiveDescendant, commands: &mut Commands) {
    if let Some(item) = active.0 {
        commands.trigger(ValueChange {
            source: listbox,
            value: item,
            is_final: true,
        });
    }
}

pub(crate) fn listbox_press(
    event: On<PointerPress>,
    mut boxes: Query<
        (
            &ComputedWidgetArea,
            Option<&ScrollOffset>,
            &Children,
            &mut ActiveDescendant,
        ),
        With<ListBox>,
    >,
    items: Query<(), With<ListItem>>,
    mut commands: Commands,
) {
    let listbox = event.entity;
    let Ok((area, offset, children, mut active)) = boxes.get_mut(listbox) else {
        return;
    };
    let scrolled = offset.map_or(0, |offset| offset.0.y);
    let row = event
        .position
        .y
        .saturating_sub(area.0.y)
        .saturating_add(scrolled);
    let Some(item) = row_entity(children, &items, usize::from(row)) else {
        return;
    };
    active.set_if_neq(ActiveDescendant(Some(item)));
    select_active(listbox, *active, &mut commands);
}

fn list_rows<'a>(
    children: &'a Children,
    items: &'a Query<(), With<ListItem>>,
) -> impl Iterator<Item = Entity> + 'a {
    children
        .iter()
        .copied()
        .filter(|child| items.contains(*child))
}

fn row_entity(
    children: &Children,
    items: &Query<(), With<ListItem>>,
    row: usize,
) -> Option<Entity> {
    list_rows(children, items).nth(row)
}
