use bevy_input::ButtonState;
use bevy_input::keyboard::Key;
use crossterm::event::{
    KeyCode as CtKeyCode, KeyEvent, KeyEventKind, KeyModifiers as CtModifiers, MouseButton,
    MouseEvent, MouseEventKind,
};

use crossterm::event::Event;

use plurimus_term::InputCapabilities;

use super::{TerminalPointer, autorepeat_press_at, convert_key, convert_mouse, learn_capabilities};

fn key_event(code: CtKeyCode, kind: KeyEventKind) -> Event {
    let mut key = KeyEvent::new(code, CtModifiers::NONE);
    key.kind = kind;
    Event::Key(key)
}

fn held(code: CtKeyCode) -> [Event; 2] {
    [
        key_event(code, KeyEventKind::Release),
        key_event(code, KeyEventKind::Press),
    ]
}

const MOVED: Event = Event::Mouse(MouseEvent {
    kind: MouseEventKind::Moved,
    column: 1,
    row: 1,
    modifiers: CtModifiers::NONE,
});

#[test]
fn a_release_then_press_of_one_key_is_the_repeat_it_is() {
    assert_eq!(autorepeat_press_at(&held(CtKeyCode::Char('w'))), Some(1));
}

#[test]
fn a_mouse_report_between_the_two_does_not_break_the_pair() {
    let [release, press] = held(CtKeyCode::Right);
    let batch = [release, MOVED, MOVED, press];
    assert_eq!(
        autorepeat_press_at(&batch),
        Some(3),
        "the events between the pair keep their own places"
    );
}

#[test]
fn another_key_between_the_two_does_break_the_pair() {
    let [release, press] = held(CtKeyCode::Char('w'));
    let batch = [
        release,
        key_event(CtKeyCode::Char('d'), KeyEventKind::Press),
        press,
    ];
    assert_eq!(autorepeat_press_at(&batch), None);
}

#[test]
fn a_release_that_ends_a_hold_stays_a_release() {
    let release = key_event(CtKeyCode::Char('w'), KeyEventKind::Release);
    assert_eq!(autorepeat_press_at(std::slice::from_ref(&release)), None);
    assert_eq!(autorepeat_press_at(&[release, MOVED]), None);
}

#[test]
fn a_different_key_pressed_after_a_release_is_not_a_repeat() {
    let batch = [
        key_event(CtKeyCode::Char('w'), KeyEventKind::Release),
        key_event(CtKeyCode::Char('d'), KeyEventKind::Press),
    ];
    assert_eq!(autorepeat_press_at(&batch), None);
}

#[test]
fn a_press_first_is_never_the_start_of_a_pair() {
    let batch = [
        key_event(CtKeyCode::Char('w'), KeyEventKind::Press),
        key_event(CtKeyCode::Char('w'), KeyEventKind::Release),
    ];
    assert_eq!(autorepeat_press_at(&batch), None);
}

#[test]
fn converts_char_with_modifiers() {
    let key = KeyEvent::new(CtKeyCode::Char('q'), CtModifiers::CONTROL);
    let message = convert_key(key).unwrap();
    assert_eq!(message.key, Key::Character("q".into()));
    assert!(message.modifiers.ctrl);
    assert!(!message.modifiers.alt);
    assert_eq!(message.state, ButtonState::Pressed);
    assert!(!message.repeat);
}

#[test]
fn converts_release_kind() {
    let mut key = KeyEvent::new(CtKeyCode::Char('w'), CtModifiers::NONE);
    key.kind = KeyEventKind::Release;
    assert_eq!(convert_key(key).unwrap().state, ButtonState::Released);
}

#[test]
fn back_tab_becomes_shifted_tab() {
    let bare = KeyEvent::new(CtKeyCode::BackTab, CtModifiers::NONE);
    let message = convert_key(bare).unwrap();
    assert_eq!(message.key, Key::Tab);
    assert!(message.modifiers.shift);

    let flagged = KeyEvent::new(CtKeyCode::BackTab, CtModifiers::SHIFT);
    assert_eq!(convert_key(flagged).unwrap(), message);
}

#[test]
fn an_uppercase_letter_carries_shift_on_either_tier() {
    let substituted = KeyEvent::new(CtKeyCode::Char('W'), CtModifiers::NONE);
    let message = convert_key(substituted).unwrap();
    assert_eq!(message.key, Key::Character("W".into()));
    assert!(message.modifiers.shift);

    let legacy = KeyEvent::new(CtKeyCode::Char('W'), CtModifiers::SHIFT);
    assert_eq!(convert_key(legacy).unwrap(), message);
}

