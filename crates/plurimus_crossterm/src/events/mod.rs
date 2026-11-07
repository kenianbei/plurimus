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
use bevy_input::ButtonState;
use bevy_input::keyboard::Key;
use crossterm::event::{self, Event, KeyEvent, KeyEventKind, MouseEventKind};

use plurimus_core::ratatui_core::layout::Position;
use plurimus_term::{
    InputCapabilities, KeyModifiers, MouseButton, PasteMessage, TerminalInput, TerminalResized,
};

#[derive(SystemParam)]
pub(crate) struct EventSinks<'w, 's> {
    input: TerminalInput<'w, 's>,
    paste: MessageWriter<'w, PasteMessage>,
    resize: MessageWriter<'w, TerminalResized>,
}

/// A mouse report in the sink's terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalPointer {
    Button(MouseButton, ButtonState),
    Moved,
    /// Wheel notches, `y` up and `x` left positive, as the sink takes them.
    Wheel(i32, i32),
}

/// A key event in the contract's terms, before the sink writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TerminalKey {
    key: Key,
    modifiers: KeyModifiers,
    state: ButtonState,
    repeat: bool,
}

/// How long a drain ending on a key release waits for the press that would
/// make it an autorepeat. The two arrive in the same millisecond, so this is
/// a scheduling margin rather than a guess at the terminal.
const AUTOREPEAT_GRACE: Duration = Duration::from_millis(1);

/// How the terminal encodes a held key, learned from what it sends.
///
/// A conforming terminal reports [`KeyEventKind::Repeat`]. One that honors
/// the protocol's event types without detectable autorepeat sends a release
/// immediately followed by a press instead, and only that one needs the pair
/// recognized or a trailing release waited on - so a terminal is asked to
/// prove it before either cost is paid on its behalf.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepeatEncoding {
    #[default]
    Unknown,
    Native,
    Paired,
}

/// `known`, raised by whatever `batch` proves the terminal reports.
///
/// Learned rather than taken from the kitty query, which proves a terminal
/// speaks the protocol and not that it honors the flags pushed to it. Never
/// lowered: one release proves the terminal sends them.
fn learn_capabilities(batch: &[Event], known: InputCapabilities) -> InputCapabilities {
    let proves = |proof: fn(&KeyEvent) -> bool| {
        batch
            .iter()
            .any(|event| matches!(event, Event::Key(key) if proof(key)))
    };
    known
        .with_key_release(known.key_release || proves(|key| key.kind == KeyEventKind::Release))
        .with_modifier_keys(
            known.modifier_keys || proves(|key| matches!(key.code, event::KeyCode::Modifier(_))),
        )
}

pub(crate) fn pump_events(
    mut sinks: EventSinks,
    mut batch: Local<Vec<Event>>,
    mut encoding: Local<RepeatEncoding>,
) {
    drain(&mut batch, &mut encoding);
    sinks.input.learn(|known| learn_capabilities(&batch, known));
    forward_batch(&mut batch, &mut sinks, &mut encoding);
}

fn drain(batch: &mut Vec<Event>, encoding: &mut RepeatEncoding) {
    batch.clear();
    while event::poll(Duration::ZERO).unwrap_or(false) {
        let Ok(terminal_event) = event::read() else {
            break;
        };
        batch.push(terminal_event);
    }
    // Classified before anything is rewritten, since a coalesced pair is a
    // repeat by the time the walk sees it.
    if batch
        .iter()
        .any(|event| matches!(event, Event::Key(key) if key.kind == KeyEventKind::Repeat))
    {
        *encoding = RepeatEncoding::Native;
    }
    // Only a terminal proven to pair waits: elsewhere a trailing release is
    // a genuine key-up and the poll blocks out the full grace period for
    // nothing.
    if *encoding == RepeatEncoding::Paired
        && matches!(batch.last(), Some(Event::Key(key)) if key.kind == KeyEventKind::Release)
        && event::poll(AUTOREPEAT_GRACE).unwrap_or(false)
        && let Ok(trailing) = event::read()
    {
        batch.push(trailing);
    }
}

fn forward_batch(batch: &mut [Event], sinks: &mut EventSinks, encoding: &mut RepeatEncoding) {
    let mut index = 0;
    while index < batch.len() {
        if *encoding != RepeatEncoding::Native
            && let Some(press_at) = autorepeat_press_at(&batch[index..])
        {
            // The press carries the repeat, so whatever sat between the two
            // still travels by the ordinary path.
            if let Event::Key(press) = &mut batch[index + press_at] {
                press.kind = KeyEventKind::Repeat;
            }
            *encoding = RepeatEncoding::Paired;
            index += 1;
            continue;
        }
        forward_event(&mut batch[index], sinks);
        index += 1;
    }
}

/// Where the press sits that makes `events[0]` an autorepeat's release.
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
fn autorepeat_press_at(events: &[Event]) -> Option<usize> {
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
    (press.kind == KeyEventKind::Press && press.code == release.code).then_some(offset + 1)
}

