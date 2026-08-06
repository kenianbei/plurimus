//! The activation contract shared by every widget that responds to a click
//! or an Enter/Space press.
//!
//! [`Activate`] is not the button's private event: `menu.rs` triggers it for
//! menu items that have no [`Button`] component, and `menu_button()` composes
//! [`Button`] deliberately to inherit this path. Widgets with their own key
//! handling route around this file entirely; what arrives here is the generic
//! click-or-Enter/Space contract, so [`ActivationTargets`] is where a new
//! widget opts into it.

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, Commands, EntityEvent, Has, On, Query, With, Without};
use bevy_ecs::system::SystemParam;
use bevy_input::ButtonState;
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::FocusedInput;

use crate::button::Button;
use crate::checkbox::Checkbox;
use crate::radio::{RadioButton, RadioGroup};
use plurimus_ui::{Checked, Click, InteractionDisabled, ValueChange};

/// The widget was activated (button click, Enter/Space).
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct Activate {
    /// The activated widget.
    pub entity: Entity,
}

// Activation fires on key PRESS, unlike bevy's release semantics: legacy
// terminals have no timely release events.
pub(crate) fn is_activate_key(input: &KeyboardInput) -> bool {
    let key = &input.logical_key;
    let activates = *key == Key::Enter || matches!(key, Key::Character(c) if c == " ");
    input.state == ButtonState::Pressed && !input.repeat && activates
}

#[derive(SystemParam)]
pub(crate) struct ActivationTargets<'w, 's> {
    buttons: Query<'w, 's, (), (With<Button>, Without<InteractionDisabled>)>,
    checkboxes: Query<'w, 's, Has<Checked>, (With<Checkbox>, Without<InteractionDisabled>)>,
    radios: Query<'w, 's, (), (With<RadioButton>, Without<InteractionDisabled>)>,
    parents: Query<'w, 's, &'static ChildOf>,
    groups: Query<'w, 's, (), With<RadioGroup>>,
}

pub(crate) fn widget_click(click: On<Click>, targets: ActivationTargets, mut commands: Commands) {
    activate_widget(click.entity, &targets, &mut commands);
}

pub(crate) fn widget_key(
    mut input: On<FocusedInput<KeyboardInput>>,
    targets: ActivationTargets,
    mut commands: Commands,
) {
    if !is_activate_key(&input.input) {
        return;
    }
    if activate_widget(input.focused_entity, &targets, &mut commands) {
        input.propagate(false);
    }
}

fn activate_widget(entity: Entity, targets: &ActivationTargets, commands: &mut Commands) -> bool {
    if targets.buttons.contains(entity) {
        commands.trigger(Activate { entity });
        return true;
    }
    if let Ok(checked) = targets.checkboxes.get(entity) {
        commands.trigger(ValueChange {
            source: entity,
            value: !checked,
            is_final: true,
        });
        return true;
    }
    if targets.radios.contains(entity)
        && let Some(group) = radio_group_of(entity, targets)
    {
        commands.trigger(ValueChange {
            source: group,
            value: entity,
            is_final: true,
        });
        return true;
    }
    false
}

fn radio_group_of(entity: Entity, targets: &ActivationTargets) -> Option<Entity> {
    targets
        .parents
        .iter_ancestors(entity)
        .find(|&ancestor| targets.groups.contains(ancestor))
}
