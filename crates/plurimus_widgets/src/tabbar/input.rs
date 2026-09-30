//! Tab bar interaction: keys on the bar through its own table, clicks on
//! its items. Both end in one `ValueChange<Entity>` on the bar.

use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::{ChildOf, Children};
use bevy_ecs::prelude::{Commands, Has, On, Query, With, Without};
use bevy_ecs::system::SystemParam;
use bevy_input::keyboard::KeyboardInput;
use bevy_input_focus::FocusedInput;
use plurimus_term::bevy_compat::HeldModifiers;

use super::{TabBar, TabBarAction, TabBarKeys, TabItem};
use plurimus_ui::{Checked, Click, ComputedDisabled, ValueChange, first_bound};

#[derive(SystemParam)]
pub(crate) struct TabAccess<'w, 's> {
    bars: Query<
        'w,
        's,
        (&'static Children, &'static TabBarKeys),
        (With<TabBar>, Without<ComputedDisabled>),
    >,
    items: Query<'w, 's, Has<Checked>, (With<TabItem>, Without<ComputedDisabled>)>,
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
    let Ok((children, keys)) = tabs.bars.get(bar) else {
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
            // A repeat selects nothing, but is the same intent as its press.
            input.propagate(false);
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
    if let Some(item) = target.and_then(|index| tabs.live(children).nth(index)) {
        input.propagate(false);
        commands.trigger(ValueChange::new(bar, item, true));
    }
}

pub(crate) fn tab_item_click(click: On<Click>, tabs: TabAccess, mut commands: Commands) {
    let item = click.entity;
    if !tabs.items.contains(item) {
        return;
    }
    let Ok(bar) = tabs.parents.get(item).map(ChildOf::parent) else {
        return;
    };
    if tabs.bars.contains(bar) {
        commands.trigger(ValueChange::new(bar, item, true));
    }
}
