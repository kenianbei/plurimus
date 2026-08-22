//! The shared shape of a widget's key bindings.
//!
//! A widget that takes movement keys holds them as data rather than a
//! closed match, so an app remaps it without reimplementing the movement
//! beside it. The bindings live on the widget's own component - the
//! action type differs per widget - and this is the pair they are written
//! in and the scan they share.

use bevy_input::ButtonState;
use bevy_input::keyboard::{Key, KeyboardInput};
use plurimus_term::KeyModifiers;

/// A key and the modifiers it must be pressed under.
///
/// Every modifier but shift is matched exactly, so a binding for `Ctrl+D`
/// does not fire on `Ctrl+Alt+D`, on `Super+D`, or on a bare `D` - the same
/// "chorded" a text field refuses to type under. `shift` is matched exactly
/// for a named key, and for a [`Key::Character`] only when the binding asks
/// for it: a character already says whether it was shifted, and the modifier
/// beside it is not dependable - a shifted symbol carries it on some terminals
/// and not others. So `G` and `:` are spelled as themselves with no
/// `with_shift` and match however the terminal reported the key, while
/// `with_shift` is for what has no shifted spelling: `Shift+Tab`, the shifted
/// arrows, and `Shift+Space`.
///
/// The modifiers a binding is checked against are the ones held when the key
/// arrived, as polled state - bevy's [`KeyboardInput`] carries none of its
/// own. That read is settled before any key observer runs, so it is right for
/// every case but a chord landing in the same frame as its modifier's
/// release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    /// The key itself.
    pub key: Key,
    /// The modifiers it must be pressed under.
    pub modifiers: KeyModifiers,
}

impl KeyBinding {
    /// `key` with no modifiers.
    #[must_use]
    pub const fn new(key: Key) -> Self {
        Self {
            key,
            modifiers: KeyModifiers::none(),
        }
    }

    /// The same binding with control held.
    #[must_use]
    pub const fn with_ctrl(mut self) -> Self {
        self.modifiers = self.modifiers.with_ctrl(true);
        self
    }

    /// The same binding with alt held.
    #[must_use]
    pub const fn with_alt(mut self) -> Self {
        self.modifiers = self.modifiers.with_alt(true);
        self
    }

    /// The same binding with shift held; see the type's note on which keys
    /// that applies to.
    #[must_use]
    pub const fn with_shift(mut self) -> Self {
        self.modifiers = self.modifiers.with_shift(true);
        self
    }

    /// Whether `input`, arriving with `held` down, is this binding.
    ///
    /// The key state is not consulted: a release and a repeat match the same
    /// as a press, and whoever scans decides what each means - [`first_bound`]
    /// binds a release to nothing.
    #[must_use]
    pub fn matches(&self, input: &KeyboardInput, held: KeyModifiers) -> bool {
        let wanted = self.modifiers;
        let shift_agrees = if matches!(self.key, Key::Character(_)) {
            !wanted.shift || held.shift
        } else {
            wanted.shift == held.shift
        };
        self.key == input.logical_key
            && wanted.ctrl == held.ctrl
            && wanted.alt == held.alt
            && wanted.super_key == held.super_key
            && wanted.hyper == held.hyper
            && wanted.meta == held.meta
            && shift_agrees
    }
}

impl From<Key> for KeyBinding {
    fn from(key: Key) -> Self {
        Self::new(key)
    }
}

/// The action bound to `input`'s key under the `held` modifiers, or `None`
/// if nothing is.
///
/// Scanned in order so the first match wins, which is what lets an app
/// shadow one binding by putting it ahead of the rest. A release binds to
/// nothing; a repeat binds as the press it is, so a held key keeps
/// acting. A widget that must act once per physical press checks
/// [`KeyboardInput::repeat`] itself.
#[must_use]
pub fn first_bound<A: Copy>(
    bindings: &[(KeyBinding, A)],
    input: &KeyboardInput,
    held: KeyModifiers,
) -> Option<A> {
    if input.state != ButtonState::Pressed {
        return None;
    }
    bindings
        .iter()
        .find(|(binding, _)| binding.matches(input, held))
        .map(|(_, action)| *action)
}

