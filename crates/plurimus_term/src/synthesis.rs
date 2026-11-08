//! The input no terminal will send.
//!
//! Two gaps, both ending with a key release written as if a backend had
//! reported it. Most terminals report only key presses, so a press without a
//! real release is expired on a timeout, and that half turns itself off when
//! [`InputCapabilities`](crate::InputCapabilities) says the terminal reports
//! releases itself. The other half no capability covers: a terminal reports
//! nothing at all while unfocused, so a key held across a focus loss is
//! released here or stays down forever - as `bevy_winit` releases what a
//! window held when it loses focus.
//!
//! Both read one [`HeldKeys`] registry, which is why recording into it is a
//! system of its own rather than the first half of the timeout's: what is
//! held has to be known on every tier, and only the expiry is a capability's
//! to turn off.

use std::collections::HashMap;
use std::time::Duration;

use bevy_ecs::prelude::{MessageReader, MessageWriter, Res, ResMut, Resource};
use bevy_input::keyboard::{KeyCode, KeyboardFocusLost, KeyboardInput};
use bevy_input::mouse::MouseButton;
use bevy_input::{ButtonInput, ButtonState};
use bevy_time::{Real, Time};
use bevy_window::WindowFocused;

use crate::InputCapabilities;
use crate::keyboard::is_modifier_code;

/// Every key the crate believes is down - the press that put it there, timed
/// from its last press or repeat.
///
/// Keyed by the physical code bevy polls by, so a hold shifted midway still
/// has one entry: `w` and `W` are both `KeyW`.
#[derive(Resource, Default, Debug)]
pub(crate) struct HeldKeys(HashMap<KeyCode, (KeyboardInput, Duration)>);

/// How long after the last press/repeat a key without release events is
/// considered released. Must exceed the OS first-repeat delay.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ReleaseTimeout(pub Duration);

impl Default for ReleaseTimeout {
    fn default() -> Self {
        Self(Duration::from_millis(600))
    }
}

/// Records what is held, on every tier: a terminal that reports its own
/// releases still reports nothing while unfocused, so there is no tier on
/// which [`release_keys_on_focus_loss`] can do without this.
///
/// [`InputCapabilities`] decides only what is skipped. Where the terminal
/// sends modifiers as bits, the sink writes their keys and is the one to
/// release them, so neither expiry nor a focus loss may see one.
pub(crate) fn record_held_keys(
    mut keys: MessageReader<KeyboardInput>,
    time: Res<Time<Real>>,
    capabilities: Res<InputCapabilities>,
    mut held: ResMut<HeldKeys>,
) {
    let now = time.elapsed();
    for input in keys.read() {
        if !capabilities.modifier_keys && is_modifier_code(input.key_code) {
            continue;
        }
        match input.state {
            ButtonState::Pressed => {
                held.0.insert(input.key_code, (input.clone(), now));
            }
            ButtonState::Released => {
                held.0.remove(&input.key_code);
            }
        }
    }
}

/// Releases what a terminal reporting no releases would have left held.
///
/// Runs only where [`releases_are_synthesized`] says the terminal needs it.
pub(crate) fn expire_held_keys(
    timeout: Res<ReleaseTimeout>,
    time: Res<Time<Real>>,
    mut held: ResMut<HeldKeys>,
    mut keys: MessageWriter<KeyboardInput>,
) {
    keys.write_batch(expire_held(&mut held, time.elapsed(), timeout.0));
}

/// Whether the terminal leaves releases for [`expire_held_keys`] to guess at.
pub(crate) fn releases_are_synthesized(capabilities: Res<InputCapabilities>) -> bool {
    !capabilities.key_release
}

