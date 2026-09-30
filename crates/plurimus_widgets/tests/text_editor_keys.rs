//! The editor's keys come from its `TextEditorKeys` table: a bound key acts
//! and is consumed, an unbound character types, and an unbound chord
//! reaches the editor's ancestors.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::FocusedInput;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{InputCapabilities, KeyCode, KeyModifiers};
use plurimus_test::{clipboard_writes, press_key, press_key_with, set_focus};
use plurimus_ui::{KeyBinding, UiArea};
use plurimus_widgets::ratatui_textarea::CursorMove;
use plurimus_widgets::{TextEditor, TextEditorAction, TextEditorKeys, WidgetsPlugin, text_editor};

const AREA: Rect = Rect::new(0, 0, 20, 4);

#[derive(Resource, Default)]
struct Propagated(Vec<Key>);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    // The legacy tier reads a modifier from the bits on the key, which is
    // all `press_key_with` sends; the kitty tier waits for the modifier's
    // own press.
    app.insert_resource(InputCapabilities::none());
    app.insert_resource(TerminalSize::new(20, 4));
    app.init_resource::<Propagated>();
    app.world_mut().spawn(TerminalCamera::default());
    app
}

/// A focused editor holding `text`, inside a form that records every key
/// but a modifier propagating past it.
fn spawn_editor(app: &mut App, text: &str) -> Entity {
    let form = app.world_mut().spawn_empty().id();
    app.world_mut().entity_mut(form).observe(
        |input: On<FocusedInput<KeyboardInput>>, mut seen: ResMut<Propagated>| {
            let key = &input.input.logical_key;
            let is_modifier = matches!(
                key,
                Key::Control | Key::Shift | Key::Alt | Key::Super | Key::Meta | Key::Hyper
            );
            if input.input.state.is_pressed() && !is_modifier {
                seen.0.push(key.clone());
            }
        },
    );
    let editor = app
        .world_mut()
        .spawn((text_editor(text), UiArea::Fixed(AREA), ChildOf(form)))
        .id();
    set_focus(app, editor);
    app.update();
    editor
}

fn propagated(app: &App) -> &[Key] {
    &app.world().resource::<Propagated>().0
}

fn lines(app: &App, editor: Entity) -> Vec<String> {
    editor_handle(app, editor).lock().lines().to_vec()
}

fn is_selecting(app: &App, editor: Entity) -> bool {
    editor_handle(app, editor).lock().is_selecting()
}

fn editor_handle(app: &App, editor: Entity) -> &TextEditor {
    app.world().get::<TextEditor>(editor).unwrap()
}

fn ctrl(app: &mut App, character: char) {
    press_key_with(
        app,
        KeyCode::Char(character),
        KeyModifiers::default().with_ctrl(true),
    );
}

fn shift(app: &mut App, code: KeyCode) {
    press_key_with(app, code, KeyModifiers::default().with_shift(true));
}

#[test]
fn a_remapped_key_acts_instead_of_typing() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "ab");
    let keys = TextEditorKeys(vec![(
        KeyBinding::new(Key::Character("l".into())),
        TextEditorAction::Move(CursorMove::Forward),
    )]);
    app.world_mut().entity_mut(editor).insert(keys);

    press_key(&mut app, KeyCode::Home);
    press_key(&mut app, KeyCode::Char('l'));
    press_key(&mut app, KeyCode::Char('x'));

    assert_eq!(lines(&app, editor), ["axb"], "l moved rather than typed");
}

#[test]
fn an_unbound_character_still_types_under_an_empty_table() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "");
    app.world_mut()
        .entity_mut(editor)
        .insert(TextEditorKeys(Vec::new()));

    press_key(&mut app, KeyCode::Char('a'));
    press_key(&mut app, KeyCode::Enter);

    assert_eq!(lines(&app, editor), ["a"], "Enter is a binding, not a key");
    assert_eq!(propagated(&app), [Key::Enter]);
}

#[test]
fn an_unbound_chord_propagates_and_edits_nothing() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "abcd");

    ctrl(&mut app, 'k');

    assert_eq!(lines(&app, editor), ["abcd"]);
    assert_eq!(propagated(&app), [Key::Character("k".into())]);
}

#[test]
fn a_bound_key_that_moves_nothing_is_consumed() {
    let mut app = app();
    spawn_editor(&mut app, "ab");

    press_key(&mut app, KeyCode::Down);

    assert!(propagated(&app).is_empty(), "{:?}", propagated(&app));
}

#[test]
fn ctrl_y_redoes_what_ctrl_z_undid() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "");
    press_key(&mut app, KeyCode::Char('a'));

    ctrl(&mut app, 'z');
    assert_eq!(lines(&app, editor), [""]);
    ctrl(&mut app, 'y');

    assert_eq!(lines(&app, editor), ["a"]);
}

#[test]
fn alt_backspace_deletes_a_word() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "ab cd");
    press_key(&mut app, KeyCode::End);

    press_key_with(
        &mut app,
        KeyCode::Backspace,
        KeyModifiers::default().with_alt(true),
    );

    assert_eq!(lines(&app, editor), ["ab "]);
}

#[test]
fn ctrl_a_selects_everything() {
    let mut app = app();
    spawn_editor(&mut app, "ab\ncd");

    ctrl(&mut app, 'a');
    ctrl(&mut app, 'c');

    assert_eq!(clipboard_writes(&mut app), ["ab\ncd"]);
}

#[test]
fn escape_ends_the_selection() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "abcd");
    shift(&mut app, KeyCode::End);
    assert!(is_selecting(&app, editor));

    press_key(&mut app, KeyCode::Esc);

    assert!(!is_selecting(&app, editor));
    assert!(propagated(&app).is_empty(), "{:?}", propagated(&app));
}

// The engine leaves the selection alone when a plain motion finds nowhere
// to go; the table ends it, as a conventional editor does.
#[test]
fn a_plain_arrow_ends_the_selection_even_at_the_edge() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "ab");
    shift(&mut app, KeyCode::End);

    press_key(&mut app, KeyCode::Right);

    assert!(!is_selecting(&app, editor));
}

// An empty selection left behind would make the Backspace after it delete
// nothing.
#[test]
fn a_shift_arrow_that_cannot_move_starts_no_selection() {
    let mut app = app();
    let editor = spawn_editor(&mut app, "ab");
    press_key(&mut app, KeyCode::End);

    shift(&mut app, KeyCode::Right);

    assert!(!is_selecting(&app, editor));
    press_key(&mut app, KeyCode::Backspace);
    assert_eq!(lines(&app, editor), ["a"]);
}

#[test]
fn ctrl_shift_home_selects_to_the_start_of_the_document() {
    let mut app = app();
    spawn_editor(&mut app, "ab\ncd");
    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::End);

    press_key_with(
        &mut app,
        KeyCode::Home,
        KeyModifiers::default().with_ctrl(true).with_shift(true),
    );
    ctrl(&mut app, 'c');

    assert_eq!(clipboard_writes(&mut app), ["ab\ncd"]);
}
