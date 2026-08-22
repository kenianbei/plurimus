//! Polled input state derived from the message stream.
//!
//! The message stream says what happened this frame; a game loop usually
//! wants to know what is held right now. Both views are kept from the same
//! source here, so a widget can read discrete presses while a movement system
//! polls [`ButtonInput`](bevy_input::ButtonInput) for the same keys.
//!
//! The two views are keyed differently, and deliberately. A message reports
//! the character the terminal produced, while held state is keyed on
//! [`KeyCode::held_as`] - so a key is held as `Char('w')` whatever case it
//! was struck or released in, and `pressed(Char('W'))` is never true. Poll
//! for the identity, not for what was typed.

use bevy_ecs::prelude::{MessageReader, ResMut};
use bevy_ecs::schedule::SystemSet;
use bevy_input::ButtonInput;

use super::{CursorCell, KeyCode, KeyKind, KeyMessage, MouseButton, MouseKind, MouseMessage};

/// Ordered phases of input handling in `PreUpdate`.
#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputSystems {
    /// Backends drain their event source into messages.
    Pump,
    /// State is derived (button state, cursor, synthesis) from messages.
    Update,
}

pub(crate) fn update_button_input(
    mut keys: MessageReader<KeyMessage>,
    mut mouse: MessageReader<MouseMessage>,
    mut key_state: ResMut<ButtonInput<KeyCode>>,
    mut mouse_state: ResMut<ButtonInput<MouseButton>>,
) {
    key_state.clear();
    mouse_state.clear();
    for key in keys.read() {
        match key.kind {
            // A repeat means the key is down, so it presses like one, and a
            // hold whose press never arrived is put back by the next one.
            // `press` sets `just_pressed` only on a transition.
            KeyKind::Press | KeyKind::Repeat => key_state.press(key.code.held_as()),
            KeyKind::Release => key_state.release(key.code.held_as()),
        }
    }
    for event in mouse.read() {
        match event.kind {
            MouseKind::Down(button) => mouse_state.press(button),
            MouseKind::Up(button) => mouse_state.release(button),
            _ => {}
        }
    }
}

pub(crate) fn track_cursor_cell(
    mut mouse: MessageReader<MouseMessage>,
    mut cursor: ResMut<CursorCell>,
) {
    if let Some(event) = mouse.read().last() {
        cursor.0 = Some(event.position);
    }
}

#[cfg(test)]
mod tests {
    use bevy_app::App;
    use bevy_input::ButtonInput;
    use plurimus_core::ratatui_core::layout::Position;

    use crate::{
        CursorCell, KeyCode, KeyKind, KeyMessage, KeyModifiers, MouseButton, MouseKind,
        MouseMessage, TermPlugin,
    };

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((plurimus_core::CorePlugin, TermPlugin));
        app
    }

    fn key(kind: KeyKind) -> KeyMessage {
        typed(KeyCode::Char('w'), kind)
    }

    fn typed(code: KeyCode, kind: KeyKind) -> KeyMessage {
        KeyMessage::new(code, KeyModifiers::none(), kind)
    }

    #[test]
    fn press_and_release_drive_button_state() {
        let mut app = app();
        app.world_mut().write_message(key(KeyKind::Press));
        app.update();
        let state = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(state.pressed(KeyCode::Char('w')));
        assert!(state.just_pressed(KeyCode::Char('w')));

        app.world_mut().write_message(key(KeyKind::Repeat));
        app.update();
        let state = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(state.pressed(KeyCode::Char('w')));
        assert!(!state.just_pressed(KeyCode::Char('w')));

        app.world_mut().write_message(key(KeyKind::Release));
        app.update();
        let state = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(!state.pressed(KeyCode::Char('w')));
        assert!(state.just_released(KeyCode::Char('w')));
    }

    // A backend can only pair an autorepeat's release with its press inside
    // one drained batch, so a pair split across two leaks the release.
    #[test]
    fn a_repeat_restores_a_hold_a_leaked_release_ended() {
        let mut app = app();
        app.world_mut().write_message(key(KeyKind::Press));
        app.update();
        app.world_mut().write_message(key(KeyKind::Release));
        app.update();
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::Char('w'))
        );

        app.world_mut().write_message(key(KeyKind::Repeat));
        app.update();
        let state = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(state.pressed(KeyCode::Char('w')));
        assert!(state.just_pressed(KeyCode::Char('w')));
    }

    // The gesture a raw-byte probe caught: hold `w`, press shift, release
    // `w`. Kitty substitutes the shifted character, so the release arrives
    // as `W`.
    #[test]
    fn a_hold_shifted_before_it_ends_is_still_released() {
        let mut app = app();
        app.world_mut().write_message(key(KeyKind::Press));
        app.update();

        app.world_mut()
            .write_message(typed(KeyCode::Char('W'), KeyKind::Release));
        app.update();

        let state = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(!state.pressed(KeyCode::Char('w')));
        assert!(state.just_released(KeyCode::Char('w')));
    }

    #[test]
    fn shifting_a_hold_does_not_press_a_second_key() {
        let mut app = app();
        app.world_mut().write_message(key(KeyKind::Press));
        app.update();

        app.world_mut()
            .write_message(typed(KeyCode::Char('W'), KeyKind::Repeat));
        app.update();

        let state = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(state.pressed(KeyCode::Char('w')), "one hold, uninterrupted");
        assert!(!state.just_pressed(KeyCode::Char('w')));
        assert!(
            !state.pressed(KeyCode::Char('W')),
            "the case a key was struck in is the message's, not the state's"
        );
    }

    #[test]
    fn mouse_state_and_cursor_track_messages() {
        let mut app = app();
        app.world_mut().write_message(MouseMessage {
            kind: MouseKind::Down(MouseButton::Left),
            position: Position::new(7, 3),
            modifiers: KeyModifiers::default(),
        });
        app.update();

        let state = app.world().resource::<ButtonInput<MouseButton>>();
        assert!(state.pressed(MouseButton::Left));
        assert_eq!(
            app.world().resource::<CursorCell>().0,
            Some(Position::new(7, 3))
        );
    }
}
