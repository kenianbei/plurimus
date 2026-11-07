//! The physical key a terminal never reports, inferred from the logical one.

use bevy_input::keyboard::{Key, KeyCode, NativeKeyCode};

const LETTERS: [KeyCode; 26] = [
    KeyCode::KeyA,
    KeyCode::KeyB,
    KeyCode::KeyC,
    KeyCode::KeyD,
    KeyCode::KeyE,
    KeyCode::KeyF,
    KeyCode::KeyG,
    KeyCode::KeyH,
    KeyCode::KeyI,
    KeyCode::KeyJ,
    KeyCode::KeyK,
    KeyCode::KeyL,
    KeyCode::KeyM,
    KeyCode::KeyN,
    KeyCode::KeyO,
    KeyCode::KeyP,
    KeyCode::KeyQ,
    KeyCode::KeyR,
    KeyCode::KeyS,
    KeyCode::KeyT,
    KeyCode::KeyU,
    KeyCode::KeyV,
    KeyCode::KeyW,
    KeyCode::KeyX,
    KeyCode::KeyY,
    KeyCode::KeyZ,
];

const DIGITS: [KeyCode; 10] = [
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

/// The physical key `key` sits on, on a US layout.
///
/// A terminal reports the character a key produced, never the key, so the
/// position is inferred: an ASCII letter or digit, whatever its case, is its
/// own key, a named key its namesake, and a modifier its left side, the bits
/// carrying none. Anything else is `Unidentified`, which is what bevy polls
/// it as - so two such keys held together read as one.
#[must_use]
pub fn physical_code(key: &Key) -> KeyCode {
    match key {
        Key::Character(text) => character_code(text),
        Key::Enter => KeyCode::Enter,
        Key::Escape => KeyCode::Escape,
        Key::Tab => KeyCode::Tab,
        Key::Backspace => KeyCode::Backspace,
        Key::Delete => KeyCode::Delete,
        Key::Insert => KeyCode::Insert,
        Key::ArrowUp => KeyCode::ArrowUp,
        Key::ArrowDown => KeyCode::ArrowDown,
        Key::ArrowLeft => KeyCode::ArrowLeft,
        Key::ArrowRight => KeyCode::ArrowRight,
        Key::Home => KeyCode::Home,
        Key::End => KeyCode::End,
        Key::PageUp => KeyCode::PageUp,
        Key::PageDown => KeyCode::PageDown,
        Key::CapsLock => KeyCode::CapsLock,
        Key::NumLock => KeyCode::NumLock,
        Key::ScrollLock => KeyCode::ScrollLock,
        Key::Shift => KeyCode::ShiftLeft,
        Key::Control => KeyCode::ControlLeft,
        Key::Alt => KeyCode::AltLeft,
        Key::Super => KeyCode::SuperLeft,
        Key::Hyper => KeyCode::Hyper,
        Key::Meta => KeyCode::Meta,
        Key::F1 => KeyCode::F1,
        Key::F2 => KeyCode::F2,
        Key::F3 => KeyCode::F3,
        Key::F4 => KeyCode::F4,
        Key::F5 => KeyCode::F5,
        Key::F6 => KeyCode::F6,
        Key::F7 => KeyCode::F7,
        Key::F8 => KeyCode::F8,
        Key::F9 => KeyCode::F9,
        Key::F10 => KeyCode::F10,
        Key::F11 => KeyCode::F11,
        Key::F12 => KeyCode::F12,
        _ => KeyCode::Unidentified(NativeKeyCode::Unidentified),
    }
}

fn character_code(text: &str) -> KeyCode {
    let mut chars = text.chars();
    let (Some(character), None) = (chars.next(), chars.next()) else {
        return KeyCode::Unidentified(NativeKeyCode::Unidentified);
    };
    match character {
        'a'..='z' | 'A'..='Z' => LETTERS[(character.to_ascii_lowercase() as u8 - b'a') as usize],
        '0'..='9' => DIGITS[(character as u8 - b'0') as usize],
        ' ' => KeyCode::Space,
        _ => KeyCode::Unidentified(NativeKeyCode::Unidentified),
    }
}

#[cfg(test)]
mod tests {
    use bevy_input::keyboard::{Key, KeyCode, NativeKeyCode};

    use super::physical_code;

    #[test]
    fn a_letter_is_its_key_whatever_its_case() {
        assert_eq!(physical_code(&Key::Character("w".into())), KeyCode::KeyW);
        assert_eq!(physical_code(&Key::Character("W".into())), KeyCode::KeyW);
        assert_eq!(physical_code(&Key::Character("7".into())), KeyCode::Digit7);
        assert_eq!(physical_code(&Key::Character(" ".into())), KeyCode::Space);
    }

    #[test]
    fn what_has_no_key_on_a_us_layout_is_unidentified() {
        let unidentified = KeyCode::Unidentified(NativeKeyCode::Unidentified);
        assert_eq!(physical_code(&Key::Character(":".into())), unidentified);
        assert_eq!(physical_code(&Key::Character("ф".into())), unidentified);
        assert_eq!(physical_code(&Key::F13), unidentified);
    }

    #[test]
    fn a_modifier_is_its_left_key() {
        assert_eq!(physical_code(&Key::Shift), KeyCode::ShiftLeft);
        assert_eq!(physical_code(&Key::Hyper), KeyCode::Hyper);
    }
}
