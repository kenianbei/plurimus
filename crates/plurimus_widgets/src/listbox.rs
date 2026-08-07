//! The list box widget: a focusable list container with entity items.
//!
//! Rows are entities carrying [`ListItem`], not strings, so an app builds a
//! list the same way it builds anything else and each row can hold its own
//! components. The list box owns selection and keyboard movement; making it
//! scrollable is a separate decision, and adding a
//! [`ScrollArea`](plurimus_ui::ScrollArea) is all it takes for the generic
//! scroll machinery to window the rows.

use bevy_ecs::bundle::Bundle;
use bevy_ecs::change_detection::{DetectChanges, DetectChangesMut};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{
    Changed, Children, Commands, Component, Has, On, Or, Query, Res, With, Without,
};
use bevy_input::ButtonState;
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::tab_navigation::TabIndex;
use bevy_input_focus::{FocusedInput, InputFocus};
use plurimus_core::ratatui_core::layout::{Rect, Size};
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::Line;
use ratatui_widgets::list::{List, ListItem as ListRow, ListState};

use super::{UiLabel, ValueChange, is_activate_key, placeholder};
use crate::stylist::{
    StateQuery, StylistCache, StylistDisabled, UiStyle, decorate, hashed_bits, observed,
};
use crate::theme::UiTheme;
use plurimus_core::UiWidget;
use plurimus_ui::{Checked, ComputedWidgetArea, Hovered, InteractionDisabled, PointerPress};
use plurimus_ui::{ScrollArea, ScrollIntoView, ScrollOffset};

/// A focusable list of [`ListItem`] children. Selection emits
/// [`ValueChange<Entity>`] on the list box; attach
/// [`listbox_self_update`](super::listbox_self_update) for uncontrolled
/// behavior.
#[derive(Component, Debug, Clone, Copy)]
#[require(Hovered, StylistCache, ActiveDescendant)]
pub struct ListBox;

/// Allows multiple [`Checked`](plurimus_ui::Checked) items in a [`ListBox`].
#[derive(Component, Debug, Clone, Copy)]
pub struct ListBoxMultiSelect;

/// Draws a [`ListBox`]'s marker column, telling
/// [`Checked`](plurimus_ui::Checked) rows apart from the row under the
/// cursor. Costs two cells of width, so it is asked for rather than
/// assumed.
#[derive(Component, Debug, Clone, Copy)]
pub struct ListBoxSelectionMarker;

/// One row of a [`ListBox`]: a child entity carrying its
/// [`UiLabel`] and [`Checked`](plurimus_ui::Checked) selection state.
#[derive(Component, Debug, Clone, Copy)]
pub struct ListItem;

/// The highlighted item. Keyboard focus stays on the [`ListBox`] itself;
/// this tracks which row its keys act on.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ActiveDescendant(pub Option<Entity>);

/// Spawn bundle for a list box; parent [`list_item`]s to it.
pub fn listbox() -> impl Bundle {
    (ListBox, TabIndex(0), placeholder())
}

/// Spawn bundle for one list row.
pub fn list_item(label: impl Into<Line<'static>>) -> impl Bundle {
    (ListItem, UiLabel(label.into()))
}

enum ListKey {
    Up,
    Down,
    First,
    Last,
    Select,
}

pub(crate) fn listbox_key(
    mut input: On<FocusedInput<KeyboardInput>>,
    mut boxes: Query<
        (&Children, &mut ActiveDescendant),
        (With<ListBox>, Without<InteractionDisabled>),
    >,
    items: Query<(), With<ListItem>>,
    mut commands: Commands,
) {
    let listbox = input.focused_entity;
    let Ok((children, mut active)) = boxes.get_mut(listbox) else {
        return;
    };
    let Some(action) = list_key(&input.input) else {
        return;
    };
    input.propagate(false);
    let rows: Vec<Entity> = list_rows(children, &items).collect();
    if rows.is_empty() {
        return;
    }
    match action {
        ListKey::Select => select_active(listbox, &active, &mut commands),
        movement => {
            let current = active
                .0
                .and_then(|item| rows.iter().position(|&row| row == item));
            let index = moved_index(&movement, current, rows.len() - 1);
            active.set_if_neq(ActiveDescendant(Some(rows[index])));
            reveal_row(listbox, index, &mut commands);
        }
    }
}

