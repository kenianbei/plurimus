use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{In, With};
use bevy_input::ButtonState;
use bevy_input::keyboard::Key;
use bevy_input_focus::{FocusCause, InputFocus};
use bevy_window::PrimaryWindow;
use plurimus_core::ratatui_core::layout::Position;
use plurimus_term::{KeyModifiers, MouseButton, PasteMessage, TerminalInput, VirtualWindow};

/// The key a character is typed as.
#[must_use]
pub fn character(c: char) -> Key {
    Key::Character(c.encode_utf8(&mut [0; 4]).into())
}

/// What a backend reports, in the sink's terms.
#[derive(Debug, Clone)]
enum Report {
    Key(Key, ButtonState, bool),
    Focus(bool),
    Button(MouseButton, ButtonState, Position),
    Moved(Position),
    Wheel(i32, i32, Position),
}

// `TermPlugin` spawns the window on the first frame; a test writing input
// before any frame needs it now.
fn window(app: &mut App) {
    let world = app.world_mut();
    if world
        .query_filtered::<(), With<PrimaryWindow>>()
        .single(world)
        .is_err()
    {
        world.spawn(VirtualWindow);
    }
}

fn inject(In((report, modifiers)): In<(Report, KeyModifiers)>, mut input: TerminalInput) {
    match report {
        Report::Key(key, state, repeat) => input.key(key, state, repeat, modifiers),
        Report::Focus(focused) => input.focus(focused),
        Report::Button(button, state, at) => input.button(button, state, at, modifiers),
        Report::Moved(at) => input.moved(at, modifiers),
        Report::Wheel(x, y, at) => input.wheel(at, x, y, modifiers),
    }
}

/// Queues `report` exactly as a backend would write it, through one cached
/// sink so its own state - the cell the pointer was last placed in -
/// carries across calls.
fn report(app: &mut App, report: Report, modifiers: KeyModifiers) {
    window(app);
    app.world_mut()
        .run_system_cached_with(inject, (report, modifiers))
        .expect("the sink runs against TermPlugin's resources");
}

fn write_key_state(app: &mut App, key: Key, state: ButtonState, repeat: bool) {
    report(
        app,
        Report::Key(key, state, repeat),
        KeyModifiers::default(),
    );
}

/// Queues a key press with no modifiers.
pub fn write_key(app: &mut App, key: Key) {
    write_key_state(app, key, ButtonState::Pressed, false);
}

/// Queues an autorepeat of `key` with no modifiers.
pub fn write_repeat(app: &mut App, key: Key) {
    write_key_state(app, key, ButtonState::Pressed, true);
}

fn press_key_state(app: &mut App, key: Key, state: ButtonState, repeat: bool) {
    write_key_state(app, key, state, repeat);
    app.update();
}

fn press_and_release(app: &mut App, key: Key, repeat: bool) {
    press_key_state(app, key.clone(), ButtonState::Pressed, repeat);
    press_key_state(app, key, ButtonState::Released, false);
}

/// The key of every modifier `held` names; the sink lands each on its left
/// side.
fn modifier_keys(held: KeyModifiers) -> impl Iterator<Item = Key> {
    [
        (held.ctrl, Key::Control),
        (held.alt, Key::Alt),
        (held.shift, Key::Shift),
        (held.super_key, Key::Super),
        (held.hyper, Key::Hyper),
        (held.meta, Key::Meta),
    ]
    .into_iter()
    .filter_map(|(is_held, key)| is_held.then_some(key))
}

/// Presses and releases `key` with no modifiers, ticking after each.
pub fn press_key(app: &mut App, key: Key) {
    press_and_release(app, key, false);
}

/// An autorepeat of `key` with no modifiers, then its release, ticking
/// after each.
///
/// A held key, as a terminal reports it on the kitty tier: widgets repeat
/// movement on one but must not re-activate.
pub fn repeat_key(app: &mut App, key: Key) {
    press_and_release(app, key, true);
}

/// A whole chord: presses the key of every modifier in `modifiers`, presses
/// and releases `key`, then releases the modifiers, ticking after each.
///
/// A test app's capabilities claim the kitty tier, where modifier keys
/// arrive as keys of their own and nothing expires a hold - so the modifier
/// is held by its own press and release, and a chord that skipped them would
/// reach every later key.
pub fn press_key_with(app: &mut App, key: Key, modifiers: KeyModifiers) {
    for modifier in modifier_keys(modifiers) {
        press_key_state(app, modifier, ButtonState::Pressed, false);
    }
    press_and_release(app, key, false);
    for modifier in modifier_keys(modifiers) {
        press_key_state(app, modifier, ButtonState::Released, false);
    }
}

