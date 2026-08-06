//! Best-effort mapping from the plurimus key vocabulary to bevy's
//! physical and logical key types.

use bevy_input::keyboard::{Key, KeyCode as BevyKeyCode, NativeKeyCode};

use super::super::{KeyCode, ModifierKey};

const LETTERS: [BevyKeyCode; 26] = [
    BevyKeyCode::KeyA,
    BevyKeyCode::KeyB,
    BevyKeyCode::KeyC,
    BevyKeyCode::KeyD,
    BevyKeyCode::KeyE,
    BevyKeyCode::KeyF,
    BevyKeyCode::KeyG,
    BevyKeyCode::KeyH,
    BevyKeyCode::KeyI,
    BevyKeyCode::KeyJ,
    BevyKeyCode::KeyK,
    BevyKeyCode::KeyL,
    BevyKeyCode::KeyM,
    BevyKeyCode::KeyN,
    BevyKeyCode::KeyO,
    BevyKeyCode::KeyP,
    BevyKeyCode::KeyQ,
    BevyKeyCode::KeyR,
    BevyKeyCode::KeyS,
    BevyKeyCode::KeyT,
    BevyKeyCode::KeyU,
    BevyKeyCode::KeyV,
    BevyKeyCode::KeyW,
    BevyKeyCode::KeyX,
    BevyKeyCode::KeyY,
    BevyKeyCode::KeyZ,
];

const DIGITS: [BevyKeyCode; 10] = [
    BevyKeyCode::Digit0,
    BevyKeyCode::Digit1,
    BevyKeyCode::Digit2,
    BevyKeyCode::Digit3,
    BevyKeyCode::Digit4,
    BevyKeyCode::Digit5,
    BevyKeyCode::Digit6,
    BevyKeyCode::Digit7,
    BevyKeyCode::Digit8,
    BevyKeyCode::Digit9,
];

const FUNCTION_KEYS: [BevyKeyCode; 12] = [
    BevyKeyCode::F1,
    BevyKeyCode::F2,
    BevyKeyCode::F3,
    BevyKeyCode::F4,
    BevyKeyCode::F5,
    BevyKeyCode::F6,
    BevyKeyCode::F7,
    BevyKeyCode::F8,
    BevyKeyCode::F9,
    BevyKeyCode::F10,
    BevyKeyCode::F11,
    BevyKeyCode::F12,
];

pub(super) fn physical_code(code: KeyCode) -> BevyKeyCode {
    match code {
        KeyCode::Char(c) if c.is_ascii_alphabetic() => {
            LETTERS[(c.to_ascii_lowercase() as u8 - b'a') as usize]
        }
        KeyCode::Char(c) if c.is_ascii_digit() => DIGITS[(c as u8 - b'0') as usize],
        KeyCode::Char(' ') => BevyKeyCode::Space,
        KeyCode::Enter => BevyKeyCode::Enter,
        KeyCode::Esc => BevyKeyCode::Escape,
        KeyCode::Tab => BevyKeyCode::Tab,
        KeyCode::Backspace => BevyKeyCode::Backspace,
        KeyCode::Delete => BevyKeyCode::Delete,
        KeyCode::Insert => BevyKeyCode::Insert,
        KeyCode::Up => BevyKeyCode::ArrowUp,
        KeyCode::Down => BevyKeyCode::ArrowDown,
        KeyCode::Left => BevyKeyCode::ArrowLeft,
        KeyCode::Right => BevyKeyCode::ArrowRight,
        KeyCode::Home => BevyKeyCode::Home,
        KeyCode::End => BevyKeyCode::End,
        KeyCode::PageUp => BevyKeyCode::PageUp,
        KeyCode::PageDown => BevyKeyCode::PageDown,
        KeyCode::F(n) if (1..=12).contains(&n) => FUNCTION_KEYS[(n - 1) as usize],
        KeyCode::CapsLock => BevyKeyCode::CapsLock,
        KeyCode::NumLock => BevyKeyCode::NumLock,
        KeyCode::ScrollLock => BevyKeyCode::ScrollLock,
        KeyCode::Modifier(modifier) => modifier_physical(modifier),
        KeyCode::Char(_) | KeyCode::F(_) => BevyKeyCode::Unidentified(NativeKeyCode::Unidentified),
    }
}

