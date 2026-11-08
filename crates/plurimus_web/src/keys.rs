//! Browser keyboard events in bevy's vocabulary.

use bevy_input::keyboard::Key;
use plurimus_term::KeyModifiers;

/// The highest function key a browser names (`"F24"`).
const MAX_FUNCTION_KEY: usize = 24;

const FUNCTION_KEYS: [Key; MAX_FUNCTION_KEY] = [
    Key::F1,
    Key::F2,
    Key::F3,
    Key::F4,
    Key::F5,
    Key::F6,
    Key::F7,
    Key::F8,
    Key::F9,
    Key::F10,
    Key::F11,
    Key::F12,
    Key::F13,
    Key::F14,
    Key::F15,
    Key::F16,
    Key::F17,
    Key::F18,
    Key::F19,
    Key::F20,
    Key::F21,
    Key::F22,
    Key::F23,
    Key::F24,
];

/// Keys left to the browser by default: reload, hard reload and devtools,
/// with the command-key reloads a Mac keyboard uses.
pub(crate) fn default_passthrough() -> Vec<(Key, KeyModifiers)> {
    let ctrl = KeyModifiers::none().with_ctrl(true);
    let command = KeyModifiers::none().with_super_key(true);
    vec![
        (Key::F5, KeyModifiers::none()),
        (character("r"), ctrl),
        (character("R"), ctrl.with_shift(true)),
        (character("r"), command),
        (character("R"), command.with_shift(true)),
        (Key::F12, KeyModifiers::none()),
        (character("I"), ctrl.with_shift(true)),
    ]
}

fn character(text: &str) -> Key {
    Key::Character(text.into())
}

/// Maps `KeyboardEvent.key` to a key, or `None` for what the contract has no
/// word for: dead keys, IME composition, `AltGraph`, media keys.
///
/// A single character is that character, already shifted, which is how a
/// terminal reports it too. A modifier key is its key whichever side was
/// struck; the browser's meta key is the OS key - command on a Mac, the
/// Windows key elsewhere - which is `Key::Super`.
pub(crate) fn key(name: &str) -> Option<Key> {
    let mut chars = name.chars();
    if let (Some(_), None) = (chars.next(), chars.next()) {
        return Some(character(name));
    }
    Some(match name {
        "Enter" => Key::Enter,
        "Escape" => Key::Escape,
        "Tab" => Key::Tab,
        "Backspace" => Key::Backspace,
        "Delete" => Key::Delete,
        "Insert" => Key::Insert,
        "ArrowUp" => Key::ArrowUp,
        "ArrowDown" => Key::ArrowDown,
        "ArrowLeft" => Key::ArrowLeft,
        "ArrowRight" => Key::ArrowRight,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "CapsLock" => Key::CapsLock,
        "NumLock" => Key::NumLock,
        "ScrollLock" => Key::ScrollLock,
        "Shift" => Key::Shift,
        "Control" => Key::Control,
        "Alt" => Key::Alt,
        "Meta" | "OS" | "Super" => Key::Super,
        "Hyper" => Key::Hyper,
        _ => return function_key(name),
    })
}

fn function_key(name: &str) -> Option<Key> {
    let number = name.strip_prefix('F')?.parse::<usize>().ok()?;
    FUNCTION_KEYS.get(number.checked_sub(1)?).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_character_is_that_character_as_shifted() {
        assert_eq!(key("w"), Some(character("w")));
        assert_eq!(key("W"), Some(character("W")));
        assert_eq!(key(" "), Some(character(" ")));
        assert_eq!(key("é"), Some(character("é")));
    }

    #[test]
    fn named_keys_map_to_their_variants() {
        assert_eq!(key("Enter"), Some(Key::Enter));
        assert_eq!(key("Escape"), Some(Key::Escape));
        assert_eq!(key("ArrowLeft"), Some(Key::ArrowLeft));
        assert_eq!(key("PageDown"), Some(Key::PageDown));
    }

    #[test]
    fn function_keys_stop_at_f24() {
        assert_eq!(key("F1"), Some(Key::F1));
        assert_eq!(key("F24"), Some(Key::F24));
        assert_eq!(key("F25"), None);
        assert_eq!(key("F0"), None);
    }

    #[test]
    fn modifier_keys_are_their_key_whichever_side() {
        assert_eq!(key("Shift"), Some(Key::Shift));
        assert_eq!(key("Control"), Some(Key::Control));
        assert_eq!(key("Meta"), Some(Key::Super));
    }

    #[test]
    fn keys_the_contract_has_no_word_for_are_dropped() {
        for name in [
            "Dead",
            "Process",
            "Unidentified",
            "AltGraph",
            "MediaPlayPause",
        ] {
            assert_eq!(key(name), None, "{name}");
        }
    }

    #[test]
    fn default_passthrough_is_reload_and_devtools_exactly() {
        let ctrl = KeyModifiers::none().with_ctrl(true);
        let passes = |key, held| default_passthrough().contains(&(key, held));
        assert!(passes(character("r"), ctrl));
        assert!(passes(Key::F5, KeyModifiers::none()));
        assert!(!passes(character("r"), KeyModifiers::none()));
        assert!(!passes(character("r"), ctrl.with_alt(true)));
        assert!(!passes(Key::Tab, KeyModifiers::none()));
    }
}
