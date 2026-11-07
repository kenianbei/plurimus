//! Modifier state: the bits a terminal reports with an event, and the keys
//! bevy polls them back from.

use bevy_ecs::entity::Entity;
use bevy_ecs::system::{Res, SystemParam};
use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy_input::{ButtonInput, ButtonState};

/// Modifier keys held during an input event.
///
/// One flag per modifier, with left and right collapsed: a right shift
/// sets `shift`. Super/hyper/meta are only reported on the kitty tier.
///
/// A shifted character reports the character, and `shift` beside it is only
/// dependable for letters: `W` carries the flag on every terminal, while a
/// shifted symbol such as `:` carries it on some and not others, depending
/// on whether the terminal reports the unshifted key alongside it. Match the
/// character for those, and compare the fields a binding cares about rather
/// than whole values, since an unexpected flag makes an equality test fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[expect(
    clippy::struct_excessive_bools,
    reason = "one flag per modifier the protocol reports; a bitflags type would be the same set behind an API break"
)]
pub struct KeyModifiers {
    /// Control held.
    pub ctrl: bool,
    /// Alt held.
    pub alt: bool,
    /// Shift held.
    pub shift: bool,
    /// Super/command held.
    pub super_key: bool,
    /// Hyper held.
    pub hyper: bool,
    /// Meta held.
    pub meta: bool,
}

impl Default for KeyModifiers {
    fn default() -> Self {
        Self::none()
    }
}

impl KeyModifiers {
    /// Nothing held: [`Default`] in a `const` context, so a binding table can
    /// be built from the `with_*` builders at compile time.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: false,
            super_key: false,
            hyper: false,
            meta: false,
        }
    }

    /// Sets whether control is held.
    #[must_use]
    pub const fn with_ctrl(mut self, held: bool) -> Self {
        self.ctrl = held;
        self
    }

    /// Sets whether alt is held.
    #[must_use]
    pub const fn with_alt(mut self, held: bool) -> Self {
        self.alt = held;
        self
    }

    /// Sets whether shift is held.
    #[must_use]
    pub const fn with_shift(mut self, held: bool) -> Self {
        self.shift = held;
        self
    }

    /// Sets whether super/command is held.
    #[must_use]
    pub const fn with_super_key(mut self, held: bool) -> Self {
        self.super_key = held;
        self
    }

    /// Sets whether hyper is held.
    #[must_use]
    pub const fn with_hyper(mut self, held: bool) -> Self {
        self.hyper = held;
        self
    }

    /// Sets whether meta is held.
    #[must_use]
    pub const fn with_meta(mut self, held: bool) -> Self {
        self.meta = held;
        self
    }

    fn holds(mut self, modifier: &Modifier) -> bool {
        *(modifier.flag)(&mut self)
    }
}

/// One [`KeyModifiers`] flag: both physical keys that drive it, the logical
/// key they report, and the flag itself.
///
/// Reading and synthesizing both work from this one table, so neither can
/// come to disagree about which keys drive which flag. Hyper and meta have
/// one physical code in bevy, so their pair is the same key twice.
struct Modifier {
    left: KeyCode,
    right: KeyCode,
    logical: Key,
    flag: for<'a> fn(&'a mut KeyModifiers) -> &'a mut bool,
}

const MODIFIERS: [Modifier; 6] = [
    Modifier {
        left: KeyCode::ShiftLeft,
        right: KeyCode::ShiftRight,
        logical: Key::Shift,
        flag: |held| &mut held.shift,
    },
    Modifier {
        left: KeyCode::ControlLeft,
        right: KeyCode::ControlRight,
        logical: Key::Control,
        flag: |held| &mut held.ctrl,
    },
    Modifier {
        left: KeyCode::AltLeft,
        right: KeyCode::AltRight,
        logical: Key::Alt,
        flag: |held| &mut held.alt,
    },
    Modifier {
        left: KeyCode::SuperLeft,
        right: KeyCode::SuperRight,
        logical: Key::Super,
        flag: |held| &mut held.super_key,
    },
    Modifier {
        left: KeyCode::Hyper,
        right: KeyCode::Hyper,
        logical: Key::Hyper,
        flag: |held| &mut held.hyper,
    },
    Modifier {
        left: KeyCode::Meta,
        right: KeyCode::Meta,
        logical: Key::Meta,
        flag: |held| &mut held.meta,
    },
];

