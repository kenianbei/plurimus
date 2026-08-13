//! Input-injection helpers.
//!
//! Two families: `write_*` only queues the message, so several land in one
//! frame; `press_*`/`send_*` also tick the app.

use bevy_app::App;
use plurimus_core::ratatui_core::layout::Position;
use plurimus_term::{
    FocusMessage, KeyCode, KeyKind, KeyMessage, KeyModifiers, ModifierKey, MouseButton, MouseKind,
    MouseMessage,
};

/// Queues a key press with no modifiers.
pub fn write_key(app: &mut App, code: KeyCode) {
    write_key_kind(app, code, KeyModifiers::default(), KeyKind::Press);
}

fn write_key_kind(app: &mut App, code: KeyCode, modifiers: KeyModifiers, kind: KeyKind) {
    app.world_mut()
        .write_message(KeyMessage::new(code, modifiers, kind));
}

fn press_key_kind(app: &mut App, code: KeyCode, modifiers: KeyModifiers, kind: KeyKind) {
    write_key_kind(app, code, modifiers, kind);
    app.update();
}

/// Queues a mouse message at `(x, y)` with no modifiers.
pub fn write_mouse(app: &mut App, kind: MouseKind, x: u16, y: u16) {
    app.world_mut().write_message(MouseMessage::new(
        kind,
        Position::new(x, y),
        KeyModifiers::default(),
    ));
}

/// Queues a key press with no modifiers, then ticks the app.
pub fn press_key(app: &mut App, code: KeyCode) {
    write_key(app, code);
    app.update();
}

/// Queues an autorepeat of `code` with no modifiers, then ticks the app.
///
/// A held key, as a terminal reports it on the kitty tier: widgets repeat
/// movement on one but must not re-activate.
pub fn repeat_key(app: &mut App, code: KeyCode) {
    press_key_kind(app, code, KeyModifiers::default(), KeyKind::Repeat);
}

/// Queues a key press carrying `modifiers`, then ticks the app.
pub fn press_key_with(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    press_key_kind(app, code, modifiers, KeyKind::Press);
}

/// A whole chord: presses `modifier`, presses and releases `code` carrying
/// it, then releases `modifier`, ticking after each.
///
/// A test app's capabilities claim the kitty tier, where `ButtonInput`
/// follows real modifier key events rather than the bits a message carries,
/// and where nothing expires a hold - so bits alone never mark the modifier
/// held, and a chord that skips its releases reaches every later key. The
/// last release carries none, reporting the state the event leaves behind,
/// which is also what the legacy tier diffs to derive its own.
pub fn press_chord(app: &mut App, modifier: ModifierKey, code: KeyCode) {
    let modifier_code = KeyCode::Modifier(modifier);
    let held = KeyModifiers::from(modifier);
    let none = KeyModifiers::default();
    press_key_kind(app, modifier_code, held, KeyKind::Press);
    press_key_kind(app, code, held, KeyKind::Press);
    press_key_kind(app, code, held, KeyKind::Release);
    press_key_kind(app, modifier_code, none, KeyKind::Release);
}

/// Queues a terminal focus change, `gained` for focus arriving.
pub fn write_focus(app: &mut App, gained: bool) {
    app.world_mut().write_message(FocusMessage::new(gained));
}

/// Queues a terminal focus change, then ticks the app.
///
/// Losing focus releases every held key, since a terminal reports nothing
/// while unfocused.
pub fn send_focus(app: &mut App, gained: bool) {
    write_focus(app, gained);
    app.update();
}

/// Queues a mouse message at `(x, y)`, then ticks the app.
pub fn send_mouse(app: &mut App, kind: MouseKind, x: u16, y: u16) {
    write_mouse(app, kind, x, y);
    app.update();
}

/// A full left click at `(x, y)`: moved, pressed, released, ticking
/// after each so hover resolves before the press lands.
pub fn click(app: &mut App, x: u16, y: u16) {
    send_mouse(app, MouseKind::Moved, x, y);
    send_mouse(app, MouseKind::Down(MouseButton::Left), x, y);
    send_mouse(app, MouseKind::Up(MouseButton::Left), x, y);
}