fn reveal_row(listbox: Entity, index: usize, commands: &mut Commands) {
    let row = u16::try_from(index).unwrap_or(u16::MAX);
    commands.trigger(ScrollIntoView {
        entity: listbox,
        target: Rect::new(0, row, 1, 1),
    });
}

fn list_key(input: &KeyboardInput) -> Option<ListKey> {
    if input.state != ButtonState::Pressed {
        return None;
    }
    match &input.logical_key {
        Key::ArrowUp => Some(ListKey::Up),
        Key::ArrowDown => Some(ListKey::Down),
        Key::Home => Some(ListKey::First),
        Key::End => Some(ListKey::Last),
        _ if is_activate_key(input) => Some(ListKey::Select),
        _ => None,
    }
}

fn moved_index(movement: &ListKey, current: Option<usize>, last: usize) -> usize {
    match (movement, current) {
        (ListKey::Up, Some(index)) => index.saturating_sub(1),
        (ListKey::Down, Some(index)) => (index + 1).min(last),
        (ListKey::Last, _) => last,
        _ => 0,
    }
}

fn select_active(listbox: Entity, active: &ActiveDescendant, commands: &mut Commands) {
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
    select_active(listbox, &active, &mut commands);
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

pub(crate) fn style_listboxes(
    theme: Res<UiTheme>,
    focus: Res<InputFocus>,
    mut boxes: Query<
        (
            StateQuery,
            &ActiveDescendant,
            &Children,
            Has<ListBoxSelectionMarker>,
            &mut StylistCache,
            &mut UiWidget,
        ),
        (With<ListBox>, Without<StylistDisabled>),
    >,
    items: Query<(&UiLabel, Has<Checked>, Option<&UiStyle>), With<ListItem>>,
) {
    for (state, active, children, marker, mut cache, mut widget) in &mut boxes {
        let rows: Vec<Row> = children
            .iter()
            .filter_map(|&child| {
                let (label, checked, over) = items.get(child).ok()?;
                Some((child, &label.0, checked, over.map(|style| style.0)))
            })
            .collect();
        let selected = active
            .0
            .and_then(|item| rows.iter().position(|(row, ..)| *row == item));

        let next = observed(state, &focus, hashed_bits((&rows, selected, marker)));
        if !theme.is_changed() && next == *cache {
            continue;
        }
        *cache = next;
        *widget = list_widget(&rows, selected, marker, next.style(&theme));
    }
}

// A row's entity, label, checked state, and per-row style override.
type Row<'a> = (Entity, &'a Line<'static>, bool, Option<Style>);

fn list_widget(rows: &[Row], selected: Option<usize>, marker: bool, style: Style) -> UiWidget {
    let items: Vec<ListRow> = rows
        .iter()
        .map(|(_, label, checked, over)| {
            let line = if marker {
                decorate(if *checked { "▪ " } else { "  " }, label, "")
            } else {
                (*label).clone()
            };
            let row = ListRow::new(line);
            // Applied over the whole row rather than the label's own cells,
            // which is what reaches the cursor gutter.
            match over {
                Some(over) => row.style(*over),
                None => row,
            }
        })
        .collect();
    let mut highlight = ListState::default();
    highlight.select(selected);
    UiWidget::stateful(
        List::new(items).style(style).highlight_symbol("> "),
        highlight,
    )
}

// Keeps a scrollable ListBox's content size at (content width, item
// count) so the generic scroll machinery windows it correctly.
pub(crate) fn sync_listbox_scroll(
    mut boxes: Query<
        (&ComputedWidgetArea, &Children, &mut ScrollArea),
        (
            With<ListBox>,
            Or<(
                Changed<Children>,
                Changed<ComputedWidgetArea>,
                Changed<ScrollArea>,
            )>,
        ),
    >,
    items: Query<(), With<ListItem>>,
) {
    for (area, children, mut scroll) in &mut boxes {
        let rows = list_rows(children, &items).count();
        let content = Size::new(
            scroll.content_width(area.0.width),
            u16::try_from(rows).unwrap_or(u16::MAX),
        );
        if scroll.content_size != content {
            scroll.content_size = content;
        }
    }
}
