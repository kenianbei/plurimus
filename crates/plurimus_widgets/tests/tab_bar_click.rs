//! Clicking a tab: the event it emits on the bar, and where focus goes.
//! Before any widget holds focus the virtual window does, so "the bar did
//! not take focus" is the assertion, never `None`.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input_focus::InputFocus;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::click;
use plurimus_ui::{Checked, InteractionDisabled, PressFocusDisabled, UiArea, ValueChange};
use plurimus_widgets::{WidgetsPlugin, tab_bar, tab_bar_self_update, tab_item};

#[derive(Resource, Default)]
struct Activated(Vec<Entity>);

struct Bar {
    bar: Entity,
    items: Vec<Entity>,
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 3));
    app.world_mut().spawn(TerminalCamera::default());
    app.init_resource::<Activated>();
    app
}

/// `a` at cells 0..3, `b` at 3..6, `b` active.
fn spawn_bar(app: &mut App) -> Bar {
    let bar = app
        .world_mut()
        .spawn((tab_bar(), UiArea::Fixed(Rect::new(0, 0, 20, 1))))
        .id();
    app.world_mut()
        .entity_mut(bar)
        .observe(
            |change: On<ValueChange<Entity>>, mut seen: ResMut<Activated>| {
                seen.0.push(change.value);
            },
        )
        .observe(tab_bar_self_update);
    let items: Vec<Entity> = ["a", "b"]
        .iter()
        .map(|label| app.world_mut().spawn((tab_item(*label), ChildOf(bar))).id())
        .collect();
    app.world_mut().entity_mut(items[1]).insert(Checked);
    app.update();
    Bar { bar, items }
}

fn activated(app: &App) -> Vec<Entity> {
    app.world().resource::<Activated>().0.clone()
}

fn focused(app: &App) -> Option<Entity> {
    app.world().resource::<InputFocus>().get()
}

#[test]
fn a_click_activates_the_item_and_focuses_the_bar() {
    let mut app = app();
    let bar = spawn_bar(&mut app);

    click(&mut app, 1, 0);
    app.update();

    assert_eq!(activated(&app), [bar.items[0]]);
    assert_eq!(focused(&app), Some(bar.bar));
    assert!(app.world().get::<Checked>(bar.items[0]).is_some());
    assert!(app.world().get::<Checked>(bar.items[1]).is_none());
}

#[test]
fn clicking_the_active_item_activates_it_again() {
    let mut app = app();
    let bar = spawn_bar(&mut app);

    click(&mut app, 4, 0);

    assert_eq!(activated(&app), [bar.items[1]]);
}

#[test]
fn press_focus_disabled_on_the_bar_leaves_focus_alone() {
    let mut app = app();
    let bar = spawn_bar(&mut app);
    app.world_mut()
        .entity_mut(bar.bar)
        .insert(PressFocusDisabled);

    click(&mut app, 1, 0);

    assert_eq!(activated(&app), [bar.items[0]]);
    assert_ne!(focused(&app), Some(bar.bar));
}

#[test]
fn a_disabled_item_activates_nothing() {
    let mut app = app();
    let bar = spawn_bar(&mut app);
    app.world_mut()
        .entity_mut(bar.items[0])
        .insert(InteractionDisabled);
    app.update();

    click(&mut app, 1, 0);

    assert!(activated(&app).is_empty());
}

#[test]
fn a_disabled_bar_activates_nothing() {
    let mut app = app();
    let bar = spawn_bar(&mut app);
    app.world_mut()
        .entity_mut(bar.bar)
        .insert(InteractionDisabled);
    app.update();

    click(&mut app, 1, 0);

    assert!(activated(&app).is_empty());
    assert_ne!(focused(&app), Some(bar.bar));
}
