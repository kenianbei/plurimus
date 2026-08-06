//! Input-injection helpers.
//!
//! Two families: `write_*` only queues the message, so several land in one
//! frame; `press_*`/`send_*` also tick the app.

use bevy_app::App;
use plurimus_core::ratatui_core::layout::Position;
use plurimus_input::{
    KeyCode, KeyKind, KeyMessage, KeyModifiers, ModifierKey, MouseButton, MouseKind, MouseMessage,
};

/// Queues a key press with no modifiers.
pub fn write_key(app: &mut App, code: KeyCode) {
    write_key_with(app, code, KeyModifiers::default());
}

fn write_key_with(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    app.world_mut().write_message(KeyMessage {
        code,
        modifiers,
        kind: KeyKind::Press,
    });
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

/// Queues a key press carrying `modifiers`, then ticks the app.
pub fn press_key_with(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    write_key_with(app, code, modifiers);
    app.update();
}

/// Presses `modifier`, then `code` carrying it, ticking after each.
///
/// Setting the modifier bits alone is not enough on the kitty tier
/// ([`plurimus_input::InputCapabilities::modifier_keys`]), which reports
/// real modifier key events: without the leading press,
/// `ButtonInput<KeyCode>` never sees the modifier as held.
pub fn press_chord(app: &mut App, modifier: ModifierKey, code: KeyCode) {
    let modifiers = KeyModifiers::from(modifier);
    press_key_with(app, KeyCode::Modifier(modifier), modifiers);
    press_key_with(app, code, modifiers);
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