/// Queues a terminal focus change, `focused` for focus arriving.
pub fn write_focus(app: &mut App, focused: bool) {
    report(app, Report::Focus(focused), KeyModifiers::default());
}

/// Queues a terminal focus change, then ticks the app.
///
/// Losing focus releases every held key, since a terminal reports nothing
/// while unfocused.
pub fn send_focus(app: &mut App, focused: bool) {
    write_focus(app, focused);
    app.update();
}

fn write_button(app: &mut App, state: ButtonState, x: u16, y: u16, modifiers: KeyModifiers) {
    let button = Report::Button(MouseButton::Left, state, Position::new(x, y));
    report(app, button, modifiers);
}

/// Queues a pointer move to `(x, y)`, a button held or not.
pub fn write_move_to(app: &mut App, x: u16, y: u16) {
    report(
        app,
        Report::Moved(Position::new(x, y)),
        KeyModifiers::default(),
    );
}

/// Moves the pointer to `(x, y)`, then ticks the app.
pub fn move_to(app: &mut App, x: u16, y: u16) {
    write_move_to(app, x, y);
    app.update();
}

/// Queues a left press at `(x, y)` carrying `modifiers`, placing the
/// pointer there first as a terminal's click does.
pub fn write_press_at_with(app: &mut App, x: u16, y: u16, modifiers: KeyModifiers) {
    write_button(app, ButtonState::Pressed, x, y, modifiers);
}

/// Queues a left press at `(x, y)`, placing the pointer there first.
pub fn write_press_at(app: &mut App, x: u16, y: u16) {
    write_press_at_with(app, x, y, KeyModifiers::default());
}

/// Queues a left press at `(x, y)`, then ticks the app: the move and the
/// press land in one frame.
pub fn send_press_at(app: &mut App, x: u16, y: u16) {
    write_press_at(app, x, y);
    app.update();
}

/// Moves to `(x, y)` and presses left there, ticking after each so hover
/// resolves before the press lands.
pub fn press_at(app: &mut App, x: u16, y: u16) {
    move_to(app, x, y);
    send_press_at(app, x, y);
}

/// Queues a left release at `(x, y)`.
pub fn write_release_at(app: &mut App, x: u16, y: u16) {
    write_button(app, ButtonState::Released, x, y, KeyModifiers::default());
}

/// Releases left at `(x, y)`, then ticks the app.
pub fn release_at(app: &mut App, x: u16, y: u16) {
    write_release_at(app, x, y);
    app.update();
}

/// A full left click at `(x, y)`: [`press_at`], then [`release_at`].
pub fn click(app: &mut App, x: u16, y: u16) {
    press_at(app, x, y);
    release_at(app, x, y);
}

/// Queues `rows` wheel notches at `(x, y)`, positive scrolling down.
pub fn write_scroll_at(app: &mut App, x: u16, y: u16, rows: i32) {
    for _ in 0..rows.unsigned_abs() {
        let notch = Report::Wheel(0, -rows.signum(), Position::new(x, y));
        report(app, notch, KeyModifiers::default());
    }
}

/// Turns the wheel `rows` notches at `(x, y)`, positive scrolling down,
/// then ticks the app.
pub fn scroll_at(app: &mut App, x: u16, y: u16, rows: i32) {
    write_scroll_at(app, x, y, rows);
    app.update();
}

/// Turns the wheel `columns` notches sideways at `(x, y)`, positive
/// scrolling right, then ticks the app.
pub fn scroll_sideways(app: &mut App, x: u16, y: u16, columns: i32) {
    for _ in 0..columns.unsigned_abs() {
        let notch = Report::Wheel(-columns.signum(), 0, Position::new(x, y));
        report(app, notch, KeyModifiers::default());
    }
    app.update();
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
    use bevy_input::keyboard::{Key, KeyCode};
    use plurimus_core::CorePlugin;
    use plurimus_term::{ButtonInput, KeyModifiers, TermPlugin};

    use super::{character, press_key, press_key_with, repeat_key};

    fn is_held(app: &App, code: KeyCode) -> bool {
        app.world().resource::<ButtonInput<KeyCode>>().pressed(code)
    }

    #[test]
    fn an_injected_keystroke_leaves_nothing_held() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, TermPlugin));

        press_key(&mut app, character('a'));
        assert!(!is_held(&app, KeyCode::KeyA), "press_key");
        repeat_key(&mut app, Key::ArrowDown);
        assert!(!is_held(&app, KeyCode::ArrowDown), "repeat_key");
        press_key_with(
            &mut app,
            character('b'),
            KeyModifiers::default().with_ctrl(true),
        );
        assert!(!is_held(&app, KeyCode::KeyB), "press_key_with");
        assert!(
            !is_held(&app, KeyCode::ControlLeft),
            "press_key_with's modifier"
        );
    }
}
