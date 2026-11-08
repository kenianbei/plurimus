use bevy_app::{App, First};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::{IntoScheduleConfigs, With};
use bevy_ecs::system::{Local, Single};
use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy_input::mouse::{MouseButton, MouseButtonInput};
use bevy_input::{ButtonInput, ButtonState};
use bevy_window::{CursorMoved, PrimaryWindow, WindowEvent};
use plurimus_core::ratatui_core::layout::Position;

use super::{PointerModifiers, TerminalInput, write_input};
use crate::{InputCapabilities, InputSystems, KeyModifiers, TermPlugin};

fn app(modifier_keys: bool) -> App {
    let mut app = App::new();
    app.add_plugins((plurimus_core::CorePlugin, TermPlugin));
    app.insert_resource(InputCapabilities::default().with_modifier_keys(modifier_keys));
    app
}

fn written<T: bevy_ecs::message::Message + Clone>(app: &mut App) -> Vec<T> {
    app.world_mut()
        .resource_mut::<Messages<T>>()
        .drain()
        .collect()
}

fn window(app: &mut App) -> Entity {
    app.world_mut()
        .run_system_cached(|window: Single<Entity, With<PrimaryWindow>>| *window)
        .unwrap()
}

fn pump_shifted_tab(mut input: TerminalInput) {
    input.key(
        Key::Tab,
        ButtonState::Pressed,
        false,
        KeyModifiers::none().with_shift(true),
    );
}

fn pump_shifted_click(mut input: TerminalInput) {
    let shift = KeyModifiers::none().with_shift(true);
    input.button(
        MouseButton::Left,
        ButtonState::Pressed,
        Position::new(3, 1),
        shift,
    );
}

#[test]
fn a_key_names_the_window_its_key_and_its_text() {
    let mut app = app(true);
    app.add_systems(
        First,
        (|mut input: TerminalInput| {
            input.key(
                Key::Character("w".into()),
                ButtonState::Pressed,
                true,
                KeyModifiers::none(),
            );
        })
        .in_set(InputSystems::Pump),
    );
    app.update();

    let keys: Vec<KeyboardInput> = written(&mut app);
    let window = window(&mut app);
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].key_code, KeyCode::KeyW);
    assert_eq!(keys[0].text, Some("w".into()));
    assert!(keys[0].repeat);
    assert_eq!(keys[0].window, window);
    assert!(
        written::<WindowEvent>(&mut app)
            .iter()
            .any(|event| matches!(event, WindowEvent::KeyboardInput(_))),
        "the ordered stream carries it too"
    );
}

#[test]
fn bits_become_modifier_keys_only_where_the_terminal_sends_none() {
    let mut legacy = app(false);
    legacy.add_systems(First, pump_shifted_tab.in_set(InputSystems::Pump));
    legacy.update();
    let keys: Vec<KeyboardInput> = written(&mut legacy);
    assert_eq!(keys.len(), 2, "{keys:?}");
    assert_eq!(keys[0].key_code, KeyCode::ShiftLeft);
    assert_eq!(keys[0].state, ButtonState::Pressed);
    assert_eq!(keys[1].key_code, KeyCode::Tab);

    let mut kitty = app(true);
    kitty.add_systems(First, pump_shifted_tab.in_set(InputSystems::Pump));
    kitty.update();
    assert_eq!(written::<KeyboardInput>(&mut kitty).len(), 1);
}

fn pump_shift_across_a_focus_loss(mut input: TerminalInput, mut frame: Local<u8>) {
    let shift = KeyModifiers::none().with_shift(true);
    match *frame {
        0 => input.key(Key::Tab, ButtonState::Pressed, false, shift),
        1 => input.focus(false),
        2 => {
            input.focus(true);
            input.key(Key::Tab, ButtonState::Pressed, false, shift);
        }
        _ => {}
    }
    *frame += 1;
}