#[cfg(test)]
mod tests {
    use bevy_ecs::entity::Entity;
    use bevy_input::ButtonState;
    use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};
    use plurimus_term::KeyModifiers;

    use super::{KeyBinding, first_bound};

    fn pressed(key: Key) -> KeyboardInput {
        KeyboardInput {
            key_code: KeyCode::Unidentified(bevy_input::keyboard::NativeKeyCode::Unidentified),
            logical_key: key,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        }
    }

    fn character(text: &str) -> Key {
        Key::Character(text.into())
    }

    const BARE: KeyModifiers = KeyModifiers::none();
    const CTRL: KeyModifiers = KeyModifiers::none().with_ctrl(true);
    const SHIFT: KeyModifiers = KeyModifiers::none().with_shift(true);

    #[test]
    fn a_bare_binding_is_the_key_alone() {
        let binding = KeyBinding::new(character("d"));

        assert!(binding.matches(&pressed(character("d")), BARE));
        assert!(!binding.matches(&pressed(character("d")), CTRL));
        assert!(!binding.matches(&pressed(character("e")), BARE));
    }

    #[test]
    fn a_chord_needs_its_modifier_and_no_other() {
        let binding = KeyBinding::new(character("d")).with_ctrl();

        assert!(binding.matches(&pressed(character("d")), CTRL));
        assert!(!binding.matches(&pressed(character("d")), BARE));
        assert!(!binding.matches(&pressed(character("d")), CTRL.with_alt(true)));
    }

    // The shifted-symbol trap: a `:` arrives with shift physically held, and a
    // `G` does too, so a character binding cannot care.
    #[test]
    fn a_character_binding_ignores_shift_either_way() {
        let colon = KeyBinding::new(character(":"));

        assert!(colon.matches(&pressed(character(":")), SHIFT));
        assert!(colon.matches(&pressed(character(":")), BARE));
        assert!(
            KeyBinding::new(character("G")).matches(&pressed(character("G")), SHIFT),
            "a capital is spelled as itself, never with_shift"
        );
    }

    #[test]
    fn a_chord_on_another_modifier_is_not_the_bare_key() {
        let bare = KeyBinding::new(character("d"));

        assert!(!bare.matches(&pressed(character("d")), BARE.with_super_key(true)));
        assert!(!bare.matches(&pressed(character("d")), BARE.with_meta(true)));
    }

    // Space is a character with no shifted spelling, so it is the one
    // character binding `with_shift` means something on.
    #[test]
    fn a_character_binding_asking_for_shift_requires_it() {
        let shift_space = KeyBinding::new(character(" ")).with_shift();

        assert!(shift_space.matches(&pressed(character(" ")), SHIFT));
        assert!(!shift_space.matches(&pressed(character(" ")), BARE));
        assert!(
            KeyBinding::new(character(" ")).matches(&pressed(character(" ")), SHIFT),
            "and the bare binding still takes the shifted press"
        );
    }

    #[test]
    fn a_named_key_binding_honours_shift() {
        let shift_tab = KeyBinding::new(Key::Tab).with_shift();

        assert!(shift_tab.matches(&pressed(Key::Tab), SHIFT));
        assert!(!shift_tab.matches(&pressed(Key::Tab), BARE));
        assert!(!KeyBinding::new(Key::Tab).matches(&pressed(Key::Tab), SHIFT));
    }

    #[test]
    fn a_key_converts_to_its_bare_binding() {
        assert_eq!(KeyBinding::from(Key::Home), KeyBinding::new(Key::Home));
    }

    #[test]
    fn the_first_match_wins_and_a_release_binds_nothing() {
        let bindings = [
            (KeyBinding::new(Key::Home).with_ctrl(), "top"),
            (KeyBinding::new(Key::Home), "line start"),
            (KeyBinding::new(Key::Home), "never reached"),
        ];

        assert_eq!(
            first_bound(&bindings, &pressed(Key::Home), CTRL),
            Some("top")
        );
        assert_eq!(
            first_bound(&bindings, &pressed(Key::Home), BARE),
            Some("line start")
        );
        let mut released = pressed(Key::Home);
        released.state = ButtonState::Released;
        assert_eq!(first_bound(&bindings, &released, BARE), None);
    }
}
