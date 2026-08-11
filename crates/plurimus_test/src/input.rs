//! Input-injection helpers.
//!
//! Two families: `write_*` only queues the message, so several land in one
//! frame; `press_*`/`send_*` also tick the app.

use bevy_app::App;
use plurimus_core::ratatui_core::layout::Position;
use plurimus_term::{
    KeyCode, KeyKind, KeyMessage, KeyModifiers, ModifierKey, MouseButton, MouseKind, MouseMessage,
};

/// Queues a key press with no modifiers.
pub fn write_key(app: &mut App, code: KeyCode) {
    write_key_kind(app, code, KeyModifiers::default(), KeyKind::Press);
}

fn write_key_kind(app: &mut App, code: KeyCode, modifiers: KeyModifiers, kind: KeyKind) {
    app.world_mut().write_message(KeyMessage {
        code,
        modifiers,
        kind,
    });
}

fn press_key_kind(app: &mut App, code: KeyCode, modifiers: KeyModifiers, kind: KeyKind) {
    write_key_kind(app, code, modifiers, kind);
    app.update();
}

/// Queues a mouse message at `(x, y)` with no modifiers.
pub fn write_mouse(app: &mut App, kind: MouseKind, x: u16, y: u16) {
    app.world_mut().write_message(MouseMessage {
        kind,
        position: Position::new(x, y),
        modifiers: KeyModifiers::default(),
    });
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
/// Both halves matter on the kitty tier
/// ([`plurimus_term::InputCapabilities::modifier_keys`]), which is what a
/// test app's default capabilities claim. Without the leading press,
/// `ButtonInput<KeyCode>` never sees the modifier held; without the
/// releases nothing ever ends the hold, since those same capabilities turn
/// release synthesis off, and the modifier would reach every later injected
/// key. The trailing release carries no bits, because a terminal reports the
/// state left after an event and the legacy tier derives its own modifier
/// release by diffing successive bitfields.
pub fn press_chord(app: &mut App, modifier: ModifierKey, code: KeyCode) {
    let modifiers = KeyModifiers::from(modifier);
    let modifier_code = KeyCode::Modifier(modifier);
    press_key_kind(app, modifier_code, modifiers, KeyKind::Press);
    press_key_kind(app, code, modifiers, KeyKind::Press);
    press_key_kind(app, code, modifiers, KeyKind::Release);
    press_key_kind(
        app,
        modifier_code,
        KeyModifiers::default(),
        KeyKind::Release,
    );
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
