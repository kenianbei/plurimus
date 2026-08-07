//! Forwards the plurimus input contract into `bevy_input`'s native keyboard
//! vocabulary, for consumers of the bevy input-focus stack.
//!
//! The physical `KeyCode` mapping is best-effort: terminals report logical
//! keys, so positions are inferred from a US layout. App logic should keep
//! using [`crate::KeyCode`]; this exists for interop.

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Local, MessageReader, MessageWriter, Res};
use bevy_input::ButtonState;
use bevy_input::keyboard::KeyboardInput;

use super::{InputCapabilities, KeyCode, KeyKind, KeyMessage, KeyModifiers, ModifierKey};

mod keymap;

use keymap::{logical_key, modifier_logical, modifier_physical, physical_code};

/// Forwards [`KeyMessage`]s as bevy `KeyboardInput` messages.
///
/// Modifier keys are handled capability-first. With
/// [`InputCapabilities::modifier_keys`] set, real [`KeyCode::Modifier`]
/// events carry every transition and convert like any other key. Without
/// it, presses/releases are synthesized from [`KeyModifiers`] transitions.
/// A terminal that answers the kitty probe but withholds
/// modifier-keys-as-keys therefore reports no modifier state here; the raw
/// [`KeyMessage::modifiers`] bitfield stays accurate.
///
/// Synthesized transitions always name the left key, since [`KeyModifiers`]
/// collapses the sides: a right-shift press arrives as `ShiftLeft`.
///
/// Register in [`crate::InputSystems::Update`] before
/// `bevy_input::InputSystems`.
pub fn forward_keyboard(
    mut keys: MessageReader<KeyMessage>,
    capabilities: Res<InputCapabilities>,
    mut output: MessageWriter<KeyboardInput>,
    mut held_modifiers: Local<KeyModifiers>,
) {
    for message in keys.read() {
        if !capabilities.modifier_keys {
            sync_modifiers(&mut held_modifiers, message.modifiers, &mut output);
        }
        output.write(convert(message));
    }
}

/// One key per [`KeyModifiers`] flag; a flag with no entry here never has
/// its transitions synthesized.
const SYNTHESIZED_MODIFIERS: [ModifierKey; 6] = [
    ModifierKey::ShiftLeft,
    ModifierKey::ControlLeft,
    ModifierKey::AltLeft,
    ModifierKey::SuperLeft,
    ModifierKey::HyperLeft,
    ModifierKey::MetaLeft,
];