fn shift_keys(app: &mut App) -> Vec<ButtonState> {
    written::<KeyboardInput>(app)
        .into_iter()
        .filter(|input| input.key_code == KeyCode::ShiftLeft)
        .map(|input| input.state)
        .collect()
}

#[test]
fn a_focus_loss_releases_a_synthesized_modifier_through_the_sink() {
    let mut app = app(false);
    app.add_systems(
        First,
        pump_shift_across_a_focus_loss.in_set(InputSystems::Pump),
    );
    app.update();
    assert_eq!(shift_keys(&mut app), [ButtonState::Pressed]);
    app.update();
    assert_eq!(shift_keys(&mut app), [ButtonState::Released]);
    app.update();
    assert_eq!(
        shift_keys(&mut app),
        [ButtonState::Pressed],
        "the next shifted key presses shift again"
    );
    let keys = app.world().resource::<ButtonInput<KeyCode>>();
    assert!(keys.pressed(KeyCode::ShiftLeft));
}

#[test]
fn a_shift_click_holds_the_shift_key_only_where_the_terminal_sends_none() {
    let mut legacy = app(false);
    legacy.add_systems(First, pump_shifted_click.in_set(InputSystems::Pump));
    legacy.update();
    let keys: Vec<KeyboardInput> = written(&mut legacy);
    assert_eq!(keys.len(), 1, "{keys:?}");
    assert_eq!(keys[0].key_code, KeyCode::ShiftLeft);
    assert!(legacy.world().resource::<PointerModifiers>().0.shift);

    let mut kitty = app(true);
    kitty.add_systems(First, pump_shifted_click.in_set(InputSystems::Pump));
    kitty.update();
    assert_eq!(written::<KeyboardInput>(&mut kitty), []);
    assert!(kitty.world().resource::<PointerModifiers>().0.shift);
}

#[test]
fn a_button_is_placed_in_its_cell_once() {
    let mut app = app(true);
    app.add_systems(
        First,
        (|mut input: TerminalInput| {
            let at = Position::new(3, 1);
            input.moved(at, KeyModifiers::none());
            input.button(
                MouseButton::Left,
                ButtonState::Pressed,
                at,
                KeyModifiers::none(),
            );
        })
        .in_set(InputSystems::Pump),
    );
    app.update();

    let moves: Vec<CursorMoved> = written(&mut app);
    assert_eq!(moves.len(), 1, "the press is at the cell the move placed");
    let cell = app.world().resource::<plurimus_core::CellSize>().vec();
    assert_eq!(moves[0].position, bevy_math::Vec2::new(3.5, 1.5) * cell);
    let stream: Vec<WindowEvent> = written(&mut app);
    assert!(matches!(stream[0], WindowEvent::CursorMoved(_)));
    assert!(matches!(stream[1], WindowEvent::MouseButtonInput(_)));
}

// A release after a press waits a frame; a key does not.
#[test]
fn a_press_ends_the_frames_batch_for_pointer_input_alone() {
    let mut app = app(true);
    app.update();
    let window = window(&mut app);
    let button = |state| MouseButtonInput {
        button: MouseButton::Left,
        state,
        window,
    };
    let key = KeyboardInput {
        key_code: KeyCode::KeyA,
        logical_key: Key::Character("a".into()),
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    };
    let none = KeyModifiers::none();
    let world = app.world_mut();
    write_input(
        world,
        WindowEvent::MouseButtonInput(button(ButtonState::Pressed)),
        none,
    );
    write_input(
        world,
        WindowEvent::MouseButtonInput(button(ButtonState::Released)),
        none,
    );
    write_input(world, WindowEvent::KeyboardInput(key), none);
    assert_eq!(written::<MouseButtonInput>(&mut app).len(), 1);
    assert_eq!(written::<KeyboardInput>(&mut app).len(), 1);

    app.update();
    let landed: Vec<MouseButtonInput> = written(&mut app);
    assert_eq!(landed.len(), 1);
    assert_eq!(landed[0].state, ButtonState::Released);
}