#[test]
fn an_unshifted_letter_is_left_alone() {
    let message = convert_key(KeyEvent::new(CtKeyCode::Char('w'), CtModifiers::NONE)).unwrap();
    assert!(!message.modifiers.shift);
}

// A shifted symbol keeps whatever the terminal said: `:` and `;` are
// different characters, and nothing in the event says which key was hit.
#[test]
fn a_shifted_symbol_keeps_the_bit_it_arrived_with() {
    let bare = convert_key(KeyEvent::new(CtKeyCode::Char(':'), CtModifiers::NONE)).unwrap();
    assert!(!bare.modifiers.shift);

    let flagged = convert_key(KeyEvent::new(CtKeyCode::Char(':'), CtModifiers::SHIFT)).unwrap();
    assert!(flagged.modifiers.shift);
}

#[test]
fn plain_tab_carries_no_shift() {
    let key = KeyEvent::new(CtKeyCode::Tab, CtModifiers::NONE);
    let message = convert_key(key).unwrap();
    assert_eq!(message.key, Key::Tab);
    assert!(!message.modifiers.shift);
}

#[test]
fn drops_unmapped_keys() {
    let key = KeyEvent::new(CtKeyCode::KeypadBegin, CtModifiers::NONE);
    assert_eq!(convert_key(key), None);
}

#[test]
fn converts_lock_keys() {
    let caps = KeyEvent::new(CtKeyCode::CapsLock, CtModifiers::NONE);
    assert_eq!(convert_key(caps).unwrap().key, Key::CapsLock);

    let mut num = KeyEvent::new(CtKeyCode::NumLock, CtModifiers::NONE);
    num.kind = KeyEventKind::Release;
    let message = convert_key(num).unwrap();
    assert_eq!(message.key, Key::NumLock);
    assert_eq!(message.state, ButtonState::Released);

    let scroll = KeyEvent::new(CtKeyCode::ScrollLock, CtModifiers::NONE);
    assert_eq!(convert_key(scroll).unwrap().key, Key::ScrollLock);
}

#[test]
fn converts_modifier_keys_without_their_side() {
    use crossterm::event::ModifierKeyCode;

    let mut key = KeyEvent::new(
        CtKeyCode::Modifier(ModifierKeyCode::RightShift),
        CtModifiers::SHIFT,
    );
    key.kind = KeyEventKind::Release;
    let message = convert_key(key).unwrap();
    assert_eq!(message.key, Key::Shift);
    assert_eq!(message.state, ButtonState::Released);

    let iso = KeyEvent::new(
        CtKeyCode::Modifier(ModifierKeyCode::IsoLevel3Shift),
        CtModifiers::NONE,
    );
    assert_eq!(convert_key(iso), None);
}

#[test]
fn converts_mouse_kinds_to_the_sinks_terms() {
    assert_eq!(
        convert_mouse(MouseEventKind::Down(MouseButton::Left)),
        TerminalPointer::Button(plurimus_term::MouseButton::Left, ButtonState::Pressed)
    );
    assert_eq!(
        convert_mouse(MouseEventKind::Drag(MouseButton::Left)),
        TerminalPointer::Moved
    );
    assert_eq!(
        convert_mouse(MouseEventKind::ScrollDown),
        TerminalPointer::Wheel(0, -1)
    );
    assert_eq!(
        convert_mouse(MouseEventKind::ScrollLeft),
        TerminalPointer::Wheel(1, 0)
    );
}

fn learned(batch: &[Event]) -> InputCapabilities {
    learn_capabilities(batch, InputCapabilities::none())
}

#[test]
fn a_release_proves_releases_and_nothing_else() {
    let batch = [
        key_event(CtKeyCode::Char('a'), KeyEventKind::Press),
        key_event(CtKeyCode::Char('a'), KeyEventKind::Release),
    ];
    assert_eq!(
        learned(&batch),
        InputCapabilities::none().with_key_release(true)
    );
}

#[test]
fn a_modifier_key_proves_modifier_keys_and_nothing_else() {
    let shift = CtKeyCode::Modifier(crossterm::event::ModifierKeyCode::LeftShift);
    assert_eq!(
        learned(&[key_event(shift, KeyEventKind::Press)]),
        InputCapabilities::none().with_modifier_keys(true)
    );
}

#[test]
fn a_press_proves_nothing_and_nothing_is_unlearned() {
    let press = [key_event(CtKeyCode::Char('a'), KeyEventKind::Press)];
    assert_eq!(learned(&press), InputCapabilities::none());

    let known = InputCapabilities::none()
        .with_key_release(true)
        .with_modifier_keys(true);
    assert_eq!(learn_capabilities(&press, known), known);
}