fn sync_modifiers(
    previous: &mut KeyModifiers,
    current: KeyModifiers,
    output: &mut MessageWriter<KeyboardInput>,
) {
    for modifier in SYNTHESIZED_MODIFIERS {
        let is_held = current.holds(modifier);
        if previous.holds(modifier) == is_held {
            continue;
        }
        output.write(KeyboardInput {
            key_code: modifier_physical(modifier),
            logical_key: modifier_logical(modifier),
            state: if is_held {
                ButtonState::Pressed
            } else {
                ButtonState::Released
            },
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    *previous = current;
}

fn convert(message: &KeyMessage) -> KeyboardInput {
    let pressed = message.kind != KeyKind::Release;
    let text = match message.code {
        KeyCode::Char(c) if pressed => Some(c.to_string().into()),
        _ => None,
    };
    KeyboardInput {
        key_code: physical_code(message.code),
        logical_key: logical_key(message.code),
        state: if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        },
        text,
        repeat: message.kind == KeyKind::Repeat,
        window: Entity::PLACEHOLDER,
    }
}

#[cfg(test)]
mod tests {
    use bevy_ecs::message::Messages;
    use bevy_ecs::prelude::World;
    use bevy_ecs::system::SystemId;
    use bevy_input::ButtonState;
    use bevy_input::keyboard::{Key, KeyCode as BevyKeyCode, KeyboardInput};

    use super::{SYNTHESIZED_MODIFIERS, convert, forward_keyboard};
    use crate::{InputCapabilities, KeyCode, KeyKind, KeyMessage, KeyModifiers, ModifierKey};

    fn forwarding_world(modifier_keys: bool) -> (World, SystemId) {
        let mut world = World::new();
        world.init_resource::<Messages<KeyMessage>>();
        world.init_resource::<Messages<KeyboardInput>>();
        world.insert_resource(InputCapabilities::default().with_modifier_keys(modifier_keys));
        let system = world.register_system(forward_keyboard);
        (world, system)
    }

    fn forward(world: &mut World, system: SystemId, message: KeyMessage) -> Vec<KeyboardInput> {
        world.write_message(message);
        world.run_system(system).unwrap();
        world
            .resource_mut::<Messages<KeyboardInput>>()
            .drain()
            .collect()
    }

    fn tab_with(modifiers: KeyModifiers) -> KeyMessage {
        KeyMessage {
            code: KeyCode::Tab,
            modifiers,
            kind: KeyKind::Press,
        }
    }

    fn shifted_tab(shift: bool) -> KeyMessage {
        tab_with(KeyModifiers::default().with_shift(shift))
    }

    #[test]
    fn synthesized_list_covers_every_modifier_flag() {
        let covered =
            SYNTHESIZED_MODIFIERS
                .into_iter()
                .fold(KeyModifiers::default(), |mut covered, key| {
                    *covered.slot(key) = true;
                    covered
                });

        assert_eq!(
            covered,
            KeyModifiers {
                ctrl: true,
                alt: true,
                shift: true,
                super_key: true,
                hyper: true,
                meta: true,
            },
            "a modifier flag has no key in SYNTHESIZED_MODIFIERS, so its \
             transitions never reach bevy_input on the legacy tier"
        );
    }

    #[test]
    fn synthesizes_hyper_transitions_without_the_capability() {
        let (mut world, system) = forwarding_world(false);

        let hyper = KeyModifiers::default().with_hyper(true);
        let outputs = forward(&mut world, system, tab_with(hyper));

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].key_code, BevyKeyCode::Hyper);
        assert_eq!(outputs[0].state, ButtonState::Pressed);
    }

    #[test]
    fn synthesizes_modifier_transitions_without_the_capability() {
        let (mut world, system) = forwarding_world(false);

        let outputs = forward(&mut world, system, shifted_tab(true));
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].key_code, BevyKeyCode::ShiftLeft);
        assert_eq!(outputs[0].state, ButtonState::Pressed);
        assert_eq!(outputs[1].key_code, BevyKeyCode::Tab);

        let outputs = forward(&mut world, system, shifted_tab(false));
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].key_code, BevyKeyCode::ShiftLeft);
        assert_eq!(outputs[0].state, ButtonState::Released);
    }

    #[test]
    fn forwards_real_modifier_events_without_synthesis() {
        let (mut world, system) = forwarding_world(true);

        let outputs = forward(
            &mut world,
            system,
            KeyMessage {
                code: KeyCode::Modifier(ModifierKey::ShiftRight),
                modifiers: KeyModifiers::default().with_shift(true),
                kind: KeyKind::Press,
            },
        );
        assert_eq!(outputs.len(), 1, "no synthesized duplicate: {outputs:?}");
        assert_eq!(outputs[0].key_code, BevyKeyCode::ShiftRight);
        assert_eq!(outputs[0].state, ButtonState::Pressed);

        let outputs = forward(&mut world, system, shifted_tab(true));
        assert_eq!(
            outputs.len(),
            1,
            "bitfield synthesis gated off: {outputs:?}"
        );
        assert_eq!(outputs[0].key_code, BevyKeyCode::Tab);
    }

    #[test]
    fn converts_char_press_with_text() {
        let input = convert(&KeyMessage {
            code: KeyCode::Char('w'),
            modifiers: KeyModifiers::default(),
            kind: KeyKind::Press,
        });
        assert_eq!(input.key_code, BevyKeyCode::KeyW);
        assert_eq!(input.logical_key, Key::Character("w".into()));
        assert_eq!(input.state, ButtonState::Pressed);
        assert_eq!(input.text, Some("w".into()));
        assert!(!input.repeat);
    }

    #[test]
    fn release_and_repeat_map_to_state_and_flag() {
        let release = convert(&KeyMessage {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::default(),
            kind: KeyKind::Release,
        });
        assert_eq!(release.state, ButtonState::Released);
        assert_eq!(release.text, None);

        let repeat = convert(&KeyMessage {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::default(),
            kind: KeyKind::Repeat,
        });
        assert_eq!(repeat.state, ButtonState::Pressed);
        assert!(repeat.repeat);
    }
}
