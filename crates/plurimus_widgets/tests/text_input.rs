//! Single-line text input editing flows, fully headless.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{KeyCode, ModifierKey, PasteMessage};
use plurimus_test::{composed_styled_frame, press_chord, press_key, repeat_key};
use plurimus_ui::{UiArea, ValueChange};
use plurimus_widgets::{Submit, TextInput, WidgetsPlugin, editable_text};

#[derive(Resource, Default)]
struct Edits(Vec<(String, bool)>);

#[derive(Resource, Default)]
struct Submits(Vec<String>);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(6, 1));
    app.init_resource::<Edits>();
    app.init_resource::<Submits>();
    app.add_observer(|change: On<ValueChange<String>>, mut log: ResMut<Edits>| {
        log.0.push((change.value.clone(), change.is_final));
    });
    app.add_observer(|submit: On<Submit>, mut log: ResMut<Submits>| {
        log.0.push(submit.value.clone());
    });
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_field(app: &mut App, value: &str) -> Entity {
    let world = app.world_mut();
    let field = world
        .spawn((editable_text(value), UiArea::Fixed(Rect::new(0, 0, 6, 1))))
        .id();
    world
        .resource_mut::<InputFocus>()
        .set(field, FocusCause::Pressed);
    field
}

fn value_of(app: &App, field: Entity) -> String {
    app.world()
        .get::<TextInput>(field)
        .unwrap()
        .value()
        .to_owned()
}

#[test]
fn typing_edits_and_notifies() {
    let mut app = app();
    let field = spawn_field(&mut app, "");

    press_key(&mut app, KeyCode::Char('h'));
    press_key(&mut app, KeyCode::Char('i'));

    assert_eq!(value_of(&app, field), "hi");
    assert_eq!(
        app.world().resource::<Edits>().0,
        [("h".to_owned(), false), ("hi".to_owned(), false)]
    );
}

#[test]
fn movement_and_deletion_edit_at_the_cursor() {
    let mut app = app();
    let field = spawn_field(&mut app, "hi");

    press_key(&mut app, KeyCode::Left);
    press_key(&mut app, KeyCode::Backspace);
    assert_eq!(value_of(&app, field), "i");

    press_key(&mut app, KeyCode::Delete);
    assert_eq!(value_of(&app, field), "");

    press_key(&mut app, KeyCode::Backspace);
    assert_eq!(value_of(&app, field), "");
    assert_eq!(app.world().resource::<Edits>().0.len(), 2);
}

#[test]
fn enter_and_blur_emit_final() {
    let mut app = app();
    spawn_field(&mut app, "ok");

    press_key(&mut app, KeyCode::Enter);
    assert_eq!(app.world().resource::<Edits>().0, [("ok".to_owned(), true)]);

    let other = app.world_mut().spawn(()).id();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(other, FocusCause::Pressed);
    app.update();
    assert_eq!(app.world().resource::<Edits>().0.len(), 2);
    assert_eq!(
        app.world().resource::<Edits>().0[1],
        ("ok".to_owned(), true)
    );
}

// The pair a form reads: what the value is, and whether the user committed
// it or just walked away.
#[test]
fn enter_submits_and_blur_does_not() {
    let mut app = app();
    spawn_field(&mut app, "ok");

    press_key(&mut app, KeyCode::Enter);
    assert_eq!(app.world().resource::<Submits>().0, ["ok".to_owned()]);

    let other = app.world_mut().spawn(()).id();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(other, FocusCause::Pressed);
    app.update();

    assert_eq!(
        app.world().resource::<Submits>().0.len(),
        1,
        "losing focus commits nothing"
    );
    assert_eq!(
        app.world().resource::<Edits>().0.len(),
        2,
        "though both still report a final value"
    );
}

// A terminal autorepeats a held key many times a second, and committing an
// entry that many times is never what leaning on Enter meant.
#[test]
fn holding_enter_submits_once() {
    let mut app = app();
    spawn_field(&mut app, "ok");

    press_key(&mut app, KeyCode::Enter);
    repeat_key(&mut app, KeyCode::Enter);
    repeat_key(&mut app, KeyCode::Enter);

    assert_eq!(app.world().resource::<Submits>().0, ["ok".to_owned()]);
    assert_eq!(
        app.world().resource::<Edits>().0.len(),
        1,
        "and reports one final value, not one per repeat"
    );
}

#[test]
fn a_chorded_character_never_reaches_the_value() {
    let mut app = app();
    let field = spawn_field(&mut app, "");

    press_key(&mut app, KeyCode::Char('c'));
    press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Char('c'));

    assert_eq!(value_of(&app, field), "c");
}

#[test]
fn paste_inserts_without_control_chars() {
    let mut app = app();
    let field = spawn_field(&mut app, "");

    app.world_mut()
        .write_message(PasteMessage("wo\nrld".into()));
    app.update();

    assert_eq!(value_of(&app, field), "world");
    assert_eq!(
        app.world().resource::<Edits>().0,
        [("world".to_owned(), false)]
    );
}

#[test]
fn paste_bubbles_from_a_focused_child() {
    let mut app = app();
    let field = spawn_field(&mut app, "");
    let child = app.world_mut().spawn(ChildOf(field)).id();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(child, FocusCause::Pressed);

    app.world_mut().write_message(PasteMessage("hi".into()));
    app.update();

    assert_eq!(value_of(&app, field), "hi");
}

#[test]
fn each_paste_emits_its_own_change() {
    let mut app = app();
    let field = spawn_field(&mut app, "");

    app.world_mut().write_message(PasteMessage("a".into()));
    app.world_mut().write_message(PasteMessage("b".into()));
    app.update();

    assert_eq!(value_of(&app, field), "ab");
    assert_eq!(
        app.world().resource::<Edits>().0,
        [("a".to_owned(), false), ("ab".to_owned(), false)]
    );
}

#[test]
fn paste_without_focus_is_dropped() {
    let mut app = app();
    let field = app
        .world_mut()
        .spawn((editable_text(""), UiArea::Fixed(Rect::new(0, 0, 6, 1))))
        .id();

    app.world_mut().write_message(PasteMessage("hi".into()));
    app.update();

    assert_eq!(value_of(&app, field), "");
    assert!(app.world().resource::<Edits>().0.is_empty());
}

#[test]
fn field_windows_long_values_and_marks_the_cursor() {
    let mut app = app();
    spawn_field(&mut app, "");

    for ch in ['a', 'b', 'c', 'd', 'e', 'f', 'g'] {
        press_key(&mut app, KeyCode::Char(ch));
    }
    app.update();

    insta::assert_snapshot!("windowed_field", composed_styled_frame(&app));
}

#[test]
fn wide_chars_place_the_cursor_by_column() {
    let mut app = app();
    spawn_field(&mut app, "日本");
    app.update();

    insta::assert_snapshot!("wide_char_cursor", composed_styled_frame(&app));
}
