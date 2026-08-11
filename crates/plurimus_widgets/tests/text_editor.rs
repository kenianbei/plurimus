//! Multi-line text editor flows, fully headless.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{On, ResMut, Resource};
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{
    InputCapabilities, KeyCode, KeyModifiers, LastCopied, ModifierKey, MouseKind, PasteMessage,
};
use plurimus_test::{
    clipboard_writes, composed_frame, press_chord, press_key, press_key_with, send_mouse,
};
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

// A bare key reads held modifier state, not the message's own bits, so a
// shift outliving its chord turns this Right into a second selection step.
#[test]
fn a_chord_leaves_its_modifier_released() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "abcd");
    press_key(&mut app, KeyCode::Home);

    press_chord(&mut app, ModifierKey::ShiftLeft, KeyCode::Right);
    press_key(&mut app, KeyCode::Right);
    press_key(&mut app, KeyCode::Backspace);

    assert_eq!(
        lines_of(&app, editor),
        ["acd"],
        "the bare Right should drop the selection, not extend it"
    );
}

// Selects `count` characters from the start of the text.
fn select_from_home(app: &mut App, count: usize) {
    press_key(app, KeyCode::Home);
    for _ in 0..count {
        press_chord(app, ModifierKey::ShiftLeft, KeyCode::Right);
    }
}

#[test]
fn a_copy_offers_the_selection_to_the_terminal() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "abcd");
    select_from_home(&mut app, 2);

    press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Char('c'));

    assert_eq!(clipboard_writes(&mut app), ["ab"]);
    assert_eq!(lines_of(&app, editor), ["abcd"], "a copy removes nothing");
}

#[test]
fn a_cut_offers_the_selection_and_removes_it() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "abcd");
    let quiet = app.world().resource::<Changes>().0;
    select_from_home(&mut app, 2);

    press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Char('x'));

    assert_eq!(clipboard_writes(&mut app), ["ab"]);
    assert_eq!(lines_of(&app, editor), ["cd"]);
    assert!(
        app.world().resource::<Changes>().0 > quiet,
        "a cut edits, so it announces"
    );
}

// An empty write would take away whatever the user last copied, which is
// worse than doing nothing at all.
#[test]
fn a_copy_with_no_selection_offers_nothing() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "abcd");
    press_key(&mut app, KeyCode::Home);

    press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Char('c'));
    press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Char('x'));

    assert!(clipboard_writes(&mut app).is_empty());
    assert_eq!(lines_of(&app, editor), ["abcd"], "and cuts nothing");
}

#[test]
fn a_copy_fills_the_shared_buffer() {
    let mut app = app();
    spawn_editor(&mut app, "abcd");
    select_from_home(&mut app, 3);

    press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Char('c'));

    assert_eq!(
        app.world().resource::<LastCopied>().0.as_deref(),
        Some("abc"),
        "the echo is what another widget's paste key reads"
    );
}

#[test]
fn a_copy_in_one_editor_pastes_into_another() {
    let mut app = app();
    let source = spawn_editor(&mut app, "abcd");
    select_from_home(&mut app, 2);
    press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Char('c'));

    let target = spawn_editor(&mut app, "");
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(target, FocusCause::Pressed);
    press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Char('v'));

    assert_eq!(lines_of(&app, target), ["ab"]);
    assert_eq!(lines_of(&app, source), ["abcd"], "the source is untouched");
}

#[test]
fn ctrl_v_pastes_rather_than_paging() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "abcd");
    select_from_home(&mut app, 2);
    press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Char('c'));
    press_key(&mut app, KeyCode::End);

    press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Char('v'));

    assert_eq!(lines_of(&app, editor), ["abcdab"]);
}

// The shared buffer must not displace a kill the user has not replaced.
// A design that read the echo at paste time instead of syncing on change
// would paste "ab" here.
#[test]
fn a_kill_still_yanks_what_it_killed() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "abcd");
    select_from_home(&mut app, 2);
    press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Char('c'));

    press_key(&mut app, KeyCode::Home);
    press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Char('k'));
    press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Char('y'));

    assert_eq!(
        lines_of(&app, editor),
        ["abcd"],
        "the killed line comes back, not the older copy"
    );
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
