//! Tab bar interaction: keys on the bar through its own table, clicks on
//! its items. Both end in one `ValueChange<Entity>` on the bar.

use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::{ChildOf, Children};
use bevy_ecs::prelude::{Commands, Has, On, Query, ResMut, With, Without};
use bevy_ecs::system::SystemParam;
use bevy_input::keyboard::KeyboardInput;
use bevy_input_focus::{FocusCause, FocusedInput, InputFocus};
use plurimus_term::bevy_compat::HeldModifiers;

use super::{TabBar, TabBarAction, TabBarKeys, TabItem};
use plurimus_ui::{
    Checked, Click, InteractionDisabled, PressFocusDisabled, ValueChange, first_bound,
};

#[derive(SystemParam)]
pub(crate) struct TabAccess<'w, 's> {
    bars: Query<
        'w,
        's,
        (
            &'static Children,
            &'static TabBarKeys,
            Has<PressFocusDisabled>,
        ),
        (With<TabBar>, Without<InteractionDisabled>),
    >,
    items: Query<'w, 's, Has<Checked>, (With<TabItem>, Without<InteractionDisabled>)>,
    parents: Query<'w, 's, &'static ChildOf>,
}

impl TabAccess<'_, '_> {
    fn live<'a>(&'a self, children: &'a Children) -> impl Iterator<Item = Entity> + 'a {
        children
            .iter()
            .copied()
            .filter(|&child| self.items.contains(child))
    }
}

pub(crate) fn tab_bar_key(
    mut input: On<FocusedInput<KeyboardInput>>,
    held: HeldModifiers,
    tabs: TabAccess,
    mut commands: Commands,
) {
    let bar = input.focused_entity;
    let Ok((children, keys, _)) = tabs.bars.get(bar) else {
        return;
    };
    let Some(action) = first_bound(&keys.0, &input.input, held.get()) else {
        return;
    };
    let count = tabs.live(children).count();
    let current = tabs
        .live(children)
        .position(|item| tabs.items.get(item).is_ok_and(|checked| checked));
    let target = match action {
        TabBarAction::Select(index) => {
            if index >= count {
                return;
            }
            (!input.input.repeat).then_some(index)
        }
        TabBarAction::Previous => current.and_then(|index| index.checked_sub(1)),
        TabBarAction::Next => match current {
            None => (count > 0).then_some(0),
            Some(index) => (index + 1 < count).then_some(index + 1),
        },
        TabBarAction::First => (count > 0).then_some(0).filter(|_| current != Some(0)),
        TabBarAction::Last => count.checked_sub(1).filter(|&last| current != Some(last)),
    };
    input.propagate(false);
    if let Some(item) = target.and_then(|index| tabs.live(children).nth(index)) {
        commands.trigger(ValueChange::new(bar, item, true));
    }
}

// A press focuses only the entity it lands on, and an item carries no
// `TabIndex`, so the click hands focus to the bar itself.
pub(crate) fn tab_item_click(
    click: On<Click>,
    tabs: TabAccess,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    let item = click.entity;
    if !tabs.items.contains(item) {
        return;
    }
    let Ok(bar) = tabs.parents.get(item).map(ChildOf::parent) else {
        return;
    };
    let Ok((_, _, focus_disabled)) = tabs.bars.get(bar) else {
        return;
    };
    commands.trigger(ValueChange::new(bar, item, true));
    if !focus_disabled {
        focus.set(bar, FocusCause::Pressed);
    }
}
