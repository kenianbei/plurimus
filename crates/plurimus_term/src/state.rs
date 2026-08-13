//! Polled input state derived from the message stream.
//!
//! The message stream says what happened this frame; a game loop usually
//! wants to know what is held right now. Both views are kept from the same
//! source here, so a widget can read discrete presses while a movement system
//! polls [`ButtonInput`](bevy_input::ButtonInput) for the same keys.

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
            // A repeat presses too, so a hold recovers if its press was
            // missed. `press` sets `just_pressed` only on a transition, so
            // a key already down costs a lookup and no edge.
            KeyKind::Press | KeyKind::Repeat => key_state.press(key.code),
            KeyKind::Release => key_state.release(key.code),
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

    fn key(kind: KeyKind) -> KeyMessage {
        KeyMessage {
            code: KeyCode::Char('w'),
            modifiers: KeyModifiers::default(),
            kind,
        }
    }

    #[test]
    fn press_and_release_drive_button_state() {
        let mut app = App::new();
        app.add_plugins((plurimus_core::CorePlugin, TermPlugin));

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
    // one drained batch, so a pair split across two leaks the release. The
    // next repeat is what puts the hold back.
    #[test]
    fn a_repeat_restores_a_hold_a_leaked_release_ended() {
        let mut app = App::new();
        app.add_plugins((plurimus_core::CorePlugin, TermPlugin));

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

    #[test]
    fn mouse_state_and_cursor_track_messages() {
        let mut app = App::new();
        app.add_plugins((plurimus_core::CorePlugin, TermPlugin));

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
