//! The key releases no terminal will send.
//!
//! Two gaps, both ending with a release written as if a backend had reported
//! it. Most terminals report only key presses, so a press without a real
//! release is expired on a timeout, and that half turns itself off when
//! [`InputCapabilities`](crate::InputCapabilities) says the terminal reports
//! releases itself. The other half no capability covers: a terminal reports
//! nothing at all while unfocused, so a key held across a focus loss is
//! released here or stays down forever.

use std::collections::HashMap;
use std::time::Duration;

use bevy_ecs::prelude::{Local, MessageReader, MessageWriter, ParamSet, Res, Resource};
use bevy_ecs::system::SystemParam;
use bevy_input::ButtonInput;
use bevy_time::{Real, Time};

use super::{FocusMessage, InputCapabilities, KeyCode, KeyKind, KeyMessage, KeyModifiers};

/// Keyed by code alone: a terminal reports the state an event leaves behind,
/// so a shift+a gesture ends with an `a` release carrying no bits, and a
/// press it has to cancel that carried them.
type HeldKeys = HashMap<KeyCode, Duration>;

/// How long after the last press/repeat a key without release events is
/// considered released. Must exceed the OS first-repeat delay.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ReleaseTimeout(pub Duration);

impl Default for ReleaseTimeout {
    fn default() -> Self {
        Self(Duration::from_millis(600))
    }
}

#[derive(SystemParam)]
pub(crate) struct SynthesisClock<'w> {
    capabilities: Res<'w, InputCapabilities>,
    timeout: Res<'w, ReleaseTimeout>,
    time: Res<'w, Time<Real>>,
}

pub(crate) fn synthesize_releases(
    clock: SynthesisClock,
    mut messages: ParamSet<(MessageReader<KeyMessage>, MessageWriter<KeyMessage>)>,
    mut held: Local<HeldKeys>,
) {
    if clock.capabilities.key_release {
        return;
    }
    let now = clock.time.elapsed();
    let mut reader = messages.p0();
    for message in reader.read() {
        match message.kind {
            KeyKind::Press | KeyKind::Repeat => {
                held.insert(message.code, now);
            }
            KeyKind::Release => {
                held.remove(&message.code);
            }
        }
    }
    let releases = expire_held(&mut held, now, clock.timeout.0);
    let mut writer = messages.p1();
    for message in releases {
        writer.write(message);
    }
}

/// Releases every held key when the terminal reports losing focus.
///
/// No capability covers this gap: a terminal reports nothing at all while
/// unfocused, so the release of a key held across an alt-tab is never sent,
/// and on the kitty tier [`synthesize_releases`] is off and nothing expires
/// it either.
///
/// Keys only, and deliberately. Every keyboard consumer acts on a press, so a
/// synthetic release corrects held state and triggers nothing else; a pointer
/// release is not inert in the same way - it completes a click - so a
/// captured drag is left for a cancellation path that can express it.
///
/// Runs after [`update_button_input`](crate::state::update_button_input), so
/// it sees a key pressed in the same frame the focus was lost - which is the
/// ordinary case, since alt-tab's own keys arrive in that batch. Polled state
/// therefore clears on the next frame rather than this one; a message reader
/// running after `PreUpdate` sees the release immediately.
pub(crate) fn release_keys_on_focus_loss(
    mut focus: MessageReader<FocusMessage>,
    held: Res<ButtonInput<KeyCode>>,
    mut keys: MessageWriter<KeyMessage>,
) {
    if !focus.read().any(|message| !message.gained) {
        return;
    }
    for &code in held.get_pressed() {
        keys.write(synthetic_release(code));
    }
}

/// A release written as if a backend had reported it.
///
/// Carries no modifiers, which is what a terminal reports too: an event
/// describes the state it leaves behind, and nothing is held once the gap
/// this fills has opened.
fn synthetic_release(code: KeyCode) -> KeyMessage {
    KeyMessage::new(code, KeyModifiers::default(), KeyKind::Release)
}

fn expire_held(held: &mut HeldKeys, now: Duration, timeout: Duration) -> Vec<KeyMessage> {
    held.extract_if(|_, at| now.saturating_sub(*at) >= timeout)
        .map(|(code, _)| synthetic_release(code))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy_ecs::message::Messages;
    use bevy_ecs::prelude::World;
    use bevy_input::ButtonInput;
    use bevy_time::{Real, Time};

    use super::{
        HeldKeys, ReleaseTimeout, expire_held, release_keys_on_focus_loss, synthesize_releases,
    };
    use crate::{FocusMessage, InputCapabilities, KeyCode, KeyKind, KeyMessage, KeyModifiers};

    #[test]
    fn stale_keys_expire_as_releases() {
        let mut held = HeldKeys::default();
        held.insert(KeyCode::Char('w'), Duration::ZERO);

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
        assert!(held.is_empty());
    }

    #[test]
    fn refreshed_keys_stay_alive() {
        let mut held = HeldKeys::default();
        held.insert(KeyCode::Char('w'), Duration::from_millis(500));

        let expired = expire_held(
            &mut held,
            Duration::from_millis(700),
            Duration::from_millis(600),
        );
        assert!(expired.is_empty());
        assert_eq!(held.len(), 1);
    }

    /// Whether a press carrying `press` is still held after a release
    /// carrying `release`, with a zero timeout expiring anything that is -
    /// so a press the release failed to cancel arrives as a third message
    /// beside the two written here.
    fn still_held_after_release(press: KeyModifiers, release: KeyModifiers) -> bool {
        let mut world = World::new();
        world.init_resource::<Messages<KeyMessage>>();
        world.insert_resource(InputCapabilities::none());
        world.insert_resource(ReleaseTimeout(Duration::ZERO));
        world.insert_resource(Time::<Real>::default());
        let system = world.register_system(synthesize_releases);

        let code = KeyCode::Char('a');
        world.write_message(KeyMessage::new(code, press, KeyKind::Press));
        world.write_message(KeyMessage::new(code, release, KeyKind::Release));
        world.run_system(system).unwrap();
        world.resource_mut::<Messages<KeyMessage>>().drain().count() > 2
    }

    fn on_focus_change(gained: bool, pressed: &[KeyCode]) -> Vec<KeyMessage> {
        let mut world = World::new();
        world.init_resource::<Messages<KeyMessage>>();
        world.init_resource::<Messages<FocusMessage>>();
        let mut held = ButtonInput::<KeyCode>::default();
        for &code in pressed {
            held.press(code);
        }
        world.insert_resource(held);
        let system = world.register_system(release_keys_on_focus_loss);

        world.write_message(FocusMessage::new(gained));
        world.run_system(system).unwrap();
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
        let shifted = KeyModifiers::default().with_shift(true);
        let bare = KeyModifiers::default();

        assert!(!still_held_after_release(shifted, bare));
        assert!(!still_held_after_release(bare, bare));
    }
}
