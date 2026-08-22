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
//! Both read one [`HeldKeys`] registry, which is why it is maintained even on
//! the tier that expires nothing. Polled state is downstream of the releases
//! written here rather than an input to them, so what is held is answered in
//! one place.

use std::collections::HashMap;
use std::time::Duration;

use bevy_ecs::prelude::{MessageReader, MessageWriter, ParamSet, Res, ResMut, Resource};
use bevy_ecs::system::SystemParam;
use bevy_time::{Real, Time};

use super::{FocusMessage, InputCapabilities, KeyCode, KeyKind, KeyMessage, KeyModifiers};

/// Every key the crate believes is down: the one registry both synthesis
/// paths read, maintained on every tier because a terminal that reports its
/// own releases still reports nothing while unfocused.
///
/// Keyed by [`KeyCode::held_as`] rather than by the code as it arrived, since
/// a terminal reports the state an event leaves behind - a shift+a gesture
/// ends with an `a` release carrying no bits, and shifting a hold ends it
/// with a `W` that has to cancel a `w`.
#[derive(Resource, Default, Debug)]
pub(crate) struct HeldKeys(HashMap<KeyCode, Held>);

/// A held key as last reported, so a synthetic release names the key a
/// message reader saw pressed rather than the identity it is filed under.
#[derive(Debug, Clone, Copy)]
struct Held {
    code: KeyCode,
    at: Duration,
}

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

/// Records what is held, and expires it where no terminal will.
///
/// The recording is unconditional: [`release_keys_on_focus_loss`] needs the
/// registry on every tier. Only expiry is a capability's to turn off, since a
/// terminal that reports releases needs no guess at when a key rose.
pub(crate) fn synthesize_releases(
    clock: SynthesisClock,
    mut messages: ParamSet<(MessageReader<KeyMessage>, MessageWriter<KeyMessage>)>,
    mut held: ResMut<HeldKeys>,
) {
    let now = clock.time.elapsed();
    let mut reader = messages.p0();
    for message in reader.read() {
        let identity = message.code.held_as();
        match message.kind {
            KeyKind::Press | KeyKind::Repeat => {
                held.0.insert(
                    identity,
                    Held {
                        code: message.code,
                        at: now,
                    },
                );
            }
            KeyKind::Release => {
                held.0.remove(&identity);
            }
        }
    }
    if clock.capabilities.key_release {
        return;
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
/// Runs after [`synthesize_releases`] and before
/// [`update_button_input`](crate::state::update_button_input): the first
/// puts this frame's presses in the registry, which is the ordinary case
/// since alt-tab's own keys arrive in that batch, and the second turns the
/// releases written here into polled state without a frame's delay.
pub(crate) fn release_keys_on_focus_loss(
    mut focus: MessageReader<FocusMessage>,
    mut held: ResMut<HeldKeys>,
    mut keys: MessageWriter<KeyMessage>,
) {
    if !focus.read().any(|message| !message.gained) {
        return;
    }
    for (_, entry) in held.0.drain() {
        keys.write(synthetic_release(entry.code));
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
    held.0
        .extract_if(|_, entry| now.saturating_sub(entry.at) >= timeout)
        .map(|(_, entry)| synthetic_release(entry.code))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy_ecs::message::Messages;
    use bevy_ecs::prelude::World;
    use bevy_time::{Real, Time};

    use super::{
        Held, HeldKeys, ReleaseTimeout, expire_held, release_keys_on_focus_loss,
        synthesize_releases,
    };
    use crate::{FocusMessage, InputCapabilities, KeyCode, KeyKind, KeyMessage, KeyModifiers};

    fn holding(code: KeyCode, at: Duration) -> HeldKeys {
        let mut held = HeldKeys::default();
        held.0.insert(code.held_as(), Held { code, at });
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

    // Filed under the key it is held as, released as the key last reported:
    // a terminal would have said `W`, and nothing downstream should have to
    // know the registry folded it.
    #[test]
    fn a_shifted_hold_expires_as_the_key_last_reported() {
        let mut held = holding(KeyCode::Char('W'), Duration::ZERO);

        let expired = expire_held(&mut held, Duration::from_millis(700), Duration::ZERO);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].code, KeyCode::Char('W'));
    }

    fn world_holding(capabilities: InputCapabilities) -> World {
        let mut world = World::new();
        world.init_resource::<Messages<KeyMessage>>();
        world.init_resource::<Messages<FocusMessage>>();
        world.init_resource::<HeldKeys>();
        world.insert_resource(capabilities);
        world.insert_resource(ReleaseTimeout(Duration::ZERO));
        world.insert_resource(Time::<Real>::default());
        world
    }

    /// Whether a press carrying `press` is still held after a release
    /// carrying `release`, with a zero timeout expiring anything that is -
    /// so a press the release failed to cancel arrives as a third message
    /// beside the two written here.
    fn still_held_after_release(press: KeyModifiers, release: KeyModifiers) -> bool {
        let mut world = world_holding(InputCapabilities::none());
        let system = world.register_system(synthesize_releases);

        let code = KeyCode::Char('a');
        world.write_message(KeyMessage::new(code, press, KeyKind::Press));
        world.write_message(KeyMessage::new(code, release, KeyKind::Release));
        world.run_system(system).unwrap();
        world.resource_mut::<Messages<KeyMessage>>().drain().count() > 2
    }

    fn on_focus_change(
        gained: bool,
        pressed: &[KeyCode],
        capabilities: InputCapabilities,
    ) -> Vec<KeyMessage> {
        let mut world = world_holding(capabilities);
        let record = world.register_system(synthesize_releases);
        let release = world.register_system(release_keys_on_focus_loss);

        for &code in pressed {
            world.write_message(KeyMessage::new(
                code,
                KeyModifiers::default(),
                KeyKind::Press,
            ));
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

    fn on_focus_loss(pressed: &[KeyCode]) -> Vec<KeyMessage> {
        on_focus_change(false, pressed, InputCapabilities::default())
    }

    #[test]
    fn losing_focus_releases_every_held_key() {
        let held = [KeyCode::Char('w'), KeyCode::Left];
        let released = on_focus_loss(&held);

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

    // The kitty tier expires nothing, so before the registry was kept there
    // too this is the case that had nothing to drain.
    #[test]
    fn a_terminal_reporting_its_own_releases_still_loses_focus() {
        let released = on_focus_change(
            false,
            &[KeyCode::Char('w')],
            InputCapabilities::default().with_key_release(true),
        );
        assert_eq!(released.len(), 1);
        assert_eq!(released[0].code, KeyCode::Char('w'));
    }

    #[test]
    fn a_shifted_hold_is_released_as_the_key_last_reported() {
        let released = on_focus_loss(&[KeyCode::Char('W')]);
        assert_eq!(released.len(), 1);
        assert_eq!(released[0].code, KeyCode::Char('W'));
    }

    #[test]
    fn focus_arriving_releases_nothing() {
        assert!(
            on_focus_change(true, &[KeyCode::Char('w')], InputCapabilities::default()).is_empty()
        );
    }

    #[test]
    fn losing_focus_with_nothing_held_is_silent() {
        assert!(on_focus_loss(&[]).is_empty());
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
