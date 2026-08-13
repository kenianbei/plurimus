//! Translates crossterm events into plurimus input messages.
//!
//! Crossterm reports what the terminal sent; the input contract describes
//! what happened. The gap between those is what this file closes - mouse
//! positions become cell coordinates, terminal quirks are normalized into one
//! shape consumers can reason about, and a resize becomes a
//! [`TerminalResized`](plurimus_term::TerminalResized) rather than an event
//! anyone polls for.

use std::time::Duration;

use bevy_ecs::prelude::{Local, MessageWriter};
use bevy_ecs::system::SystemParam;
use crossterm::event::{self, Event, KeyEvent, KeyEventKind, MouseEvent, MouseEventKind};

use plurimus_core::ratatui_core::layout::Position;
use plurimus_term::{
    FocusMessage, KeyCode, KeyKind, KeyMessage, KeyModifiers, ModifierKey, MouseButton, MouseKind,
    MouseMessage, PasteMessage, TerminalResized,
};

#[derive(SystemParam)]
pub(crate) struct EventSinks<'w> {
    keys: MessageWriter<'w, KeyMessage>,
    mouse: MessageWriter<'w, MouseMessage>,
    paste: MessageWriter<'w, PasteMessage>,
    focus: MessageWriter<'w, FocusMessage>,
    resize: MessageWriter<'w, TerminalResized>,
}

/// How long a drain ending on a key release waits for the press that would
/// make it an autorepeat. The two arrive in the same millisecond, so this is
/// a scheduling margin rather than a guess at the terminal.
const AUTOREPEAT_GRACE: Duration = Duration::from_millis(1);

pub(crate) fn pump_events(mut sinks: EventSinks, mut batch: Local<Vec<Event>>) {
    batch.clear();
    while event::poll(Duration::ZERO).unwrap_or(false) {
        let Ok(terminal_event) = event::read() else {
            break;
        };
        batch.push(terminal_event);
    }
    // A terminal encoding autorepeat as release+press can split the pair
    // across two drains, and the leaked release ends a hold that never
    // ended. Waiting out the grace period costs a millisecond on a drain
    // that happens to end on a key-up.
    if ends_on_key_release(&batch)
        && event::poll(AUTOREPEAT_GRACE).unwrap_or(false)
        && let Ok(trailing) = event::read()
    {
        batch.push(trailing);
    }

    let mut index = 0;
    while index < batch.len() {
        let Some(repeat) = autorepeat_at(&batch[index..]) else {
            forward_event(&batch[index], &mut sinks);
            index += 1;
            continue;
        };
        sinks.keys.write(repeat.message);
        for skipped in &batch[index + 1..index + repeat.press_at] {
            forward_event(skipped, &mut sinks);
        }
        index += repeat.press_at + 1;
    }
}

fn ends_on_key_release(batch: &[Event]) -> bool {
    matches!(batch.last(), Some(Event::Key(key)) if key.kind == KeyEventKind::Release)
}

/// An autorepeat reported the legacy way, and where its press sits.
struct Autorepeat {
    message: KeyMessage,
    /// Index of the press within the slice matched, so whatever landed
    /// between the two still reaches its own sink.
    press_at: usize,
}

/// The autorepeat starting at `events[0]`, if that is what it is.
///
/// A terminal that reports releases but not repeats encodes a held key as a
/// release immediately followed by a press of the same key: the key never
/// rose, so the contract reports the pair as the repeat it is rather than
/// letting a release nobody made reach a consumer.
///
/// Events that are not keys are stepped over rather than breaking the pair,
/// since a mouse report can land between the two - and does, for anyone
/// moving the mouse while holding a key. An intervening *key* event does
/// break it: that is a different key's business, not this one's.
fn autorepeat_at(events: &[Event]) -> Option<Autorepeat> {
    let [Event::Key(release), rest @ ..] = events else {
        return None;
    };
    if release.kind != KeyEventKind::Release {
        return None;
    }
    let (offset, press) = rest
        .iter()
        .enumerate()
        .find_map(|(offset, event)| match event {
            Event::Key(key) => Some((offset, key)),
            _ => None,
        })?;
    if press.kind != KeyEventKind::Press || press.code != release.code {
        return None;
    }
    let mut repeat = *press;
    repeat.kind = KeyEventKind::Repeat;
    Some(Autorepeat {
        message: convert_key(repeat)?,
        press_at: offset + 1,
    })
}

