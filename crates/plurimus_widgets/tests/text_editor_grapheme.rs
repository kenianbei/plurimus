//! Grapheme-cluster movement and deletion in the multi-line editor.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_input::{KeyCode, ModifierKey};
use plurimus_test::{press_chord, press_key};
use plurimus_ui::UiArea;
use plurimus_widgets::{TextEditor, WidgetsPlugin, text_editor};
use ratatui_textarea::DataCursor;

const ACCENT: &str = "e\u{301}";
const FAMILY: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
const FLAG: &str = "\u{1F1EB}\u{1F1F7}";

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize { cols: 20, rows: 4 });
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_editor(app: &mut App, text: &str) -> Entity {
    let world = app.world_mut();
    let editor = world
        .spawn((text_editor(text), UiArea::Fixed(Rect::new(0, 0, 20, 4))))
        .id();
    world
        .resource_mut::<InputFocus>()
        .set(editor, FocusCause::Pressed);
    editor
}

fn cursor(app: &App, editor: Entity) -> (usize, usize) {
    let handle = app.world().entity(editor).get::<TextEditor>().unwrap();
    let DataCursor(row, column) = handle.lock().cursor();
    (row, column)
}

fn lines(app: &App, editor: Entity) -> Vec<String> {
    let handle = app.world().entity(editor).get::<TextEditor>().unwrap();
    handle.lock().lines().to_vec()
}

// Clusters are tested embedded in ordinary text: a line holding nothing
// but a cluster whose tail is zero-width collapses to zero display width
// once the base scalar goes, and textarea's screen map then refuses
// further edits on it.
#[test]
fn left_crosses_a_whole_cluster() {
    for cluster in [ACCENT, FAMILY, FLAG] {
        let mut app = app();
        let editor = spawn_editor(&mut app, &format!("a{cluster}b"));
        press_key(&mut app, KeyCode::End);
        press_key(&mut app, KeyCode::Left);
        press_key(&mut app, KeyCode::Left);
        press_key(&mut app, KeyCode::Delete);
        assert_eq!(lines(&app, editor), ["ab"], "left crosses {cluster:?}");
    }
}

#[test]
fn right_crosses_a_whole_cluster() {
    for cluster in [ACCENT, FAMILY, FLAG] {
        let mut app = app();
        let editor = spawn_editor(&mut app, &format!("a{cluster}b"));
        press_key(&mut app, KeyCode::Home);
        press_key(&mut app, KeyCode::Right);
        press_key(&mut app, KeyCode::Right);
        press_key(&mut app, KeyCode::Backspace);
        assert_eq!(lines(&app, editor), ["ab"], "right crosses {cluster:?}");
    }
}

#[test]
fn backspace_removes_a_whole_cluster() {
    for cluster in [ACCENT, FAMILY, FLAG] {
        let mut app = app();
        let editor = spawn_editor(&mut app, &format!("a{cluster}"));
        press_key(&mut app, KeyCode::End);
        press_key(&mut app, KeyCode::Backspace);
        assert_eq!(lines(&app, editor), ["a"], "backspace removes {cluster:?}");
    }
}

#[test]
fn delete_removes_a_whole_cluster() {
    for cluster in [ACCENT, FAMILY, FLAG] {
        let mut app = app();
        let editor = spawn_editor(&mut app, &format!("{cluster}b"));
        press_key(&mut app, KeyCode::Home);
        press_key(&mut app, KeyCode::Delete);
        assert_eq!(lines(&app, editor), ["b"], "delete removes {cluster:?}");
    }
}

#[test]
fn left_still_wraps_to_the_previous_line() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "ab\ncd");
    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Home);
    press_key(&mut app, KeyCode::Left);
    assert_eq!(cursor(&app, editor), (0, 2), "column 0 wraps up a line");
}

#[test]
fn backspace_still_joins_lines() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "ab\ncd");
    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Home);
    press_key(&mut app, KeyCode::Backspace);
    assert_eq!(lines(&app, editor), ["abcd"], "column 0 joins the lines");
}

// The selection must span a multi-scalar cluster and leave the cursor
// past it, or the repeat count would be 1 anyway and prove nothing.
#[test]
fn backspace_over_a_selection_deletes_only_the_selection() {
    let mut app = app();
    let editor = spawn_editor(&mut app, &format!("ab{FAMILY}cd"));
    press_key(&mut app, KeyCode::Home);
    press_key(&mut app, KeyCode::Right);
    press_key(&mut app, KeyCode::Right);
    press_chord(&mut app, ModifierKey::ShiftLeft, KeyCode::Right);
    press_key(&mut app, KeyCode::Backspace);
    assert_eq!(lines(&app, editor), ["abcd"], "only the selection goes");
}

#[test]
fn word_motion_still_reaches_the_engine() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "fn foo(a)");
    press_key(&mut app, KeyCode::Home);
    // Pins widgets/word.rs against the engine it mirrors: if textarea's
    // rules move, this list stops matching word.rs's unit test.
    let expected = [3, 6, 7, 8, 9];
    let mut stops = Vec::new();
    for _ in 0..expected.len() {
        press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Right);
        stops.push(cursor(&app, editor).1);
    }
    assert_eq!(stops, expected);
}
