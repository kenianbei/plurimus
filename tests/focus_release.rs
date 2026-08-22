//! A terminal reports nothing while unfocused, so the release of a key held
//! across a focus loss is one no backend will ever send. What has to be true
//! is an ordering claim only an assembled app can prove: the release covers
//! keys pressed in the losing frame too, since alt-tab's own keys arrive in
//! that batch.

use bevy_app::App;
use bevy_input::ButtonInput;
use plurimus::core::CorePlugin;
use plurimus::term::{KeyCode, TermPlugin};
use plurimus_test::{press_key, send_focus, write_focus, write_key};

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
    press_key(&mut app, KeyCode::Char('w'));
    assert!(is_held(&app, KeyCode::Char('w')));

    send_focus(&mut app, false);
    assert!(
        !is_held(&app, KeyCode::Char('w')),
        "the key is still down after the terminal stopped reporting it"
    );
}

// The press and the focus loss in one frame is the alt-tab a key triggers:
// the release must still win, since it is written after the press is
// recorded - and in the same frame, polled state being derived from the
// releases rather than read by whoever writes them.
#[test]
fn a_key_pressed_in_the_losing_frame_does_not_survive_it() {
    let mut app = app();
    write_key(&mut app, KeyCode::Char('w'));
    write_focus(&mut app, false);
    app.update();

    assert!(!is_held(&app, KeyCode::Char('w')));
}