fn forward_event(terminal_event: &Event, sinks: &mut EventSinks) {
    match terminal_event {
        Event::Key(key) => {
            if let Some(message) = convert_key(*key) {
                sinks.keys.write(message);
            }
        }
        Event::Mouse(mouse) => {
            sinks.mouse.write(convert_mouse(*mouse));
        }
        Event::Paste(text) => {
            sinks.paste.write(PasteMessage(text.clone()));
        }
        Event::FocusGained => {
            sinks.focus.write(FocusMessage::new(true));
        }
        Event::FocusLost => {
            sinks.focus.write(FocusMessage::new(false));
        }
        Event::Resize(cols, rows) => {
            sinks.resize.write(TerminalResized::new(*cols, *rows));
        }
    }
}

fn convert_key(key: KeyEvent) -> Option<KeyMessage> {
    // Terminals report shift-tab as its own key, and not every one sets
    // the shift bit alongside it; the contract carries it as a modified
    // Tab so consumers need only one key to reason about.
    let is_back_tab = key.code == event::KeyCode::BackTab;
    let modifiers = convert_modifiers(key.modifiers);
    Some(KeyMessage::new(
        convert_code(key.code)?,
        if is_back_tab {
            modifiers.with_shift(true)
        } else {
            modifiers
        },
        convert_kind(key.kind),
    ))
}

const fn convert_kind(kind: KeyEventKind) -> KeyKind {
    match kind {
        KeyEventKind::Press => KeyKind::Press,
        KeyEventKind::Repeat => KeyKind::Repeat,
        KeyEventKind::Release => KeyKind::Release,
    }
}

const fn convert_code(code: event::KeyCode) -> Option<KeyCode> {
    match code {
        event::KeyCode::Char(c) => Some(KeyCode::Char(c)),
        event::KeyCode::Enter => Some(KeyCode::Enter),
        event::KeyCode::Esc => Some(KeyCode::Esc),
        event::KeyCode::Tab | event::KeyCode::BackTab => Some(KeyCode::Tab),
        event::KeyCode::Backspace => Some(KeyCode::Backspace),
        event::KeyCode::Delete => Some(KeyCode::Delete),
        event::KeyCode::Insert => Some(KeyCode::Insert),
        event::KeyCode::Up => Some(KeyCode::Up),
        event::KeyCode::Down => Some(KeyCode::Down),
        event::KeyCode::Left => Some(KeyCode::Left),
        event::KeyCode::Right => Some(KeyCode::Right),
        event::KeyCode::Home => Some(KeyCode::Home),
        event::KeyCode::End => Some(KeyCode::End),
        event::KeyCode::PageUp => Some(KeyCode::PageUp),
        event::KeyCode::PageDown => Some(KeyCode::PageDown),
        event::KeyCode::F(n) => Some(KeyCode::F(n)),
        event::KeyCode::CapsLock => Some(KeyCode::CapsLock),
        event::KeyCode::NumLock => Some(KeyCode::NumLock),
        event::KeyCode::ScrollLock => Some(KeyCode::ScrollLock),
        event::KeyCode::Modifier(modifier) => convert_modifier_key(modifier),
        _ => None,
    }
}

const fn convert_modifier_key(modifier: event::ModifierKeyCode) -> Option<KeyCode> {
    use event::ModifierKeyCode as Ct;
    let key = match modifier {
        Ct::LeftShift => ModifierKey::ShiftLeft,
        Ct::RightShift => ModifierKey::ShiftRight,
        Ct::LeftControl => ModifierKey::ControlLeft,
        Ct::RightControl => ModifierKey::ControlRight,
        Ct::LeftAlt => ModifierKey::AltLeft,
        Ct::RightAlt => ModifierKey::AltRight,
        Ct::LeftSuper => ModifierKey::SuperLeft,
        Ct::RightSuper => ModifierKey::SuperRight,
        Ct::LeftHyper => ModifierKey::HyperLeft,
        Ct::RightHyper => ModifierKey::HyperRight,
        Ct::LeftMeta => ModifierKey::MetaLeft,
        Ct::RightMeta => ModifierKey::MetaRight,
        Ct::IsoLevel3Shift | Ct::IsoLevel5Shift => return None,
    };
    Some(KeyCode::Modifier(key))
}

