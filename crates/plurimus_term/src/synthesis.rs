//! Synthesized key releases for terminals without release events.
//!
//! Most terminals report only key presses, so held state would never end and
//! polled input would show every key stuck down. A press without a real
//! release is therefore expired on a timeout, and the whole mechanism turns
//! itself off when [`InputCapabilities`](crate::InputCapabilities) says the
//! terminal reports releases itself.

use std::collections::HashMap;
use std::time::Duration;

use bevy_ecs::prelude::{Local, MessageReader, MessageWriter, ParamSet, Res, Resource};
use bevy_ecs::system::SystemParam;
use bevy_time::{Real, Time};

use super::{InputCapabilities, KeyCode, KeyKind, KeyMessage, KeyModifiers};

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

fn expire_held(held: &mut HeldKeys, now: Duration, timeout: Duration) -> Vec<KeyMessage> {
    held.extract_if(|_, at| now.saturating_sub(*at) >= timeout)
        .map(|(code, _)| KeyMessage {
            code,
            modifiers: KeyModifiers::default(),
            kind: KeyKind::Release,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy_ecs::message::Messages;
    use bevy_ecs::prelude::World;
    use bevy_time::{Real, Time};

    use super::{HeldKeys, ReleaseTimeout, expire_held, synthesize_releases};
    use crate::{InputCapabilities, KeyCode, KeyKind, KeyMessage, KeyModifiers};

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

    // A zero timeout expires anything still held on the same run, so a
    // press the release failed to cancel shows up as a third message.
    fn surviving(press: KeyModifiers, release: KeyModifiers) -> Vec<KeyMessage> {
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
        world
            .resource_mut::<Messages<KeyMessage>>()
            .drain()
            .collect()
    }

    #[test]
    fn a_release_cancels_its_press_whatever_bits_it_carries() {
        let shifted = KeyModifiers::default().with_shift(true);
        let bare = KeyModifiers::default();

        // The real gesture: a terminal reports the state the release leaves
        // behind, so shift+a ends with an `a` release carrying nothing.
        assert_eq!(surviving(shifted, bare).len(), 2);
        assert_eq!(surviving(bare, bare).len(), 2);
        assert_eq!(surviving(shifted, shifted).len(), 2);
    }
}
