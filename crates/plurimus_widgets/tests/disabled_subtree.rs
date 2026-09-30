//! `InteractionDisabled` disables the subtree beneath it on every path a
//! widget takes input by: the press, its keys, and directional navigation.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input_focus::InputFocus;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::KeyCode;
use plurimus_test::{click, press_at, press_key, set_focus};
use plurimus_ui::{ComputedDisabled, InteractionDisabled, Pressed, UiArea};
use plurimus_widgets::{Activate, WidgetsPlugin, button, tab_bar, tab_item};

#[derive(Resource, Default)]
struct Activations(u32);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 1));
    app.world_mut().spawn(TerminalCamera::default());
    app.init_resource::<Activations>();
    app
}

fn spawn_button(app: &mut App, x: u16) -> Entity {
    app.world_mut()
        .spawn((button("go"), UiArea::Fixed(Rect::new(x, 0, 6, 1))))
        .id()
}

fn focused(app: &App) -> Option<Entity> {
    app.world().resource::<InputFocus>().get()
}

fn activations(app: &App) -> u32 {
    app.world().resource::<Activations>().0
}

/// Presses, activates by key, and reaches by an arrow from `outer`,
/// returning how many of the three got through.
fn try_every_path(app: &mut App, inner: Entity, outer: Entity) -> u32 {
    let before = activations(app);
    click(app, 1, 0);
    let pressed = activations(app) - before;
    set_focus(app, inner);
    press_key(app, KeyCode::Enter);
    let keyed = activations(app) - before - pressed;
    set_focus(app, outer);
    press_key(app, KeyCode::Left);
    pressed + keyed + u32::from(focused(app) == Some(inner))
}

#[test]
fn a_button_in_a_disabled_container_is_inert_until_it_is_enabled() {
    let mut app = app();
    let container = app.world_mut().spawn(InteractionDisabled).id();
    let inner = spawn_button(&mut app, 0);
    app.world_mut().entity_mut(inner).insert(ChildOf(container));
    app.world_mut()
        .entity_mut(inner)
        .observe(|_: On<Activate>, mut seen: ResMut<Activations>| seen.0 += 1);
    let outer = spawn_button(&mut app, 8);
    app.update();

    assert_eq!(try_every_path(&mut app, inner, outer), 0);
    assert!(app.world().get::<ComputedDisabled>(inner).is_some());

    app.world_mut()
        .entity_mut(container)
        .remove::<InteractionDisabled>();
    app.update();

    assert_eq!(try_every_path(&mut app, inner, outer), 3);
}

#[test]
fn an_item_of_a_disabled_bar_is_never_pressed() {
    let mut app = app();
    let bar = app
        .world_mut()
        .spawn((
            tab_bar(),
            UiArea::Fixed(Rect::new(0, 0, 20, 1)),
            InteractionDisabled,
        ))
        .id();
    let item = app.world_mut().spawn((tab_item("a"), ChildOf(bar))).id();
    app.update();

    press_at(&mut app, 1, 0);

    assert!(app.world().get::<Pressed>(item).is_none());
}
