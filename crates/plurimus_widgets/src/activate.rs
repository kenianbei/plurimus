//! The activation contract shared by every widget that responds to a click
//! or a bound key press.
//!
//! [`Activate`] is not the button's private event: `menu.rs` triggers it for
//! menu items that have no [`Button`] component, and `menu_button()` composes
//! [`Button`] deliberately to inherit this path. Widgets with their own key
//! handling route around this file entirely; what arrives here is the generic
//! click-or-[`ActivateKeys`] contract, so [`ActivationTargets`] is where a new
//! widget opts into it.
//!
//! Activation fires on key press, unlike bevy's release semantics: legacy
//! terminals have no timely release events.

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, Commands, Component, EntityEvent, Has, On, Query, With, Without};
use bevy_ecs::system::SystemParam;
use bevy_input::ButtonState;
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::FocusedInput;
use plurimus_term::bevy_compat::HeldModifiers;

use crate::button::Button;
use crate::checkbox::Checkbox;
use crate::radio::{RadioButton, RadioGroup};
use plurimus_ui::{Checked, Click, InteractionDisabled, KeyBinding, ValueChange};

/// The widget was activated (a click, or a key in [`ActivateKeys`]).
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct Activate {
    /// The activated widget.
    pub entity: Entity,
}

/// The keys that activate a [`Button`], [`Checkbox`] or [`RadioButton`]
/// holding focus, defaulting to `Enter` and space.
///
/// Required by all three, so replacing it is the whole of remapping:
/// binding space alone is what lets a form keep `Enter` for its submit,
/// since a key that activates nothing is left to propagate. An empty list
/// therefore disables the keyboard path without disabling the widget, which
/// a click still activates.
///
/// A repeat never activates - one intent commits once - so a held key is
/// not the way to toggle a checkbox repeatedly.
#[derive(Component, Debug, Clone)]
pub struct ActivateKeys(pub Vec<KeyBinding>);

impl Default for ActivateKeys {
    fn default() -> Self {
        Self(vec![Key::Enter.into(), Key::Character(" ".into()).into()])
    }
}

fn is_fresh_press(input: &KeyboardInput) -> bool {
    input.state == ButtonState::Pressed && !input.repeat
}

#[derive(SystemParam)]
pub(crate) struct ActivationTargets<'w, 's> {
    buttons: Query<'w, 's, (), (With<Button>, Without<InteractionDisabled>)>,
    checkboxes: Query<'w, 's, Has<Checked>, (With<Checkbox>, Without<InteractionDisabled>)>,
    radios: Query<'w, 's, (), (With<RadioButton>, Without<InteractionDisabled>)>,
    keys: Query<'w, 's, &'static ActivateKeys>,
    held: HeldModifiers<'w>,
    parents: Query<'w, 's, &'static ChildOf>,
    groups: Query<'w, 's, (), With<RadioGroup>>,
}

impl ActivationTargets<'_, '_> {
    fn binds(&self, entity: Entity, input: &KeyboardInput) -> bool {
        is_fresh_press(input)
            && self.keys.get(entity).is_ok_and(|keys| {
                let held = self.held.get();
                keys.0.iter().any(|binding| binding.matches(input, held))
            })
    }
}

pub(crate) fn widget_click(click: On<Click>, targets: ActivationTargets, mut commands: Commands) {
    activate_widget(click.entity, &targets, &mut commands);
}

pub(crate) fn widget_key(
    mut input: On<FocusedInput<KeyboardInput>>,
    targets: ActivationTargets,
    mut commands: Commands,
) {
    let entity = input.focused_entity;
    if !targets.binds(entity, &input.input) {
        return;
    }
    if activate_widget(entity, &targets, &mut commands) {
        input.propagate(false);
    }
}

fn activate_widget(entity: Entity, targets: &ActivationTargets, commands: &mut Commands) -> bool {
    if targets.buttons.contains(entity) {
        commands.trigger(Activate { entity });
        return true;
    }
    if let Ok(checked) = targets.checkboxes.get(entity) {
        commands.trigger(ValueChange::new(entity, !checked, true));
        return true;
    }
    if targets.radios.contains(entity)
        && let Some(group) = radio_group_of(entity, targets)
    {
        commands.trigger(ValueChange::new(group, entity, true));
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
