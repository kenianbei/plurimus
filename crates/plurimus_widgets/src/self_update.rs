//! Stock observers for uncontrolled widgets, mirroring `bevy_ui_widgets`.
//!
//! Widgets emit [`ValueChange`] and change nothing themselves, which leaves
//! an app free to validate, reject or transform an edit before it lands. That
//! costs a handler for every widget an app does not care to control, so these
//! observers supply the obvious behaviour - apply the value - and an app
//! attaches them per widget. Controlled and uncontrolled widgets can sit side
//! by side in one tree.

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Children, Commands, Has, On, Query, With};
use bevy_ecs::query::QueryData;

use super::{
    ListBox, ListBoxMultiSelect, ListItem, RadioButton, SliderRange, SliderValue, TabBar, TabItem,
    Table, TableMultiSelect, TablePosition, TableRow, ValueChange,
};
use plurimus_ui::Checked;

/// Applies checkbox toggles to the [`Checked`] marker.
pub fn checkbox_self_update(change: On<ValueChange<bool>>, mut commands: Commands) {
    set_checked(change.source, change.value, &mut commands);
}

/// Moves [`Checked`] to the selected radio among the group's children.
pub fn radio_self_update(
    change: On<ValueChange<Entity>>,
    children: Query<&Children>,
    radios: Query<(), With<RadioButton>>,
    mut commands: Commands,
) {
    let Ok(group_children) = children.get(change.source) else {
        return;
    };
    move_checked_among(group_children, &radios, change.value, &mut commands);
}

/// Moves [`Checked`] to the activated tab among a [`TabBar`]'s items.
pub fn tab_bar_self_update(
    change: On<ValueChange<Entity>>,
    bars: Query<&Children, With<TabBar>>,
    items: Query<(), With<TabItem>>,
    mut commands: Commands,
) {
    let Ok(children) = bars.get(change.source) else {
        return;
    };
    move_checked_among(children, &items, change.value, &mut commands);
}

/// Applies list selection to [`Checked`]: single-select moves it among
/// the items, multi-select toggles the selected item.
pub fn listbox_self_update(
    change: On<ValueChange<Entity>>,
    boxes: Query<(&Children, Has<ListBoxMultiSelect>), With<ListBox>>,
    items: Query<Has<Checked>, With<ListItem>>,
    mut commands: Commands,
) {
    let Ok((children, multi_select)) = boxes.get(change.source) else {
        return;
    };
    if multi_select {
        if let Ok(checked) = items.get(change.value) {
            set_checked(change.value, !checked, &mut commands);
        }
        return;
    }
    move_checked_among(children, &items, change.value, &mut commands);
}

/// Applies table selection to [`Checked`]: single-select moves it among the
/// rows, multi-select toggles the selected one. A selection that tracks only
/// a column carries no row, and changes nothing.
pub fn table_self_update(
    change: On<ValueChange<TablePosition>>,
    tables: Query<(&Children, Has<TableMultiSelect>), With<Table>>,
    rows: Query<Has<Checked>, With<TableRow>>,
    mut commands: Commands,
) {
    let Ok((children, multi_select)) = tables.get(change.source) else {
        return;
    };
    let Some(selected) = change.value.row else {
        return;
    };
    if multi_select {
        if let Ok(checked) = rows.get(selected) {
            set_checked(selected, !checked, &mut commands);
        }
        return;
    }
    move_checked_among(children, &rows, selected, &mut commands);
}

fn set_checked(entity: Entity, checked: bool, commands: &mut Commands) {
    if checked {
        commands.entity(entity).insert(Checked);
    } else {
        commands.entity(entity).remove::<Checked>();
    }
}

fn move_checked_among<M: Component, D: QueryData>(
    children: &Children,
    members: &Query<D, With<M>>,
    selected: Entity,
    commands: &mut Commands,
) {
    for &child in children {
        if members.contains(child) {
            set_checked(child, child == selected, commands);
        }
    }
}

/// Applies slider changes to [`SliderValue`], clamped to the range.
pub fn slider_self_update(
    change: On<ValueChange<f32>>,
    mut sliders: Query<(&mut SliderValue, &SliderRange)>,
) {
    if let Ok((mut value, range)) = sliders.get_mut(change.source) {
        value.0 = change.value.clamp(range.start(), range.end());
    }
}
