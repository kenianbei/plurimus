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

type HeldKeys = HashMap<(KeyCode, KeyModifiers), Duration>;

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
        let entry = (message.code, message.modifiers);
        match message.kind {
            KeyKind::Press | KeyKind::Repeat => {
                held.insert(entry, now);
            }
            KeyKind::Release => {
                held.remove(&entry);
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
        .map(|((code, modifiers), _)| KeyMessage {
            code,
            modifiers,
            kind: KeyKind::Release,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{HeldKeys, expire_held};
    use crate::{KeyCode, KeyKind, KeyModifiers};

    #[test]
    fn stale_keys_expire_as_releases() {
        let mut held = HeldKeys::default();
        held.insert(
            (KeyCode::Char('w'), KeyModifiers::default()),
            Duration::ZERO,
        );

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
        held.insert(
            (KeyCode::Char('w'), KeyModifiers::default()),
            Duration::from_millis(500),
        );

        let expired = expire_held(
            &mut held,
            Duration::from_millis(700),
            Duration::from_millis(600),
        );
        assert!(expired.is_empty());
        assert_eq!(held.len(), 1);
    }
}
