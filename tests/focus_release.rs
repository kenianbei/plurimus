//! A terminal reports nothing while unfocused, so the release of a key or
//! button held across a focus loss is one no backend will ever send. What has to be true
//! is an ordering claim only an assembled app can prove: the release covers
//! keys pressed in the losing frame too, since alt-tab's own keys arrive in
//! that batch.
//!
//! With no backend attached the app claims every capability, so these run on
//! the tier that expires nothing - the one where the held-key registry has
//! only this path to be drained by.

use bevy_app::App;
use bevy_input::ButtonInput;
use bevy_input::keyboard::KeyCode;
use plurimus::core::CorePlugin;
use plurimus::term::{MouseButton, TermPlugin};
use plurimus_test::{character, send_focus, send_press_at, write_focus, write_key, write_press_at};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, TermPlugin));
    app
}

fn is_held(app: &App, code: KeyCode) -> bool {
    app.world().resource::<ButtonInput<KeyCode>>().pressed(code)
}

#[test]
fn losing_focus_ends_a_hold_the_terminal_never_will() {
    let mut app = app();
    write_key(&mut app, character('w'));
    app.update();
    assert!(is_held(&app, KeyCode::KeyW));

    send_focus(&mut app, false);
    assert!(
        !is_held(&app, KeyCode::KeyW),
        "the key is still down after the terminal stopped reporting it"
    );
}

// The press and the focus loss in one frame is the alt-tab a key triggers:
// the release must still win, since it is written after the press is
// recorded - and in that same frame.
#[test]
fn a_key_pressed_in_the_losing_frame_does_not_survive_it() {
    let mut app = app();
    write_key(&mut app, character('w'));
    write_focus(&mut app, false);
    app.update();

    assert!(!is_held(&app, KeyCode::KeyW));
}

fn is_button_held(app: &App) -> bool {
    app.world()
        .resource::<ButtonInput<MouseButton>>()
        .pressed(MouseButton::Left)
}

#[test]
fn losing_focus_ends_a_button_hold_too() {
    let mut app = app();
    send_press_at(&mut app, 2, 2);
    assert!(is_button_held(&app));

    send_focus(&mut app, false);
    assert!(!is_button_held(&app));
}

#[test]
fn a_button_pressed_in_the_losing_frame_does_not_survive_it() {
    let mut app = app();
    write_press_at(&mut app, 2, 2);
    write_focus(&mut app, false);
    app.update();

    assert!(!is_button_held(&app));
}
