//! Multi-line text editor flows, fully headless.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{On, ResMut, Resource};
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_input::{
    InputCapabilities, KeyCode, KeyModifiers, ModifierKey, MouseKind, PasteMessage,
};
use plurimus_test::{composed_frame, press_key, press_key_with, send_mouse};
use plurimus_ui::UiArea;
use plurimus_widgets::{TextChanged, TextEditor, WidgetsPlugin, text_editor};

#[derive(Resource, Default)]
struct Changes(usize);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize { cols: 8, rows: 2 });
    app.init_resource::<Changes>();
    app.add_observer(|_: On<TextChanged>, mut log: ResMut<Changes>| {
        log.0 += 1;
    });
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_editor(app: &mut App, text: &str) -> Entity {
    let world = app.world_mut();
    let editor = world
        .spawn((text_editor(text), UiArea::Fixed(Rect::new(0, 0, 8, 2))))
        .id();
    world
        .resource_mut::<InputFocus>()
        .set(editor, FocusCause::Pressed);
    editor
}

// Only the modifier bits, with no modifier key press: this is what a
// terminal without the kitty tier sends.
fn ctrl_key(app: &mut App, code: KeyCode) {
    press_key_with(app, code, KeyModifiers::default().with_ctrl(true));
}

fn editor_typed_ab(app: &mut App) -> Entity {
    let editor = spawn_editor(app, "");
    press_key(app, KeyCode::Char('a'));
    press_key(app, KeyCode::Char('b'));
    assert_eq!(lines_of(app, editor), ["ab"]);
    editor
}

fn lines_of(app: &App, editor: Entity) -> Vec<String> {
    app.world()
        .get::<TextEditor>(editor)
        .unwrap()
        .lock()
        .lines()
        .to_vec()
}

#[test]
fn typing_edits_multiple_lines() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "");

    press_key(&mut app, KeyCode::Char('a'));
    press_key(&mut app, KeyCode::Char('b'));
    press_key(&mut app, KeyCode::Enter);
    press_key(&mut app, KeyCode::Char('c'));

    assert_eq!(lines_of(&app, editor), ["ab", "c"]);
    assert_eq!(app.world().resource::<Changes>().0, 4);
}

#[test]
fn tab_is_not_forwarded_to_the_editor() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "x");

    press_key(&mut app, KeyCode::Tab);

    assert_eq!(lines_of(&app, editor), ["x"]);
    assert_eq!(app.world().resource::<Changes>().0, 0);
}

#[test]
fn undo_reverts_programmatically() {
    let mut app = app();
    let editor = editor_typed_ab(&mut app);

    let handle = app.world().get::<TextEditor>(editor).unwrap().clone();
    assert!(handle.lock().undo());
    assert_ne!(lines_of(&app, editor), ["ab"]);
}

#[test]
fn ctrl_undo_from_real_modifier_keys() {
    let mut app = app();
    let editor = editor_typed_ab(&mut app);

    ctrl_key(&mut app, KeyCode::Modifier(ModifierKey::ControlLeft));
    ctrl_key(&mut app, KeyCode::Char('u'));

    assert_eq!(lines_of(&app, editor), ["a"], "ctrl+u undoes, inserts no u");
}

#[test]
fn ctrl_undo_from_synthesized_modifiers() {
    let mut app = app();
    app.insert_resource(InputCapabilities::none());
    let editor = editor_typed_ab(&mut app);

    ctrl_key(&mut app, KeyCode::Char('u'));

    assert_eq!(lines_of(&app, editor), ["a"], "ctrl+u undoes, inserts no u");
}

#[test]
fn paste_inserts_multi_line_text() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "");

    app.world_mut()
        .write_message(PasteMessage("one\ntwo".into()));
    app.update();

    assert_eq!(lines_of(&app, editor), ["one", "two"]);
    assert_eq!(app.world().resource::<Changes>().0, 1);
}

#[test]
fn live_view_renders_edits_without_rebuild() {
    let mut app = app();
    spawn_editor(&mut app, "");

    press_key(&mut app, KeyCode::Char('h'));
    press_key(&mut app, KeyCode::Char('i'));
    app.update();

    insta::assert_snapshot!("editor_live_view", composed_frame(&app));
}

#[test]
fn wheel_scrolls_the_viewport() {
    let mut app = app();
    spawn_editor(&mut app, "l1\nl2\nl3\nl4\nl5\nl6");
    app.update();

    for _ in 0..2 {
        send_mouse(&mut app, MouseKind::ScrollDown, 2, 1);
    }

    insta::assert_snapshot!("editor_scrolled", composed_frame(&app));
}