fn convert_modifiers(modifiers: event::KeyModifiers) -> KeyModifiers {
    KeyModifiers::default()
        .with_ctrl(modifiers.contains(event::KeyModifiers::CONTROL))
        .with_alt(modifiers.contains(event::KeyModifiers::ALT))
        .with_shift(modifiers.contains(event::KeyModifiers::SHIFT))
        .with_super_key(modifiers.contains(event::KeyModifiers::SUPER))
        .with_hyper(modifiers.contains(event::KeyModifiers::HYPER))
        .with_meta(modifiers.contains(event::KeyModifiers::META))
}

fn convert_mouse(mouse: MouseEvent) -> MouseMessage {
    MouseMessage::new(
        convert_mouse_kind(mouse.kind),
        Position::new(mouse.column, mouse.row),
        convert_modifiers(mouse.modifiers),
    )
}

const fn convert_mouse_kind(kind: MouseEventKind) -> MouseKind {
    match kind {
        MouseEventKind::Down(button) => MouseKind::Down(convert_button(button)),
        MouseEventKind::Up(button) => MouseKind::Up(convert_button(button)),
        MouseEventKind::Drag(button) => MouseKind::Drag(convert_button(button)),
        MouseEventKind::Moved => MouseKind::Moved,
        MouseEventKind::ScrollUp => MouseKind::ScrollUp,
        MouseEventKind::ScrollDown => MouseKind::ScrollDown,
        MouseEventKind::ScrollLeft => MouseKind::ScrollLeft,
        MouseEventKind::ScrollRight => MouseKind::ScrollRight,
    }
}

