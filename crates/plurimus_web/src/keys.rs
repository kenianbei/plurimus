//! Browser keyboard events in `plurimus_term`'s vocabulary.

use plurimus_term::{KeyCode, KeyModifiers, ModifierKey};

/// `KeyboardEvent.location` for the right-hand copy of a modifier key.
const LOCATION_RIGHT: u32 = 2;

/// The highest function key a browser names (`"F24"`).
const MAX_FUNCTION_KEY: u8 = 24;

/// Keys left to the browser by default: reload, hard reload and devtools,
/// with the command-key reloads a Mac keyboard uses.
pub(crate) const DEFAULT_PASSTHROUGH: [(KeyCode, KeyModifiers); 7] = [
    (KeyCode::F(5), KeyModifiers::none()),
    (KeyCode::Char('r'), KeyModifiers::none().with_ctrl(true)),
    (
        KeyCode::Char('R'),
        KeyModifiers::none().with_ctrl(true).with_shift(true),
    ),
    (
        KeyCode::Char('r'),
        KeyModifiers::none().with_super_key(true),
    ),
    (
        KeyCode::Char('R'),
        KeyModifiers::none().with_super_key(true).with_shift(true),
    ),
    (KeyCode::F(12), KeyModifiers::none()),
    (
        KeyCode::Char('I'),
        KeyModifiers::none().with_ctrl(true).with_shift(true),
    ),
];

/// The modifier flags a browser event carries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "one flag per property the DOM event exposes"
)]
pub(crate) struct ModifierFlags {
    pub(crate) ctrl: bool,
    pub(crate) alt: bool,
    pub(crate) shift: bool,
    pub(crate) meta: bool,
}

/// The browser's meta key is the OS key - command on a Mac, the Windows key
/// elsewhere - which is what `super_key` names.
pub(crate) const fn modifiers(flags: ModifierFlags) -> KeyModifiers {
    KeyModifiers::none()
        .with_ctrl(flags.ctrl)
        .with_alt(flags.alt)
        .with_shift(flags.shift)
        .with_super_key(flags.meta)
}

/// Maps `KeyboardEvent.key` to a key, or `None` for what the contract has no
/// word for: dead keys, IME composition, `AltGraph`, media keys.
///
/// A single character is that character, already shifted, which is how a
/// terminal reports it too.
pub(crate) fn key_code(key: &str, location: u32) -> Option<KeyCode> {
    let mut chars = key.chars();
    if let (Some(character), None) = (chars.next(), chars.next()) {
        return Some(KeyCode::Char(character));
    }
    let is_right = location == LOCATION_RIGHT;
    let code = match key {
        "Enter" => KeyCode::Enter,
        "Escape" => KeyCode::Esc,
        "Tab" => KeyCode::Tab,
        "Backspace" => KeyCode::Backspace,
        "Delete" => KeyCode::Delete,
        "Insert" => KeyCode::Insert,
        "ArrowUp" => KeyCode::Up,
        "ArrowDown" => KeyCode::Down,
        "ArrowLeft" => KeyCode::Left,
        "ArrowRight" => KeyCode::Right,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        "CapsLock" => KeyCode::CapsLock,
        "NumLock" => KeyCode::NumLock,
        "ScrollLock" => KeyCode::ScrollLock,
        "Shift" => sided(is_right, ModifierKey::ShiftLeft, ModifierKey::ShiftRight),
        "Control" => sided(
            is_right,
            ModifierKey::ControlLeft,
            ModifierKey::ControlRight,
        ),
        "Alt" => sided(is_right, ModifierKey::AltLeft, ModifierKey::AltRight),
        "Meta" | "OS" | "Super" => sided(is_right, ModifierKey::SuperLeft, ModifierKey::SuperRight),
        "Hyper" => sided(is_right, ModifierKey::HyperLeft, ModifierKey::HyperRight),
        _ => return function_key(key),
    };
    Some(code)
}

