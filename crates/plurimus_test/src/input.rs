//! Input-injection helpers.
//!
//! Two families: `write_*` only queues the message, so several land in one
//! frame; `press_*`/`send_*` also tick the app.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::Position;
use plurimus_term::{
    FocusMessage, KeyCode, KeyKind, KeyMessage, KeyModifiers, ModifierKey, MouseButton, MouseKind,
    MouseMessage, PasteMessage,
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

fn press_and_release(app: &mut App, code: KeyCode, modifiers: KeyModifiers, kind: KeyKind) {
    press_key_kind(app, code, modifiers, kind);
    press_key_kind(app, code, modifiers, KeyKind::Release);
}

/// Queues a mouse message at `(x, y)` with no modifiers.
pub fn write_mouse(app: &mut App, kind: MouseKind, x: u16, y: u16) {
    app.world_mut().write_message(MouseMessage::new(
        kind,
        Position::new(x, y),
        KeyModifiers::default(),
    ));
}

/// Presses and releases `code` with no modifiers, ticking after each.
pub fn press_key(app: &mut App, code: KeyCode) {
    press_and_release(app, code, KeyModifiers::default(), KeyKind::Press);
}

/// An autorepeat of `code` with no modifiers, then its release, ticking
/// after each.
///
/// A held key, as a terminal reports it on the kitty tier: widgets repeat
/// movement on one but must not re-activate.
pub fn repeat_key(app: &mut App, code: KeyCode) {
    press_and_release(app, code, KeyModifiers::default(), KeyKind::Repeat);
}

/// Presses and releases `code` carrying `modifiers`, ticking after each.
pub fn press_key_with(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    press_and_release(app, code, modifiers, KeyKind::Press);
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
    press_and_release(app, code, held, KeyKind::Press);
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

/// Queues a move to `(x, y)` and a left press there, landing in one frame.
pub fn write_press_at(app: &mut App, x: u16, y: u16) {
    write_mouse(app, MouseKind::Moved, x, y);
    write_mouse(app, MouseKind::Down(MouseButton::Left), x, y);
}

/// Moves to `(x, y)` and presses left there, ticking after each so hover
/// resolves before the press lands.
pub fn press_at(app: &mut App, x: u16, y: u16) {
    send_mouse(app, MouseKind::Moved, x, y);
    send_mouse(app, MouseKind::Down(MouseButton::Left), x, y);
}

/// Releases left at `(x, y)`, then ticks the app.
pub fn release_at(app: &mut App, x: u16, y: u16) {
    send_mouse(app, MouseKind::Up(MouseButton::Left), x, y);
}

/// A full left click at `(x, y)`: [`press_at`], then [`release_at`].
pub fn click(app: &mut App, x: u16, y: u16) {
    press_at(app, x, y);
    release_at(app, x, y);
}

/// Queues a bracketed paste of `text`.
pub fn write_paste(app: &mut App, text: &str) {
    app.world_mut().write_message(PasteMessage(text.into()));
}

/// Queues a bracketed paste of `text`, then ticks the app.
pub fn send_paste(app: &mut App, text: &str) {
    write_paste(app, text);
    app.update();
}

/// Gives `entity` input focus, without ticking.
///
/// A set before the first frame survives it: `bevy_input_focus` hands the
/// window focus at startup only when nothing holds it.
pub fn set_focus(app: &mut App, entity: Entity) {
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(entity, FocusCause::Navigated);
}

#[cfg(test)]
mod tests {
    use bevy_app::App;
    use plurimus_core::CorePlugin;
    use plurimus_term::{ButtonInput, KeyCode, KeyModifiers, TermPlugin};

    use super::{press_key, press_key_with, repeat_key};

    fn is_held(app: &App, code: KeyCode) -> bool {
        app.world().resource::<ButtonInput<KeyCode>>().pressed(code)
    }

    #[test]
    fn an_injected_keystroke_leaves_nothing_held() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, TermPlugin));

        press_key(&mut app, KeyCode::Char('a'));
        assert!(!is_held(&app, KeyCode::Char('a')), "press_key");
        repeat_key(&mut app, KeyCode::Down);
        assert!(!is_held(&app, KeyCode::Down), "repeat_key");
        press_key_with(
            &mut app,
            KeyCode::Char('b'),
            KeyModifiers::default().with_ctrl(true),
        );
        assert!(!is_held(&app, KeyCode::Char('b')), "press_key_with");
    }
}
