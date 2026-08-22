//! The key releases no terminal will send.
//!
//! Two gaps, both ending with a release written as if a backend had reported
//! it. Most terminals report only key presses, so a press without a real
//! release is expired on a timeout, and that half turns itself off when
//! [`InputCapabilities`](crate::InputCapabilities) says the terminal reports
//! releases itself. The other half no capability covers: a terminal reports
//! nothing at all while unfocused, so a key held across a focus loss is
//! released here or stays down forever.
//!
//! Both read one [`HeldKeys`] registry, which is why recording into it is a
//! system of its own rather than the first half of the timeout's: what is
//! held has to be known on every tier, and only the expiry is a capability's
//! to turn off.

use std::collections::HashMap;
use std::time::Duration;

use bevy_ecs::prelude::{MessageReader, MessageWriter, Res, ResMut, Resource};
use bevy_time::{Real, Time};

use super::{FocusMessage, InputCapabilities, KeyCode, KeyKind, KeyMessage, KeyModifiers};

/// Every key the crate believes is down, timed from its last press or repeat.
///
/// Keyed by [`KeyCode::held_as`], so shifting a hold does not file a second
/// entry the release of the first can never reach.
#[derive(Resource, Default, Debug)]
pub(crate) struct HeldKeys(HashMap<KeyCode, Duration>);

/// How long after the last press/repeat a key without release events is
/// considered released. Must exceed the OS first-repeat delay.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ReleaseTimeout(pub Duration);

impl Default for ReleaseTimeout {
    fn default() -> Self {
        Self(Duration::from_millis(600))
    }
}