const fn convert_button(button: event::MouseButton) -> MouseButton {
    match button {
        event::MouseButton::Left => MouseButton::Left,
        event::MouseButton::Right => MouseButton::Right,
        event::MouseButton::Middle => MouseButton::Middle,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{
        KeyCode as CtKeyCode, KeyEvent, KeyEventKind, KeyModifiers as CtModifiers, MouseButton,
        MouseEvent, MouseEventKind,
    };
    use plurimus_core::ratatui_core::layout::Position;
    use plurimus_term::{KeyCode, KeyKind, MouseKind};

    use crossterm::event::Event;

    use super::{autorepeat_at, convert_key, convert_mouse, ends_on_key_release};

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
        let batch = held(CtKeyCode::Char('w'));
        let repeat = autorepeat_at(&batch).expect("a held key");
        assert_eq!(repeat.message.kind, KeyKind::Repeat);
        assert_eq!(repeat.message.code, KeyCode::Char('w'));
        assert_eq!(repeat.press_at, 1);
    }

    #[test]
    fn a_mouse_report_between_the_two_does_not_break_the_pair() {
        let [release, press] = held(CtKeyCode::Right);
        let batch = [release, MOVED, MOVED, press];
        let repeat = autorepeat_at(&batch).expect("a held key past the mouse");
        assert_eq!(repeat.message.code, KeyCode::Right);
        assert_eq!(
            repeat.press_at, 3,
            "the skipped events have to stay forwardable"
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
        assert!(autorepeat_at(&batch).is_none());
    }

    #[test]
    fn a_release_that_ends_a_hold_stays_a_release() {
        let lone = [key_event(CtKeyCode::Char('w'), KeyEventKind::Release)];
        assert!(autorepeat_at(&lone).is_none());

        let trailing = [
            key_event(CtKeyCode::Char('w'), KeyEventKind::Release),
            MOVED,
        ];
        assert!(autorepeat_at(&trailing).is_none());
    }

    #[test]
    fn a_different_key_pressed_after_a_release_is_not_a_repeat() {
        let batch = [
            key_event(CtKeyCode::Char('w'), KeyEventKind::Release),
            key_event(CtKeyCode::Char('d'), KeyEventKind::Press),
        ];
        assert!(autorepeat_at(&batch).is_none());
    }

    #[test]
    fn a_press_first_is_never_the_start_of_a_pair() {
        let batch = [
            key_event(CtKeyCode::Char('w'), KeyEventKind::Press),
            key_event(CtKeyCode::Char('w'), KeyEventKind::Release),
        ];
        assert!(autorepeat_at(&batch).is_none());
    }

    // The grace poll is what closes the split-drain case, and it can only be
    // asked for by a batch whose last event is a key release.
    #[test]
    fn only_a_trailing_release_asks_for_the_grace_period() {
        assert!(ends_on_key_release(&[key_event(
            CtKeyCode::Char('w'),
            KeyEventKind::Release
        )]));
        assert!(!ends_on_key_release(&held(CtKeyCode::Char('w'))));
        assert!(!ends_on_key_release(&[MOVED]));
        assert!(!ends_on_key_release(&[]));
    }

    #[test]
    fn converts_char_with_modifiers() {
        let key = KeyEvent::new(CtKeyCode::Char('q'), CtModifiers::CONTROL);
        let message = convert_key(key).unwrap();
        assert_eq!(message.code, KeyCode::Char('q'));
        assert!(message.modifiers.ctrl);
        assert!(!message.modifiers.alt);
        assert_eq!(message.kind, KeyKind::Press);
    }

    #[test]
    fn converts_release_kind() {
        let mut key = KeyEvent::new(CtKeyCode::Char('w'), CtModifiers::NONE);
        key.kind = KeyEventKind::Release;
        assert_eq!(convert_key(key).unwrap().kind, KeyKind::Release);
    }

    #[test]
    fn back_tab_becomes_shifted_tab() {
        let bare = KeyEvent::new(CtKeyCode::BackTab, CtModifiers::NONE);
        let message = convert_key(bare).unwrap();
        assert_eq!(message.code, KeyCode::Tab);
        assert!(message.modifiers.shift);

        let flagged = KeyEvent::new(CtKeyCode::BackTab, CtModifiers::SHIFT);
        assert_eq!(convert_key(flagged).unwrap(), message);
    }

    #[test]
    fn plain_tab_carries_no_shift() {
        let key = KeyEvent::new(CtKeyCode::Tab, CtModifiers::NONE);
        let message = convert_key(key).unwrap();
        assert_eq!(message.code, KeyCode::Tab);
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
        assert_eq!(convert_key(caps).unwrap().code, KeyCode::CapsLock);

        let mut num = KeyEvent::new(CtKeyCode::NumLock, CtModifiers::NONE);
        num.kind = KeyEventKind::Release;
        let message = convert_key(num).unwrap();
        assert_eq!(message.code, KeyCode::NumLock);
        assert_eq!(message.kind, KeyKind::Release);

        let scroll = KeyEvent::new(CtKeyCode::ScrollLock, CtModifiers::NONE);
        assert_eq!(convert_key(scroll).unwrap().code, KeyCode::ScrollLock);
    }

    #[test]
    fn converts_modifier_keys_with_sides() {
        use crossterm::event::ModifierKeyCode;
        use plurimus_term::ModifierKey;

        let mut key = KeyEvent::new(
            CtKeyCode::Modifier(ModifierKeyCode::RightShift),
            CtModifiers::SHIFT,
        );
        key.kind = KeyEventKind::Release;
        let message = convert_key(key).unwrap();
        assert_eq!(message.code, KeyCode::Modifier(ModifierKey::ShiftRight));
        assert_eq!(message.kind, KeyKind::Release);

        let iso = KeyEvent::new(
            CtKeyCode::Modifier(ModifierKeyCode::IsoLevel3Shift),
            CtModifiers::NONE,
        );
        assert_eq!(convert_key(iso), None);
    }

    #[test]
    fn converts_mouse_kinds_in_cell_coordinates() {
        let down = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 7,
            row: 3,
            modifiers: CtModifiers::NONE,
        };
        let message = convert_mouse(down);
        assert_eq!(
            message.kind,
            MouseKind::Down(plurimus_term::MouseButton::Left)
        );
        assert_eq!(message.position, Position::new(7, 3));

        let sideways = MouseEvent {
            kind: MouseEventKind::ScrollLeft,
            ..down
        };
        assert_eq!(convert_mouse(sideways).kind, MouseKind::ScrollLeft);
    }
}
