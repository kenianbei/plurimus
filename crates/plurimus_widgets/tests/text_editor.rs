//! Multi-line text editor flows, fully headless.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{On, ResMut, Resource};
use bevy_input::keyboard::Key;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{InputCapabilities, KeyModifiers, LastCopied};
use plurimus_test::{
    character, clipboard_writes, composed_frame, press_key, press_key_with, scroll_at, send_paste,
    set_focus, write_key,
};
use plurimus_ui::{KeyBinding, UiArea};
use plurimus_widgets::{
    TextChanged, TextEditor, TextEditorAction, TextEditorKeys, WidgetsPlugin, text_editor,
};

#[derive(Resource, Default)]
struct Changes(usize);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(8, 2));
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
    set_focus(app, editor);
    editor
}

fn ctrl_key(app: &mut App, key: Key) {
    press_key_with(app, key, KeyModifiers::default().with_ctrl(true));
}

fn ctrl_chord(app: &mut App, typed: char) {
    press_key_with(app, character(typed), KeyModifiers::none().with_ctrl(true));
}

fn ctrl_letter(letter: &str) -> KeyBinding {
    KeyBinding::new(Key::Character(letter.into())).with_ctrl()
}

fn select_from_home(app: &mut App, count: usize) {
    press_key(app, Key::Home);
    for _ in 0..count {
        press_key_with(app, Key::ArrowRight, KeyModifiers::none().with_shift(true));
    }
}

fn editor_typed_ab(app: &mut App) -> Entity {
    let editor = spawn_editor(app, "");
    press_key(app, character('a'));
    press_key(app, character('b'));
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

    press_key(&mut app, character('a'));
    press_key(&mut app, character('b'));
    press_key(&mut app, Key::Enter);
    press_key(&mut app, character('c'));

    assert_eq!(lines_of(&app, editor), ["ab", "c"]);
    assert_eq!(app.world().resource::<Changes>().0, 4);
}

#[test]
fn tab_is_not_forwarded_to_the_editor() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "x");

    press_key(&mut app, Key::Tab);

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

    ctrl_chord(&mut app, 'z');

    assert_eq!(lines_of(&app, editor), ["a"], "ctrl+z undoes, inserts no z");
}

#[test]
fn ctrl_undo_from_synthesized_modifiers() {
    let mut app = app();
    app.insert_resource(InputCapabilities::none());
    let editor = editor_typed_ab(&mut app);

    ctrl_key(&mut app, character('z'));

    assert_eq!(lines_of(&app, editor), ["a"], "ctrl+z undoes, inserts no z");
}

#[test]
fn paste_inserts_multi_line_text() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "");

    send_paste(&mut app, "one\ntwo");

    assert_eq!(lines_of(&app, editor), ["one", "two"]);
    assert_eq!(app.world().resource::<Changes>().0, 1);
}

#[test]
fn live_view_renders_edits_without_rebuild() {
    let mut app = app();
    spawn_editor(&mut app, "");

    press_key(&mut app, character('h'));
    press_key(&mut app, character('i'));
    app.update();

    insta::assert_snapshot!("editor_live_view", composed_frame(&app));
}

// A bare key reads held modifier state, not the message's own bits, so a
// shift outliving its chord turns this Right into a second selection step.
#[test]
fn a_chord_leaves_its_modifier_released() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "abcd");
    press_key(&mut app, Key::Home);

    press_key_with(
        &mut app,
        Key::ArrowRight,
        KeyModifiers::none().with_shift(true),
    );
    press_key(&mut app, Key::ArrowRight);
    press_key(&mut app, Key::Backspace);

    assert_eq!(
        lines_of(&app, editor),
        ["acd"],
        "the bare Right should drop the selection, not extend it"
    );
}

#[test]
fn a_copy_offers_the_selection_to_the_terminal() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "abcd");
    select_from_home(&mut app, 2);

    ctrl_chord(&mut app, 'c');

    assert_eq!(clipboard_writes(&mut app), ["ab"]);
    assert_eq!(lines_of(&app, editor), ["abcd"], "a copy removes nothing");
}

#[test]
fn a_cut_offers_the_selection_and_removes_it() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "abcd");
    select_from_home(&mut app, 2);

    ctrl_chord(&mut app, 'x');

    assert_eq!(clipboard_writes(&mut app), ["ab"]);
    assert_eq!(lines_of(&app, editor), ["cd"]);
    assert_eq!(
        app.world().resource::<Changes>().0,
        1,
        "a cut edits, so it announces once"
    );
}

// An empty write would take away whatever the user last copied, which is
// worse than doing nothing at all.
#[test]
fn a_copy_with_no_selection_offers_nothing() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "abcd");
    press_key(&mut app, Key::Home);

    ctrl_chord(&mut app, 'c');
    ctrl_chord(&mut app, 'x');

    assert_eq!(clipboard_writes(&mut app), [] as [String; 0]);
    assert_eq!(lines_of(&app, editor), ["abcd"], "and cuts nothing");
}

#[test]
fn a_copy_fills_the_shared_buffer() {
    let mut app = app();
    spawn_editor(&mut app, "abcd");
    select_from_home(&mut app, 3);

    ctrl_chord(&mut app, 'c');

    assert_eq!(
        app.world().resource::<LastCopied>().0.as_deref(),
        Some("abc"),
        "the echo is what another widget's paste key reads"
    );
}

// Seeding what a paste key inserts is documented app-facing use, and it
// lands on the very next frame because the paste reads the resource at the
// press rather than from anything mirrored beforehand.
#[test]
fn a_seeded_buffer_pastes_on_the_next_frame() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "");
    write_key(&mut app, Key::Control);
    app.update();

    app.world_mut().resource_mut::<LastCopied>().0 = Some("seeded".to_owned());
    ctrl_key(&mut app, character('v'));

    assert_eq!(lines_of(&app, editor), ["seeded"]);
}

#[test]
fn a_copy_in_one_editor_pastes_into_another() {
    let mut app = app();
    let source = spawn_editor(&mut app, "abcd");
    select_from_home(&mut app, 2);
    ctrl_chord(&mut app, 'c');

    let target = spawn_editor(&mut app, "");
    set_focus(&mut app, target);
    ctrl_chord(&mut app, 'v');

    assert_eq!(lines_of(&app, target), ["ab"]);
    assert_eq!(lines_of(&app, source), ["abcd"], "the source is untouched");
}

// The app clipboard and the engine's kill ring stay separate: a yank takes
// what the kill took, not the older copy. Any design that fed the shared
// buffer into the engine's own yank would paste "ab" here.
#[test]
fn a_kill_still_yanks_what_it_killed() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "abcd");
    let mut keys = TextEditorKeys::default();
    keys.0.extend([
        (ctrl_letter("k"), TextEditorAction::DeleteToLineEnd),
        (ctrl_letter("u"), TextEditorAction::Yank),
    ]);
    app.world_mut().entity_mut(editor).insert(keys);
    select_from_home(&mut app, 2);
    ctrl_chord(&mut app, 'c');

    press_key(&mut app, Key::Home);
    ctrl_chord(&mut app, 'k');
    ctrl_chord(&mut app, 'u');

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
        scroll_at(&mut app, 2, 1, 1);
    }

    insta::assert_snapshot!("editor_scrolled", composed_frame(&app));
}
