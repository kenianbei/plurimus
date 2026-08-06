//! Grapheme-cluster movement, deletion, and word motion in `EditableText`.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_input::{KeyCode, ModifierKey};
use plurimus_test::{press_chord, press_key};
use plurimus_ui::UiArea;
use plurimus_widgets::{TextInput, WidgetsPlugin, editable_text};

const ACCENT: &str = "e\u{301}";
const FAMILY: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
const FLAG: &str = "\u{1F1EB}\u{1F1F7}";
// Word rules break between `#` and its variation selector; cluster rules
// do not.
const KEYCAP: &str = "#\u{FE0F}\u{20E3}";

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize { cols: 20, rows: 1 });
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_field(app: &mut App, value: &str) -> Entity {
    let world = app.world_mut();
    let field = world
        .spawn((editable_text(value), UiArea::Fixed(Rect::new(0, 0, 20, 1))))
        .id();
    world
        .resource_mut::<InputFocus>()
        .set(field, FocusCause::Pressed);
    field
}

fn value_of(app: &App, field: Entity) -> String {
    let text = app.world().entity(field).get::<TextInput>().unwrap();
    text.value().to_owned()
}

fn position_of(app: &App, field: Entity) -> usize {
    app.world()
        .entity(field)
        .get::<TextInput>()
        .unwrap()
        .cursor()
}

#[test]
fn arrow_left_crosses_a_whole_cluster() {
    for value in [ACCENT, FAMILY, FLAG] {
        let mut app = app();
        let field = spawn_field(&mut app, value);
        press_key(&mut app, KeyCode::Left);
        assert_eq!(
            position_of(&app, field),
            0,
            "one press should cross all of {value:?}"
        );
    }
}

#[test]
fn arrow_right_crosses_a_whole_cluster() {
    for value in [ACCENT, FAMILY, FLAG] {
        let mut app = app();
        let field = spawn_field(&mut app, value);
        press_key(&mut app, KeyCode::Home);
        press_key(&mut app, KeyCode::Right);
        let text = value_of(&app, field);
        let position = position_of(&app, field);
        assert_eq!(
            position,
            text.chars().count(),
            "one press should cross all of {value:?}"
        );
    }
}

#[test]
fn backspace_removes_a_whole_cluster() {
    for value in [ACCENT, FAMILY, FLAG] {
        let mut app = app();
        let field = spawn_field(&mut app, value);
        press_key(&mut app, KeyCode::Backspace);
        assert_eq!(
            value_of(&app, field),
            "",
            "backspace should remove all of {value:?}"
        );
    }
}

#[test]
fn delete_removes_a_whole_cluster() {
    for value in [ACCENT, FAMILY, FLAG] {
        let mut app = app();
        let field = spawn_field(&mut app, value);
        press_key(&mut app, KeyCode::Home);
        press_key(&mut app, KeyCode::Delete);
        assert_eq!(
            value_of(&app, field),
            "",
            "delete should remove all of {value:?}"
        );
    }
}

#[test]
fn clusters_are_crossed_without_disturbing_neighbours() {
    let mut app = app();
    let field = spawn_field(&mut app, &format!("a{FAMILY}b"));
    press_key(&mut app, KeyCode::Left);
    press_key(&mut app, KeyCode::Backspace);
    assert_eq!(value_of(&app, field), "ab");
}

#[test]
fn word_motion_matches_textarea_boundaries() {
    let mut app = app();
    let field = spawn_field(&mut app, "fn foo(a)");
    press_key(&mut app, KeyCode::Home);
    let expected = [3, 6, 7, 8, 9];
    let mut stops = Vec::new();
    for _ in 0..expected.len() {
        press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Right);
        stops.push(position_of(&app, field));
    }
    assert_eq!(stops, expected);
}

#[test]
fn word_motion_runs_backward() {
    let mut app = app();
    let field = spawn_field(&mut app, "fn foo(a)");
    let expected = [8, 7, 6, 3, 0];
    let mut stops = Vec::new();
    for _ in 0..expected.len() {
        press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Left);
        stops.push(position_of(&app, field));
    }
    assert_eq!(stops, expected);
}

#[test]
fn word_deletion_removes_back_to_the_boundary() {
    let mut app = app();
    let field = spawn_field(&mut app, "fn foo bar");
    press_chord(&mut app, ModifierKey::AltLeft, KeyCode::Backspace);
    assert_eq!(value_of(&app, field), "fn foo ");
}

#[test]
fn forward_word_deletion_removes_to_the_boundary() {
    let mut app = app();
    let field = spawn_field(&mut app, "fn foo bar");
    press_key(&mut app, KeyCode::Home);
    press_chord(&mut app, ModifierKey::AltLeft, KeyCode::Delete);
    assert_eq!(value_of(&app, field), " foo bar");
}

#[test]
fn word_deletion_takes_whole_clusters() {
    let mut app = app();
    let field = spawn_field(&mut app, &format!("hi {FAMILY}{FLAG}"));
    press_chord(&mut app, ModifierKey::AltLeft, KeyCode::Backspace);
    assert_eq!(value_of(&app, field), "hi ");
}

#[test]
fn word_motion_never_lands_inside_a_cluster() {
    let mut app = app();
    let field = spawn_field(&mut app, &format!("a{KEYCAP}b"));
    press_key(&mut app, KeyCode::Home);
    press_key(&mut app, KeyCode::Right);
    press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Right);
    assert_eq!(
        position_of(&app, field),
        4,
        "should clear the keycap cluster, not stop inside it"
    );
}

#[test]
fn word_deletion_never_splits_a_cluster() {
    let mut app = app();
    let field = spawn_field(&mut app, &format!("a{KEYCAP}"));
    press_key(&mut app, KeyCode::Home);
    press_key(&mut app, KeyCode::Right);
    press_chord(&mut app, ModifierKey::AltLeft, KeyCode::Delete);
    assert_eq!(value_of(&app, field), "a", "left a cluster fragment");
}