const fn sided(is_right: bool, left: ModifierKey, right: ModifierKey) -> KeyCode {
    KeyCode::Modifier(if is_right { right } else { left })
}

fn function_key(key: &str) -> Option<KeyCode> {
    let number = key.strip_prefix('F')?.parse::<u8>().ok()?;
    (1..=MAX_FUNCTION_KEY)
        .contains(&number)
        .then_some(KeyCode::F(number))
}

/// Whether the browser keeps this key rather than the app: an exact match
/// of key and modifiers against `passthrough`.
pub(crate) fn passes_through(
    passthrough: &[(KeyCode, KeyModifiers)],
    code: KeyCode,
    modifiers: KeyModifiers,
) -> bool {
    passthrough
        .iter()
        .any(|&(listed, listed_modifiers)| listed == code && listed_modifiers == modifiers)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEFT: u32 = 1;

    #[test]
    fn a_single_character_is_that_character_as_shifted() {
        assert_eq!(key_code("w", 0), Some(KeyCode::Char('w')));
        assert_eq!(key_code("W", 0), Some(KeyCode::Char('W')));
        assert_eq!(key_code(" ", 0), Some(KeyCode::Char(' ')));
        assert_eq!(key_code("é", 0), Some(KeyCode::Char('é')));
    }

    #[test]
    fn named_keys_map_to_their_variants() {
        assert_eq!(key_code("Enter", 0), Some(KeyCode::Enter));
        assert_eq!(key_code("Escape", 0), Some(KeyCode::Esc));
        assert_eq!(key_code("ArrowLeft", 0), Some(KeyCode::Left));
        assert_eq!(key_code("PageDown", 0), Some(KeyCode::PageDown));
    }

    #[test]
    fn function_keys_stop_at_f24() {
        assert_eq!(key_code("F1", 0), Some(KeyCode::F(1)));
        assert_eq!(key_code("F24", 0), Some(KeyCode::F(24)));
        assert_eq!(key_code("F25", 0), None);
        assert_eq!(key_code("F0", 0), None);
    }

    #[test]
    fn modifier_keys_take_their_side_from_the_location() {
        assert_eq!(
            key_code("Shift", LEFT),
            Some(KeyCode::Modifier(ModifierKey::ShiftLeft))
        );
        assert_eq!(
            key_code("Control", LOCATION_RIGHT),
            Some(KeyCode::Modifier(ModifierKey::ControlRight))
        );
        assert_eq!(
            key_code("Meta", LEFT),
            Some(KeyCode::Modifier(ModifierKey::SuperLeft))
        );
    }

    #[test]
    fn keys_the_contract_has_no_word_for_are_dropped() {
        for key in [
            "Dead",
            "Process",
            "Unidentified",
            "AltGraph",
            "MediaPlayPause",
        ] {
            assert_eq!(key_code(key, 0), None, "{key}");
        }
    }

    #[test]
    fn meta_is_the_super_key() {
        let flags = ModifierFlags {
            meta: true,
            shift: true,
            ..ModifierFlags::default()
        };
        assert_eq!(
            modifiers(flags),
            KeyModifiers::none().with_super_key(true).with_shift(true)
        );
    }

    #[test]
    fn passthrough_matches_key_and_modifiers_exactly() {
        let reload = KeyModifiers::none().with_ctrl(true);
        assert!(passes_through(
            &DEFAULT_PASSTHROUGH,
            KeyCode::Char('r'),
            reload
        ));
        assert!(passes_through(
            &DEFAULT_PASSTHROUGH,
            KeyCode::F(5),
            KeyModifiers::none()
        ));
        assert!(!passes_through(
            &DEFAULT_PASSTHROUGH,
            KeyCode::Char('r'),
            KeyModifiers::none()
        ));
        assert!(!passes_through(
            &DEFAULT_PASSTHROUGH,
            KeyCode::Char('r'),
            reload.with_alt(true)
        ));
        assert!(!passes_through(
            &DEFAULT_PASSTHROUGH,
            KeyCode::Tab,
            KeyModifiers::none()
        ));
    }
}