/// Releases every held key when the terminal reports losing focus, and
/// drops every held mouse button.
///
/// No capability covers this gap: a terminal reports nothing at all while
/// unfocused, so the release of a key held across an alt-tab is never sent,
/// and on the kitty tier nothing expires it either. A [`KeyboardFocusLost`]
/// goes with the releases, as `bevy_winit` sends one, and bevy's own polled
/// state answers it.
///
/// The mouse buttons are [`release_buttons_on_focus_loss`]'s, and ending the
/// pointer's gesture is `plurimus_ui`'s, which sees the same focus loss.
///
/// Runs after [`record_held_keys`]: that puts this frame's presses in the
/// registry, which is the ordinary case since alt-tab's own keys arrive in
/// that batch.
pub(crate) fn release_keys_on_focus_loss(
    mut focus: MessageReader<WindowFocused>,
    mut held: ResMut<HeldKeys>,
    mut keys: MessageWriter<KeyboardInput>,
    mut lost: MessageWriter<KeyboardFocusLost>,
) {
    if !focus.read().any(|message| !message.focused) {
        return;
    }
    keys.write_batch(held.0.drain().map(|(_, (press, _))| release_of(press)));
    lost.write(KeyboardFocusLost);
}

/// Drops every button held across a focus loss from the polled state, as
/// `bevy_input` drops the keys on a [`KeyboardFocusLost`]. No release is
/// written: `bevy_picking` would complete a click nobody made from one,
/// where `plurimus_ui` cancels the gesture instead.
pub(crate) fn release_buttons_on_focus_loss(
    mut focus: MessageReader<WindowFocused>,
    mut buttons: ResMut<ButtonInput<MouseButton>>,
) {
    if focus.read().any(|message| !message.focused) {
        buttons.release_all();
    }
}

/// A release written as if a backend had reported it: the press it ends,
/// coming up, producing no text.
fn release_of(press: KeyboardInput) -> KeyboardInput {
    KeyboardInput {
        state: ButtonState::Released,
        text: None,
        repeat: false,
        ..press
    }
}