/// The modifiers `keys` currently holds, for a consumer that reads bevy's
/// polled key state rather than the bits an event arrived with.
///
/// A `FocusedInput` observer is the case this exists for: bevy's
/// `KeyboardInput` carries no modifier state, so a chord has to come from
/// here. Sides collapse - either control key sets `ctrl`.
///
/// This is frame state, not event state. It answers "what is held now",
/// which is the same thing only while no modifier is released in the frame
/// the key it modified arrives.
#[must_use]
pub fn held_modifiers(keys: &ButtonInput<KeyCode>) -> KeyModifiers {
    let mut held = KeyModifiers::default();
    for modifier in &MODIFIERS {
        *(modifier.flag)(&mut held) = keys.any_pressed([modifier.left, modifier.right]);
    }
    held
}

/// [`held_modifiers`] as a system parameter, for an observer that wants
/// the one answer without naming the resource it comes from.
#[derive(SystemParam)]
pub struct HeldModifiers<'w> {
    keys: Res<'w, ButtonInput<KeyCode>>,
}

impl HeldModifiers<'_> {
    /// The modifiers held now; see [`held_modifiers`] for what "now" means.
    #[must_use]
    pub fn get(&self) -> KeyModifiers {
        held_modifiers(&self.keys)
    }
}

/// The modifier key presses and releases that take `was` to `current`, each
/// naming the left key since the bits carry no side.
///
/// For a terminal that reports modifiers only as bits on other events, these
/// are the key events it never sent.
pub(crate) fn modifier_transitions(
    was: KeyModifiers,
    current: KeyModifiers,
    window: Entity,
) -> impl Iterator<Item = KeyboardInput> + use<> {
    MODIFIERS
        .into_iter()
        .filter(move |modifier| was.holds(modifier) != current.holds(modifier))
        .map(move |modifier| KeyboardInput {
            key_code: modifier.left,
            state: if current.holds(&modifier) {
                ButtonState::Pressed
            } else {
                ButtonState::Released
            },
            logical_key: modifier.logical,
            text: None,
            repeat: false,
            window,
        })
}

#[cfg(test)]
mod tests {
    use bevy_ecs::entity::Entity;
    use bevy_input::ButtonInput;
    use bevy_input::keyboard::KeyCode;

    use super::{KeyModifiers, MODIFIERS, held_modifiers, modifier_transitions};

    fn holding(pressed: &[KeyCode]) -> KeyModifiers {
        let mut keys = ButtonInput::default();
        for &key in pressed {
            keys.press(key);
        }
        held_modifiers(&keys)
    }

    #[test]
    fn either_side_of_every_modifier_sets_the_flag_it_shares() {
        assert_eq!(holding(&[]), KeyModifiers::default(), "nothing held");
        for modifier in &MODIFIERS {
            let mut expected = KeyModifiers::default();
            *(modifier.flag)(&mut expected) = true;
            assert_eq!(holding(&[modifier.left]), expected, "{:?}", modifier.left);
            assert_eq!(holding(&[modifier.right]), expected, "{:?}", modifier.right);
        }
    }

    #[test]
    fn a_key_that_is_no_modifier_is_never_held() {
        assert_eq!(holding(&[KeyCode::KeyW]), KeyModifiers::none());
    }

    #[test]
    fn transitions_name_each_flag_that_changed_once() {
        let none = KeyModifiers::none();
        let shifted = none.with_shift(true).with_hyper(true);
        let pressed: Vec<_> = modifier_transitions(none, shifted, Entity::PLACEHOLDER).collect();
        assert_eq!(pressed.len(), 2);
        assert_eq!(pressed[0].key_code, KeyCode::ShiftLeft);
        assert_eq!(pressed[1].key_code, KeyCode::Hyper);

        let released: Vec<_> = modifier_transitions(shifted, none, Entity::PLACEHOLDER).collect();
        assert_eq!(released.len(), 2);
        assert!(
            modifier_transitions(none, none, Entity::PLACEHOLDER)
                .next()
                .is_none()
        );
    }
}