pub(super) fn modifier_physical(modifier: ModifierKey) -> BevyKeyCode {
    use ModifierKey as M;
    match modifier {
        M::ShiftLeft => BevyKeyCode::ShiftLeft,
        M::ShiftRight => BevyKeyCode::ShiftRight,
        M::ControlLeft => BevyKeyCode::ControlLeft,
        M::ControlRight => BevyKeyCode::ControlRight,
        M::AltLeft => BevyKeyCode::AltLeft,
        M::AltRight => BevyKeyCode::AltRight,
        M::SuperLeft => BevyKeyCode::SuperLeft,
        M::SuperRight => BevyKeyCode::SuperRight,
        M::HyperLeft | M::HyperRight => BevyKeyCode::Hyper,
        M::MetaLeft | M::MetaRight => BevyKeyCode::Meta,
    }
}

pub(super) fn logical_key(code: KeyCode) -> Key {
    match code {
        KeyCode::Char(c) => Key::Character(c.to_string().into()),
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Escape,
        KeyCode::Tab => Key::Tab,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Insert => Key::Insert,
        KeyCode::Up => Key::ArrowUp,
        KeyCode::Down => Key::ArrowDown,
        KeyCode::Left => Key::ArrowLeft,
        KeyCode::Right => Key::ArrowRight,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::F(1) => Key::F1,
        KeyCode::F(2) => Key::F2,
        KeyCode::F(3) => Key::F3,
        KeyCode::F(4) => Key::F4,
        KeyCode::F(5) => Key::F5,
        KeyCode::F(6) => Key::F6,
        KeyCode::F(7) => Key::F7,
        KeyCode::F(8) => Key::F8,
        KeyCode::F(9) => Key::F9,
        KeyCode::F(10) => Key::F10,
        KeyCode::F(11) => Key::F11,
        KeyCode::F(12) => Key::F12,
        KeyCode::CapsLock => Key::CapsLock,
        KeyCode::NumLock => Key::NumLock,
        KeyCode::ScrollLock => Key::ScrollLock,
        KeyCode::Modifier(modifier) => modifier_logical(modifier),
        KeyCode::F(_) => Key::Unidentified(bevy_input::keyboard::NativeKey::Unidentified),
    }
}

pub(super) fn modifier_logical(modifier: ModifierKey) -> Key {
    use ModifierKey as M;
    match modifier {
        M::ShiftLeft | M::ShiftRight => Key::Shift,
        M::ControlLeft | M::ControlRight => Key::Control,
        M::AltLeft | M::AltRight => Key::Alt,
        M::SuperLeft | M::SuperRight => Key::Super,
        M::HyperLeft | M::HyperRight => Key::Hyper,
        M::MetaLeft | M::MetaRight => Key::Meta,
    }
}

#[cfg(test)]
mod tests {
    use bevy_input::keyboard::{Key, KeyCode as BevyKeyCode};

    use super::{logical_key, physical_code};
    use crate::{KeyCode, ModifierKey};

    #[test]
    fn modifier_keys_map_sided_with_hyper_meta_folded() {
        assert_eq!(
            physical_code(KeyCode::Modifier(ModifierKey::ControlRight)),
            BevyKeyCode::ControlRight
        );
        assert_eq!(
            physical_code(KeyCode::Modifier(ModifierKey::HyperLeft)),
            BevyKeyCode::Hyper
        );
        assert_eq!(
            logical_key(KeyCode::Modifier(ModifierKey::AltRight)),
            Key::Alt
        );
    }

    #[test]
    fn lock_keys_map_to_their_bevy_counterparts() {
        assert_eq!(physical_code(KeyCode::CapsLock), BevyKeyCode::CapsLock);
        assert_eq!(physical_code(KeyCode::NumLock), BevyKeyCode::NumLock);
        assert_eq!(logical_key(KeyCode::ScrollLock), Key::ScrollLock);
    }

    #[test]
    fn unmappable_keys_fall_back_to_unidentified() {
        assert!(matches!(
            physical_code(KeyCode::Char('é')),
            BevyKeyCode::Unidentified(_)
        ));
        assert_eq!(logical_key(KeyCode::Char('é')), Key::Character("é".into()));
        assert_eq!(physical_code(KeyCode::F(5)), BevyKeyCode::F5);
    }
}
