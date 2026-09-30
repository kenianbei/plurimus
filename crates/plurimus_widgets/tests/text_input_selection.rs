//! The single-line field's selection: shifted keys and the pointer extend
//! it, every edit replaces it, and the clipboard keys copy, cut and paste
//! it.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::FocusedInput;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{InputCapabilities, KeyCode, KeyModifiers, LastCopied};
use plurimus_test::{clipboard_writes, press_key, press_key_with, set_focus};
use plurimus_ui::{UiArea, ValueChange};
use plurimus_widgets::{TextInput, TextMask, WidgetsPlugin, editable_text};

const WIDTH: u16 = 12;

#[derive(Resource, Default)]
struct Propagated(Vec<Key>);

#[derive(Resource, Default)]
struct Edits(Vec<String>);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    // The legacy tier reads a modifier from the bits on the key, which is
    // all `press_key_with` sends; the kitty tier waits for the modifier's
    // own press.
    app.insert_resource(InputCapabilities::none());
    app.insert_resource(TerminalSize::new(WIDTH, 1));
    app.init_resource::<Propagated>();
    app.init_resource::<Edits>();
    app.add_observer(|change: On<ValueChange<String>>, mut log: ResMut<Edits>| {
        if !change.is_final {
            log.0.push(change.value.clone());
        }
    });
    app.world_mut().spawn(TerminalCamera::default());
    app
}

/// A focused field holding `value`, inside a form that records every key
/// but a modifier propagating past it.
fn spawn_field(app: &mut App, value: &str) -> Entity {
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
    let field = app
        .world_mut()
        .spawn((
            editable_text(value),
            UiArea::Fixed(Rect::new(0, 0, WIDTH, 1)),
            ChildOf(form),
        ))
        .id();
    set_focus(app, field);
    app.update();
    field
}

fn text(app: &App, field: Entity) -> &TextInput {
    app.world().get::<TextInput>(field).unwrap()
}

fn selected(app: &App, field: Entity) -> Option<String> {
    text(app, field).selected_text().map(str::to_owned)
}

fn propagated(app: &App) -> &[Key] {
    &app.world().resource::<Propagated>().0
}

fn edits(app: &App) -> &[String] {
    &app.world().resource::<Edits>().0
}

fn shift(app: &mut App, code: KeyCode) {
    press_key_with(app, code, KeyModifiers::default().with_shift(true));
}

fn ctrl(app: &mut App, character: char) {
    press_key_with(
        app,
        KeyCode::Char(character),
        KeyModifiers::default().with_ctrl(true),
    );
}

#[test]
fn shift_selects_by_cluster_and_ctrl_shift_by_word() {
    let mut app = app();
    let field = spawn_field(&mut app, "one two");

    shift(&mut app, KeyCode::Left);
    assert_eq!(selected(&app, field).as_deref(), Some("o"));

    press_key_with(
        &mut app,
        KeyCode::Left,
        KeyModifiers::default().with_shift(true).with_ctrl(true),
    );
    assert_eq!(selected(&app, field).as_deref(), Some("two"));

    shift(&mut app, KeyCode::Home);
    assert_eq!(selected(&app, field).as_deref(), Some("one two"));
    assert!(propagated(&app).is_empty());
}

#[test]
fn typing_over_a_one_char_selection_notifies() {
    let mut app = app();
    let field = spawn_field(&mut app, "abc");

    shift(&mut app, KeyCode::Left);
    press_key(&mut app, KeyCode::Char('x'));

    assert_eq!(text(&app, field).value(), "abx");
    assert_eq!(
        edits(&app),
        ["abx"],
        "the length did not change, the value did"
    );
}

#[test]
fn backspace_deletes_the_selection_rather_than_a_cluster() {
    let mut app = app();
    let field = spawn_field(&mut app, "abcd");

    shift(&mut app, KeyCode::Left);
    shift(&mut app, KeyCode::Left);
    press_key(&mut app, KeyCode::Backspace);

    assert_eq!(text(&app, field).value(), "ab");
}

// As the editor does: the selection ends and the cursor steps from where
// it was, rather than collapsing to the selection's far edge.
#[test]
fn a_plain_arrow_ends_the_selection_and_steps_from_the_cursor() {
    let mut app = app();
    let field = spawn_field(&mut app, "abc");

    shift(&mut app, KeyCode::Left);
    shift(&mut app, KeyCode::Left);
    press_key(&mut app, KeyCode::Right);

    assert_eq!(text(&app, field).selection(), None);
    assert_eq!(text(&app, field).cursor(), 2);
}

#[test]
fn ctrl_c_copies_the_selection_and_edits_nothing() {
    let mut app = app();
    let field = spawn_field(&mut app, "hello");

    ctrl(&mut app, 'a');
    ctrl(&mut app, 'c');

    assert_eq!(clipboard_writes(&mut app), ["hello"]);
    assert_eq!(text(&app, field).value(), "hello");
    assert!(edits(&app).is_empty());
    assert!(propagated(&app).is_empty());
}

#[test]
fn ctrl_x_cuts_and_notifies() {
    let mut app = app();
    let field = spawn_field(&mut app, "hello");

    shift(&mut app, KeyCode::Left);
    shift(&mut app, KeyCode::Left);
    ctrl(&mut app, 'x');

    assert_eq!(clipboard_writes(&mut app), ["lo"]);
    assert_eq!(text(&app, field).value(), "hel");
    assert_eq!(edits(&app), ["hel"]);
}

#[test]
fn ctrl_v_pastes_what_was_last_copied_over_the_selection() {
    let mut app = app();
    let field = spawn_field(&mut app, "hello");
    app.world_mut().resource_mut::<LastCopied>().0 = Some("bye".to_owned());

    ctrl(&mut app, 'a');
    ctrl(&mut app, 'v');

    assert_eq!(text(&app, field).value(), "bye");
    assert_eq!(edits(&app), ["bye"]);
}

#[test]
fn a_masked_field_copies_and_cuts_nothing() {
    let mut app = app();
    let field = spawn_field(&mut app, "secret");
    app.world_mut().entity_mut(field).insert(TextMask('*'));

    ctrl(&mut app, 'a');
    ctrl(&mut app, 'c');
    ctrl(&mut app, 'x');

    assert!(clipboard_writes(&mut app).is_empty());
    assert_eq!(text(&app, field).value(), "secret");
    assert!(
        propagated(&app).is_empty(),
        "both keys are still the field's"
    );
}

#[test]
fn an_empty_copy_is_consumed_and_escape_still_propagates() {
    let mut app = app();
    spawn_field(&mut app, "hello");

    ctrl(&mut app, 'c');
    press_key(&mut app, KeyCode::Esc);

    assert!(clipboard_writes(&mut app).is_empty());
    assert_eq!(propagated(&app), [Key::Escape]);
}

#[test]
fn losing_focus_ends_the_selection() {
    let mut app = app();
    let field = spawn_field(&mut app, "hello");

    ctrl(&mut app, 'a');
    let other = app.world_mut().spawn_empty().id();
    set_focus(&mut app, other);
    app.update();

    assert_eq!(text(&app, field).selection(), None);
}
