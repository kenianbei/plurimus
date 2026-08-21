//! `ActivateKeys`: which keys activate a widget, and what a key it is not
//! bound to does instead.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::{FocusCause, FocusedInput, InputFocus};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::KeyCode;
use plurimus_test::{press_key, repeat_key};
use plurimus_ui::{InteractionDisabled, ValueChange};
use plurimus_widgets::{
    Activate, ActivateKeys, RadioGroup, WidgetsPlugin, button, checkbox, radio,
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(30, 8));
    app.world_mut().spawn(TerminalCamera::default());
    app.init_resource::<Activations>();
    app.init_resource::<Unconsumed>();
    app
}

fn focus(app: &mut App, entity: Entity) {
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(entity, FocusCause::Navigated);
}

#[derive(Resource, Default)]
struct Activations(u32);

/// Every key that reached the ancestor, which is every key the widget
/// below it did not consume.
#[derive(Resource, Default)]
struct Unconsumed(Vec<Key>);

fn space() -> Key {
    Key::Character(" ".into())
}

fn track_unconsumed(app: &mut App, entity: Entity) {
    app.world_mut().entity_mut(entity).observe(
        |input: On<FocusedInput<KeyboardInput>>, mut seen: ResMut<Unconsumed>| {
            seen.0.push(input.input.logical_key.clone());
        },
    );
}

/// An ancestor standing in for the form a widget sits inside, recording
/// what propagates past its child.
fn form(app: &mut App) -> Entity {
    let form = app.world_mut().spawn_empty().id();
    track_unconsumed(app, form);
    form
}

fn count_activations(app: &mut App, entity: Entity) {
    app.world_mut()
        .entity_mut(entity)
        .observe(|_on: On<Activate>, mut activations: ResMut<Activations>| activations.0 += 1);
}

fn seen(app: &App) -> &[Key] {
    &app.world().resource::<Unconsumed>().0
}

#[test]
fn the_default_bindings_activate_and_consume() {
    let mut app = app();
    let form = form(&mut app);
    let button = app.world_mut().spawn((button("ok"), ChildOf(form))).id();
    count_activations(&mut app, button);
    focus(&mut app, button);

    press_key(&mut app, KeyCode::Enter);
    press_key(&mut app, KeyCode::Char(' '));

    assert_eq!(
        app.world().resource::<Activations>().0,
        2,
        "the required component should bind Enter and space with no app help"
    );
    assert!(
        seen(&app).is_empty(),
        "an activating key is consumed, not propagated"
    );
}

#[test]
fn a_narrowed_checkbox_leaves_enter_to_the_form() {
    let mut app = app();
    let form = form(&mut app);
    let check = app
        .world_mut()
        .spawn((
            checkbox("agree"),
            ActivateKeys(vec![space()]),
            ChildOf(form),
        ))
        .id();
    app.world_mut().entity_mut(check).observe(
        |_on: On<ValueChange<bool>>, mut activations: ResMut<Activations>| activations.0 += 1,
    );
    focus(&mut app, check);

    press_key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.world().resource::<Activations>().0,
        0,
        "Enter is unbound, so it must not toggle"
    );
    assert_eq!(
        seen(&app),
        [Key::Enter],
        "the key the checkbox left alone must reach the form"
    );

    press_key(&mut app, KeyCode::Char(' '));
    assert_eq!(app.world().resource::<Activations>().0, 1);
    assert_eq!(seen(&app), [Key::Enter], "the bound key is consumed");
}

#[test]
fn a_narrowed_radio_uses_only_its_bound_key() {
    let mut app = app();
    let group = app.world_mut().spawn(RadioGroup).id();
    app.world_mut().entity_mut(group).observe(
        |_on: On<ValueChange<Entity>>, mut activations: ResMut<Activations>| activations.0 += 1,
    );
    track_unconsumed(&mut app, group);
    let option = app
        .world_mut()
        .spawn((radio("one"), ActivateKeys(vec![Key::Enter]), ChildOf(group)))
        .id();
    focus(&mut app, option);

    press_key(&mut app, KeyCode::Char(' '));
    assert_eq!(app.world().resource::<Activations>().0, 0);
    assert_eq!(seen(&app), [space()]);

    press_key(&mut app, KeyCode::Enter);
    assert_eq!(app.world().resource::<Activations>().0, 1);
    assert_eq!(seen(&app), [space()]);
}

#[test]
fn an_empty_binding_list_activates_on_nothing() {
    let mut app = app();
    let form = form(&mut app);
    let button = app
        .world_mut()
        .spawn((button("ok"), ActivateKeys(Vec::new()), ChildOf(form)))
        .id();
    count_activations(&mut app, button);
    focus(&mut app, button);

    press_key(&mut app, KeyCode::Enter);
    press_key(&mut app, KeyCode::Char(' '));

    assert_eq!(app.world().resource::<Activations>().0, 0);
    assert_eq!(
        seen(&app),
        [Key::Enter, space()],
        "a widget bound to nothing keeps the keyboard path out of its way"
    );
}

#[test]
fn a_disabled_widget_leaves_its_bound_keys_alone() {
    let mut app = app();
    let form = form(&mut app);
    let check = app
        .world_mut()
        .spawn((checkbox("agree"), InteractionDisabled, ChildOf(form)))
        .id();
    app.world_mut().entity_mut(check).observe(
        |_on: On<ValueChange<bool>>, mut activations: ResMut<Activations>| activations.0 += 1,
    );
    focus(&mut app, check);

    press_key(&mut app, KeyCode::Enter);

    assert_eq!(app.world().resource::<Activations>().0, 0);
    assert_eq!(
        seen(&app),
        [Key::Enter],
        "a key is consumed by activating, so one that activated nothing carries on"
    );
}

#[test]
fn a_repeat_of_a_bound_key_does_not_activate() {
    let mut app = app();
    let button = app.world_mut().spawn(button("ok")).id();
    count_activations(&mut app, button);
    focus(&mut app, button);

    press_key(&mut app, KeyCode::Enter);
    repeat_key(&mut app, KeyCode::Enter);

    assert_eq!(
        app.world().resource::<Activations>().0,
        1,
        "one intent commits once, however long the key is held"
    );
}
