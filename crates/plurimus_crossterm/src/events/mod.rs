//! Translates crossterm events into plurimus input messages.
//!
//! Crossterm reports what the terminal sent; the input contract describes
//! what happened. The gap between those is what this file closes - mouse
//! positions become cell coordinates, terminal quirks are normalized into one
//! shape consumers can reason about, and a resize becomes a
//! [`TerminalResized`](plurimus_term::TerminalResized) rather than an event
//! anyone polls for.

use std::time::Duration;

use bevy_ecs::prelude::{DetectChangesMut, Local, MessageWriter, ResMut};
use bevy_ecs::system::SystemParam;
use crossterm::event::{self, Event, KeyEvent, KeyEventKind, MouseEvent, MouseEventKind};

use plurimus_core::ratatui_core::layout::Position;
use plurimus_term::{
    FocusMessage, InputCapabilities, KeyCode, KeyKind, KeyMessage, KeyModifiers, ModifierKey,
    MouseButton, MouseKind, MouseMessage, PasteMessage, TerminalResized,
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
    mut capabilities: ResMut<InputCapabilities>,
    mut batch: Local<Vec<Event>>,
    mut encoding: Local<RepeatEncoding>,
) {
    drain(&mut batch, &mut encoding);
    // Learned here, so this frame's release and modifier synthesis see it.
    capabilities.set_if_neq(learn_capabilities(&batch, *capabilities));
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
            if let Some(message) = convert_key(*key) {
                sinks.keys.write(message);
            }
        }
        Event::Mouse(mouse) => {
            sinks.mouse.write(convert_mouse(*mouse));
        }
        // Taken rather than cloned: a paste is unbounded, and the batch is
        // cleared before it is read again.
        Event::Paste(text) => {
            sinks.paste.write(PasteMessage(std::mem::take(text)));
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
    let code = convert_code(key.code)?;
    let modifiers = convert_modifiers(key.modifiers);
    Some(KeyMessage::new(
        code,
        if is_back_tab || is_shifted_letter(code) {
            modifiers.with_shift(true)
        } else {
            modifiers
        },
        convert_kind(key.kind),
    ))
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
/// tiers agree beyond ASCII as well - and in the same sense
/// [`KeyCode::held_as`] folds by, which is what lets a hold shifted midway
/// still be released by the press it ends.
const fn is_shifted_letter(code: KeyCode) -> bool {
    matches!(code, KeyCode::Char(c) if c.is_uppercase())
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
mod tests;