fn forward_event(terminal_event: &mut Event, sinks: &mut EventSinks) {
    match terminal_event {
        Event::Key(key) => {
            if let Some(key) = convert_key(*key) {
                sinks
                    .input
                    .key(key.key, key.state, key.repeat, key.modifiers);
            }
        }
        Event::Mouse(mouse) => {
            let at = Position::new(mouse.column, mouse.row);
            let modifiers = convert_modifiers(mouse.modifiers);
            match convert_mouse(mouse.kind) {
                TerminalPointer::Button(button, state) => {
                    sinks.input.button(button, state, at, modifiers);
                }
                TerminalPointer::Moved => sinks.input.moved(at, modifiers),
                TerminalPointer::Wheel(x, y) => sinks.input.wheel(at, x, y, modifiers),
            }
        }
        // Taken rather than cloned: a paste is unbounded, and the batch is
        // cleared before it is read again.
        Event::Paste(text) => {
            sinks.paste.write(PasteMessage(std::mem::take(text)));
        }
        Event::FocusGained => sinks.input.focus(true),
        Event::FocusLost => sinks.input.focus(false),
        Event::Resize(cols, rows) => {
            sinks.resize.write(TerminalResized::new(*cols, *rows));
        }
    }
}

fn convert_key(key: KeyEvent) -> Option<TerminalKey> {
    // Terminals report shift-tab as its own key, and not every one sets
    // the shift bit alongside it; the contract carries it as a modified
    // Tab so consumers need only one key to reason about.
    let is_back_tab = key.code == event::KeyCode::BackTab;
    let logical = convert_code(key.code)?;
    let modifiers = convert_modifiers(key.modifiers);
    Some(TerminalKey {
        modifiers: if is_back_tab || is_shifted_letter(&logical) {
            modifiers.with_shift(true)
        } else {
            modifiers
        },
        key: logical,
        state: if key.kind == KeyEventKind::Release {
            ButtonState::Released
        } else {
            ButtonState::Pressed
        },
        repeat: key.kind == KeyEventKind::Repeat,
    })
}

/// Whether `code` is a letter that shift produced.
///
/// The kitty protocol reports the shifted character alongside the key, and
/// crossterm substitutes it and drops the shift bit with it; a legacy
/// terminal reports the same uppercase character and adds the bit. Restoring
/// it is what makes one keystroke mean one thing on both tiers. Only letters
/// can be recovered this way - a shifted symbol carries no trace of the key
/// it came from.
///
/// Uppercase in the same sense crossterm's own legacy path uses, so the two
/// tiers agree beyond ASCII as well.
fn is_shifted_letter(key: &Key) -> bool {
    matches!(key, Key::Character(text) if text.chars().all(char::is_uppercase))
}

/// The F keys a terminal can name, `F1` first.
const FUNCTION_KEYS: [Key; 12] = [
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
];

fn convert_code(code: event::KeyCode) -> Option<Key> {
    Some(match code {
        event::KeyCode::Char(c) => Key::Character(c.encode_utf8(&mut [0; 4]).into()),
        event::KeyCode::Enter => Key::Enter,
        event::KeyCode::Esc => Key::Escape,
        event::KeyCode::Tab | event::KeyCode::BackTab => Key::Tab,
        event::KeyCode::Backspace => Key::Backspace,
        event::KeyCode::Delete => Key::Delete,
        event::KeyCode::Insert => Key::Insert,
        event::KeyCode::Up => Key::ArrowUp,
        event::KeyCode::Down => Key::ArrowDown,
        event::KeyCode::Left => Key::ArrowLeft,
        event::KeyCode::Right => Key::ArrowRight,
        event::KeyCode::Home => Key::Home,
        event::KeyCode::End => Key::End,
        event::KeyCode::PageUp => Key::PageUp,
        event::KeyCode::PageDown => Key::PageDown,
        event::KeyCode::F(n) => FUNCTION_KEYS.get(usize::from(n).checked_sub(1)?)?.clone(),
        event::KeyCode::CapsLock => Key::CapsLock,
        event::KeyCode::NumLock => Key::NumLock,
        event::KeyCode::ScrollLock => Key::ScrollLock,
        event::KeyCode::Modifier(modifier) => convert_modifier_key(modifier)?,
        _ => return None,
    })
}

// The side is dropped: the contract polls modifiers by flag, and a
// modifier key lands on the left one.
const fn convert_modifier_key(modifier: event::ModifierKeyCode) -> Option<Key> {
    use event::ModifierKeyCode as Ct;
    Some(match modifier {
        Ct::LeftShift | Ct::RightShift => Key::Shift,
        Ct::LeftControl | Ct::RightControl => Key::Control,
        Ct::LeftAlt | Ct::RightAlt => Key::Alt,
        Ct::LeftSuper | Ct::RightSuper => Key::Super,
        Ct::LeftHyper | Ct::RightHyper => Key::Hyper,
        Ct::LeftMeta | Ct::RightMeta => Key::Meta,
        Ct::IsoLevel3Shift | Ct::IsoLevel5Shift => return None,
    })
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

// A drag is a move; the button held is `ButtonInput<MouseButton>`'s to know.
const fn convert_mouse(kind: MouseEventKind) -> TerminalPointer {
    match kind {
        MouseEventKind::Down(button) => {
            TerminalPointer::Button(convert_button(button), ButtonState::Pressed)
        }
        MouseEventKind::Up(button) => {
            TerminalPointer::Button(convert_button(button), ButtonState::Released)
        }
        MouseEventKind::Drag(_) | MouseEventKind::Moved => TerminalPointer::Moved,
        MouseEventKind::ScrollUp => TerminalPointer::Wheel(0, 1),
        MouseEventKind::ScrollDown => TerminalPointer::Wheel(0, -1),
        MouseEventKind::ScrollLeft => TerminalPointer::Wheel(1, 0),
        MouseEventKind::ScrollRight => TerminalPointer::Wheel(-1, 0),
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
mod tests;