/// Records what is held, on every tier.
///
/// Takes no [`InputCapabilities`], deliberately: a terminal that reports its
/// own releases still reports nothing while unfocused, so there is no tier on
/// which [`release_keys_on_focus_loss`] can do without this.
pub(crate) fn record_held_keys(
    mut keys: MessageReader<KeyMessage>,
    time: Res<Time<Real>>,
    mut held: ResMut<HeldKeys>,
) {
    let now = time.elapsed();
    for message in keys.read() {
        let identity = message.code.held_as();
        match message.kind {
            KeyKind::Press | KeyKind::Repeat => {
                held.0.insert(identity, now);
            }
            KeyKind::Release => {
                held.0.remove(&identity);
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
    mut keys: MessageWriter<KeyMessage>,
) {
    for message in expire_held(&mut held, time.elapsed(), timeout.0) {
        keys.write(message);
    }
}

/// Whether the terminal leaves releases for [`expire_held_keys`] to guess at.
pub(crate) fn releases_are_synthesized(capabilities: Res<InputCapabilities>) -> bool {
    !capabilities.key_release
}

/// Releases every held key when the terminal reports losing focus.
///
/// No capability covers this gap: a terminal reports nothing at all while
/// unfocused, so the release of a key held across an alt-tab is never sent,
/// and on the kitty tier nothing expires it either.
///
/// Keys only, and deliberately. Every keyboard consumer acts on a press, so a
/// synthetic release corrects held state and triggers nothing else; a pointer
/// release is not inert in the same way - it completes a click - so a
/// captured drag is left for a cancellation path that can express it.
///
/// Runs after [`record_held_keys`] and before
/// [`update_button_input`](crate::state::update_button_input): the first puts
/// this frame's presses in the registry, which is the ordinary case since
/// alt-tab's own keys arrive in that batch, and the second turns the releases
/// written here into polled state without a frame's delay.
pub(crate) fn release_keys_on_focus_loss(
    mut focus: MessageReader<FocusMessage>,
    mut held: ResMut<HeldKeys>,
    mut keys: MessageWriter<KeyMessage>,
) {
    if !focus.read().any(|message| !message.gained) {
        return;
    }
    for (code, _) in held.0.drain() {
        keys.write(synthetic_release(code));
    }
}

/// A release written as if a backend had reported it.
///
/// Names the key as it is held rather than as it was struck, and carries no
/// modifiers. Both say the same thing: an event reports the state it leaves
/// behind, and a shifted character is exactly what nothing being held can no
/// longer produce.
fn synthetic_release(code: KeyCode) -> KeyMessage {
    KeyMessage::new(code, KeyModifiers::default(), KeyKind::Release)
}

fn expire_held(held: &mut HeldKeys, now: Duration, timeout: Duration) -> Vec<KeyMessage> {
    held.0
        .extract_if(|_, at| now.saturating_sub(*at) >= timeout)
        .map(|(code, _)| synthetic_release(code))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy_ecs::message::Messages;
    use bevy_ecs::prelude::World;
    use bevy_time::{Real, Time};

    use super::{
        HeldKeys, ReleaseTimeout, expire_held, expire_held_keys, record_held_keys,
        release_keys_on_focus_loss,
    };
    use crate::{FocusMessage, KeyCode, KeyKind, KeyMessage, KeyModifiers};

    fn holding(code: KeyCode, at: Duration) -> HeldKeys {
        let mut held = HeldKeys::default();
        held.0.insert(code.held_as(), at);
        held
    }

    #[test]
    fn stale_keys_expire_as_releases() {
        let mut held = holding(KeyCode::Char('w'), Duration::ZERO);

        let before = expire_held(
            &mut held,
            Duration::from_millis(100),
            Duration::from_millis(600),
        );
        assert!(before.is_empty());

        let after = expire_held(
            &mut held,
            Duration::from_millis(700),
            Duration::from_millis(600),
        );
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].kind, KeyKind::Release);
        assert_eq!(after[0].code, KeyCode::Char('w'));
        assert!(held.0.is_empty());
    }

    #[test]
    fn refreshed_keys_stay_alive() {
        let mut held = holding(KeyCode::Char('w'), Duration::from_millis(500));

        let expired = expire_held(
            &mut held,
            Duration::from_millis(700),
            Duration::from_millis(600),
        );
        assert!(expired.is_empty());
        assert_eq!(held.0.len(), 1);
    }

    // Naming `W` would pair a shifted character with the empty modifiers
    // every synthetic release carries - a message no backend can produce.
    #[test]
    fn a_shifted_hold_expires_as_the_key_it_is_held_as() {
        let mut held = holding(KeyCode::Char('W'), Duration::ZERO);
        assert_eq!(held.0.len(), 1);

        let expired = expire_held(&mut held, Duration::from_millis(700), Duration::ZERO);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].code, KeyCode::Char('w'));
        assert_eq!(expired[0].modifiers, KeyModifiers::none());
    }

    fn world() -> World {
        let mut world = World::new();
        world.init_resource::<Messages<KeyMessage>>();
        world.init_resource::<Messages<FocusMessage>>();
        world.init_resource::<HeldKeys>();
        world.insert_resource(ReleaseTimeout(Duration::ZERO));
        world.insert_resource(Time::<Real>::default());
        world
    }

    /// Whether a press carrying `press` is still held after a release
    /// carrying `release`, with a zero timeout expiring anything that is.
    fn still_held_after_release(press: KeyModifiers, release: KeyModifiers) -> bool {
        let mut world = world();
        let record = world.register_system(record_held_keys);
        let expire = world.register_system(expire_held_keys);

        let code = KeyCode::Char('a');
        world.write_message(KeyMessage::new(code, press, KeyKind::Press));
        world.write_message(KeyMessage::new(code, release, KeyKind::Release));
        world.run_system(record).unwrap();
        world.run_system(expire).unwrap();
        !world.resource::<HeldKeys>().0.is_empty()
    }

    fn on_focus_change(gained: bool, pressed: &[KeyCode]) -> Vec<KeyMessage> {
        let mut world = world();
        let record = world.register_system(record_held_keys);
        let release = world.register_system(release_keys_on_focus_loss);

        for &code in pressed {
            world.write_message(KeyMessage::new(code, KeyModifiers::none(), KeyKind::Press));
        }
        world.run_system(record).unwrap();
        world.resource_mut::<Messages<KeyMessage>>().clear();

        world.write_message(FocusMessage::new(gained));
        world.run_system(release).unwrap();
        world
            .resource_mut::<Messages<KeyMessage>>()
            .drain()
            .collect()
    }

    #[test]
    fn losing_focus_releases_every_held_key() {
        let held = [KeyCode::Char('w'), KeyCode::Left];
        let released = on_focus_change(false, &held);

        assert_eq!(released.len(), 2);
        assert!(
            released
                .iter()
                .all(|message| message.kind == KeyKind::Release)
        );
        for code in held {
            assert!(released.iter().any(|message| message.code == code));
        }
    }

    #[test]
    fn a_shifted_hold_is_released_as_the_key_it_is_held_as() {
        let released = on_focus_change(false, &[KeyCode::Char('W')]);
        assert_eq!(released.len(), 1);
        assert_eq!(released[0].code, KeyCode::Char('w'));
    }

    #[test]
    fn focus_arriving_releases_nothing() {
        assert!(on_focus_change(true, &[KeyCode::Char('w')]).is_empty());
    }

    #[test]
    fn losing_focus_with_nothing_held_is_silent() {
        assert!(on_focus_change(false, &[]).is_empty());
    }

    // A terminal reports the state an event leaves behind, so the real
    // shift+a gesture ends with an `a` release carrying nothing at all.
    #[test]
    fn a_release_cancels_its_press_whatever_bits_it_carries() {
        let shifted = KeyModifiers::none().with_shift(true);
        let bare = KeyModifiers::none();

        assert!(!still_held_after_release(shifted, bare));
        assert!(!still_held_after_release(bare, bare));
    }
}
