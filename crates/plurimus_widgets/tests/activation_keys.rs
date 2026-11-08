//! The keys that activate a button, a checkbox or a radio: upstream's
//! Enter and space, matched by physical key, and what any other key does
//! instead.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::FocusedInput;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::KeyModifiers;
use plurimus_test::{character, press_key, press_key_with, repeat_key, set_focus};
use plurimus_ui::{InteractionDisabled, ValueChange};
use plurimus_widgets::{Activate, WidgetsPlugin, button, checkbox};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(30, 8));
    app.world_mut().spawn(TerminalCamera::default());
    app.init_resource::<Activations>();
    app.init_resource::<Unconsumed>();
    app
}

#[derive(Resource, Default)]
struct Activations(u32);

/// Every key press that reached the ancestor, which is every press the
/// widget below it did not consume.
#[derive(Resource, Default)]
struct Unconsumed(Vec<Key>);

fn track_unconsumed(app: &mut App, entity: Entity) {
    app.world_mut().entity_mut(entity).observe(
        |input: On<FocusedInput<KeyboardInput>>, mut seen: ResMut<Unconsumed>| {
            if input.input.state.is_pressed() {
                seen.0.push(input.input.logical_key.clone());
            }
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
    set_focus(&mut app, button);

    press_key(&mut app, Key::Enter);
    press_key(&mut app, character(' '));

    assert_eq!(
        app.world().resource::<Activations>().0,
        2,
        "upstream's observer binds Enter and space with no app help"
    );
    assert!(
        seen(&app).is_empty(),
        "an activating key is consumed, not propagated"
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
    set_focus(&mut app, check);

    press_key(&mut app, Key::Enter);

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
    set_focus(&mut app, button);

    press_key(&mut app, Key::Enter);
    repeat_key(&mut app, Key::Enter);

    assert_eq!(
        app.world().resource::<Activations>().0,
        1,
        "one intent commits once, however long the key is held"
    );
}

// No modifier is consulted, so a chord is its bare key.
#[test]
fn a_chord_activates_as_its_bare_key() {
    let mut app = app();
    let form = form(&mut app);
    let ok = app.world_mut().spawn((button("ok"), ChildOf(form))).id();
    count_activations(&mut app, ok);
    set_focus(&mut app, ok);

    press_key_with(&mut app, Key::Enter, KeyModifiers::none().with_ctrl(true));

    assert_eq!(app.world().resource::<Activations>().0, 1);
    assert!(!seen(&app).contains(&Key::Enter), "and consumed");
}

#[test]
fn a_key_upstream_does_not_bind_propagates() {
    let mut app = app();
    let form = form(&mut app);
    let ok = app.world_mut().spawn((button("ok"), ChildOf(form))).id();
    count_activations(&mut app, ok);
    set_focus(&mut app, ok);

    press_key(&mut app, character('x'));

    assert_eq!(app.world().resource::<Activations>().0, 0);
    assert_eq!(seen(&app), [character('x')]);
}