fn expire_held(
    held: &mut HeldKeys,
    now: Duration,
    timeout: Duration,
) -> impl Iterator<Item = KeyboardInput> {
    held.0
        .extract_if(move |_, (_, at)| now.saturating_sub(*at) >= timeout)
        .map(|(_, (press, _))| release_of(press))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy_ecs::entity::Entity;
    use bevy_ecs::message::Messages;
    use bevy_ecs::prelude::World;
    use bevy_input::keyboard::{Key, KeyCode, KeyboardFocusLost, KeyboardInput};
    use bevy_input::mouse::MouseButton;
    use bevy_input::{ButtonInput, ButtonState};
    use bevy_time::{Real, Time};
    use bevy_window::WindowFocused;

    use super::{
        HeldKeys, ReleaseTimeout, expire_held, expire_held_keys, record_held_keys,
        release_buttons_on_focus_loss, release_keys_on_focus_loss,
    };
    use crate::{InputCapabilities, physical_code};

    fn press(key: Key) -> KeyboardInput {
        KeyboardInput {
            key_code: physical_code(&key),
            logical_key: key,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        }
    }

    fn character(c: &str) -> Key {
        Key::Character(c.into())
    }

    fn holding(key: Key, at: Duration) -> HeldKeys {
        let mut held = HeldKeys::default();
        let pressed = press(key);
        held.0.insert(pressed.key_code, (pressed, at));
        held
    }

    #[test]
    fn stale_keys_expire_as_releases() {
        let mut held = holding(character("w"), Duration::ZERO);

        let before: Vec<_> = expire_held(
            &mut held,
            Duration::from_millis(100),
            Duration::from_millis(600),
        )
        .collect();
        assert_eq!(before, []);

        let after: Vec<_> = expire_held(
            &mut held,
            Duration::from_millis(700),
            Duration::from_millis(600),
        )
        .collect();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].state, ButtonState::Released);
        assert_eq!(after[0].key_code, KeyCode::KeyW);
        assert_eq!(after[0].logical_key, character("w"));
        assert!(held.0.is_empty());
    }

    #[test]
    fn refreshed_keys_stay_alive() {
        let mut held = holding(character("w"), Duration::from_millis(500));
        let expired: Vec<_> = expire_held(
            &mut held,
            Duration::from_millis(700),
            Duration::from_millis(600),
        )
        .collect();
        assert_eq!(expired, []);
        assert_eq!(held.0.len(), 1);
    }

    fn world() -> World {
        let mut world = World::new();
        world.init_resource::<Messages<KeyboardInput>>();
        world.init_resource::<Messages<KeyboardFocusLost>>();
        world.init_resource::<Messages<WindowFocused>>();
        world.init_resource::<HeldKeys>();
        world.init_resource::<InputCapabilities>();
        world.init_resource::<ButtonInput<MouseButton>>();
        world.insert_resource(ReleaseTimeout(Duration::ZERO));
        world.insert_resource(Time::<Real>::default());
        world
    }

    fn expired_after(modifier_keys: bool, pressed: &[Key]) -> Vec<KeyCode> {
        let mut world = world();
        world.insert_resource(InputCapabilities::default().with_modifier_keys(modifier_keys));
        let record = world.register_system(record_held_keys);
        let expire = world.register_system(expire_held_keys);
        for key in pressed {
            world.write_message(press(key.clone()));
        }
        world.run_system(record).unwrap();
        world.resource_mut::<Messages<KeyboardInput>>().clear();
        world.run_system(expire).unwrap();
        world
            .resource_mut::<Messages<KeyboardInput>>()
            .drain()
            .map(|input| input.key_code)
            .collect()
    }

    // Ctrl held past the timeout while Right repeats: on a terminal sending
    // modifiers as bits the sink wrote the Ctrl press and is the one to end it.
    #[test]
    fn a_synthesized_modifier_is_never_expired() {
        let held = [Key::Control, Key::ArrowRight];
        assert_eq!(expired_after(false, &held), [KeyCode::ArrowRight]);
        let mut kitty = expired_after(true, &held);
        kitty.sort();
        assert_eq!(kitty, [KeyCode::ControlLeft, KeyCode::ArrowRight]);
    }

    // The gesture a raw-byte probe caught: hold `w`, press shift, release
    // `w`. Kitty substitutes the shifted character, so the release arrives
    // as `W` - and lands on the same physical key.
    #[test]
    fn a_hold_shifted_before_it_ends_is_still_released() {
        let mut world = world();
        let record = world.register_system(record_held_keys);
        world.write_message(press(character("w")));
        world.write_message(KeyboardInput {
            state: ButtonState::Released,
            ..press(character("W"))
        });
        world.run_system(record).unwrap();
        assert!(world.resource::<HeldKeys>().0.is_empty());
    }

    fn on_focus_change(focused: bool, pressed: &[Key]) -> (Vec<KeyboardInput>, usize) {
        let mut world = world();
        let record = world.register_system(record_held_keys);
        let release = world.register_system(release_keys_on_focus_loss);
        for key in pressed {
            world.write_message(press(key.clone()));
        }
        world.run_system(record).unwrap();
        world.resource_mut::<Messages<KeyboardInput>>().clear();
        world.write_message(WindowFocused {
            window: Entity::PLACEHOLDER,
            focused,
        });
        world.run_system(release).unwrap();
        let released = world
            .resource_mut::<Messages<KeyboardInput>>()
            .drain()
            .collect();
        let lost = world.resource::<Messages<KeyboardFocusLost>>().len();
        (released, lost)
    }

    #[test]
    fn losing_focus_releases_every_held_key_and_says_so() {
        let held = [character("w"), Key::ArrowLeft];
        let (released, lost) = on_focus_change(false, &held);
        assert_eq!(released.len(), 2);
        assert!(
            released
                .iter()
                .all(|input| input.state == ButtonState::Released && input.text.is_none())
        );
        for key in held {
            assert!(released.iter().any(|input| input.logical_key == key));
        }
        assert_eq!(lost, 1);
    }

    #[test]
    fn losing_focus_drops_every_held_button() {
        let mut world = world();
        let release = world.register_system(release_buttons_on_focus_loss);
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        world.write_message(WindowFocused {
            window: Entity::PLACEHOLDER,
            focused: false,
        });
        world.run_system(release).unwrap();
        let buttons = world.resource::<ButtonInput<MouseButton>>();
        assert!(!buttons.pressed(MouseButton::Left));
        assert!(buttons.just_released(MouseButton::Left));
    }

    #[test]
    fn focus_arriving_releases_nothing() {
        assert_eq!(on_focus_change(true, &[character("w")]), (vec![], 0));
    }

    #[test]
    fn losing_focus_with_nothing_held_still_says_so() {
        assert_eq!(on_focus_change(false, &[]), (vec![], 1));
    }
}
