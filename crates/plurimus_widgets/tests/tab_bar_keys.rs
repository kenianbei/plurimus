//! `TabBarKeys`: what each binding activates, where stepping stops, what
//! a repeat does, and what propagates past the bar.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::{FocusCause, FocusedInput, InputFocus};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::KeyCode;
use plurimus_test::{press_key, repeat_key};
use plurimus_ui::{Checked, InteractionDisabled, UiArea, ValueChange};
use plurimus_widgets::{
    TabBarAction, TabBarKeys, WidgetsPlugin, tab_bar, tab_bar_self_update, tab_item,
};

#[derive(Resource, Default)]
struct Activated(Vec<Entity>);

#[derive(Resource, Default)]
struct Unconsumed(Vec<Key>);

struct Bar {
    bar: Entity,
    items: Vec<Entity>,
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(40, 3));
    app.world_mut().spawn(TerminalCamera::default());
    app.init_resource::<Activated>();
    app.init_resource::<Unconsumed>();
    app
}

/// A bar inside a form that records what propagates past it, with
/// `active` checked and every activation recorded and applied.
fn spawn_bar(app: &mut App, labels: &[&'static str], active: Option<usize>) -> Bar {
    let form = app.world_mut().spawn_empty().id();
    app.world_mut().entity_mut(form).observe(
        |input: On<FocusedInput<KeyboardInput>>, mut seen: ResMut<Unconsumed>| {
            seen.0.push(input.input.logical_key.clone());
        },
    );
    let bar = app
        .world_mut()
        .spawn((
            tab_bar(),
            UiArea::Fixed(Rect::new(0, 0, 40, 1)),
            ChildOf(form),
        ))
        .id();
    app.world_mut()
        .entity_mut(bar)
        .observe(
            |change: On<ValueChange<Entity>>, mut seen: ResMut<Activated>| {
                seen.0.push(change.value);
            },
        )
        .observe(tab_bar_self_update);
    let items: Vec<Entity> = labels
        .iter()
        .map(|label| app.world_mut().spawn((tab_item(*label), ChildOf(bar))).id())
        .collect();
    if let Some(index) = active {
        app.world_mut().entity_mut(items[index]).insert(Checked);
    }
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(bar, FocusCause::Navigated);
    app.update();
    Bar { bar, items }
}

fn activated(app: &App) -> Vec<Entity> {
    app.world().resource::<Activated>().0.clone()
}

fn unconsumed(app: &App) -> Vec<Key> {
    app.world().resource::<Unconsumed>().0.clone()
}

fn is_checked(app: &App, item: Entity) -> bool {
    app.world().get::<Checked>(item).is_some()
}

#[test]
fn arrows_step_on_both_axes_and_home_end_jump() {
    let mut app = app();
    let bar = spawn_bar(&mut app, &["a", "b", "c"], Some(1));

    press_key(&mut app, KeyCode::Right);
    press_key(&mut app, KeyCode::Up);
    press_key(&mut app, KeyCode::Home);
    press_key(&mut app, KeyCode::End);
    press_key(&mut app, KeyCode::Left);
    press_key(&mut app, KeyCode::Down);

    assert_eq!(
        activated(&app),
        [
            bar.items[2],
            bar.items[1],
            bar.items[0],
            bar.items[2],
            bar.items[1],
            bar.items[2]
        ]
    );
    assert!(is_checked(&app, bar.items[2]));
    assert!(unconsumed(&app).is_empty());
}

#[test]
fn stepping_stops_at_the_ends_and_still_consumes() {
    let mut app = app();
    let bar = spawn_bar(&mut app, &["a", "b"], Some(0));

    press_key(&mut app, KeyCode::Left);
    press_key(&mut app, KeyCode::Home);
    press_key(&mut app, KeyCode::Right);
    press_key(&mut app, KeyCode::Right);

    assert_eq!(activated(&app), [bar.items[1]]);
    assert!(unconsumed(&app).is_empty());
}

#[test]
fn next_with_nothing_active_lands_on_the_first() {
    let mut app = app();
    let bar = spawn_bar(&mut app, &["a", "b"], None);

    press_key(&mut app, KeyCode::Left);
    press_key(&mut app, KeyCode::Right);

    assert_eq!(activated(&app), [bar.items[0]]);
}

#[test]
fn a_disabled_item_is_stepped_over() {
    let mut app = app();
    let bar = spawn_bar(&mut app, &["a", "b", "c"], Some(0));
    app.world_mut()
        .entity_mut(bar.items[1])
        .insert(InteractionDisabled);

    press_key(&mut app, KeyCode::Right);

    assert_eq!(activated(&app), [bar.items[2]]);
}

#[test]
fn a_held_arrow_keeps_stepping() {
    let mut app = app();
    let bar = spawn_bar(&mut app, &["a", "b", "c"], Some(0));

    press_key(&mut app, KeyCode::Right);
    repeat_key(&mut app, KeyCode::Right);

    assert_eq!(activated(&app), [bar.items[1], bar.items[2]]);
}

fn remap(app: &mut App, bar: &Bar) {
    app.world_mut().entity_mut(bar.bar).insert(TabBarKeys(vec![
        (Key::Character("[".into()).into(), TabBarAction::Previous),
        (Key::Character("]".into()).into(), TabBarAction::Next),
        (Key::Character("1".into()).into(), TabBarAction::Select(0)),
        (Key::Character("2".into()).into(), TabBarAction::Select(1)),
        (Key::Character("9".into()).into(), TabBarAction::Select(8)),
    ]));
}

#[test]
fn a_remapped_table_drives_the_bar_and_the_arrows_propagate() {
    let mut app = app();
    let bar = spawn_bar(&mut app, &["a", "b"], Some(0));
    remap(&mut app, &bar);

    press_key(&mut app, KeyCode::Char(']'));
    press_key(&mut app, KeyCode::Char('['));
    press_key(&mut app, KeyCode::Right);

    assert_eq!(activated(&app), [bar.items[1], bar.items[0]]);
    assert_eq!(unconsumed(&app), [Key::ArrowRight]);
}

#[test]
fn select_activates_its_item_every_press_and_never_on_a_repeat() {
    let mut app = app();
    let bar = spawn_bar(&mut app, &["a", "b"], Some(0));
    remap(&mut app, &bar);

    press_key(&mut app, KeyCode::Char('2'));
    press_key(&mut app, KeyCode::Char('2'));
    repeat_key(&mut app, KeyCode::Char('2'));

    assert_eq!(activated(&app), [bar.items[1], bar.items[1]]);
    assert!(unconsumed(&app).is_empty());
}

#[test]
fn select_past_the_last_item_activates_nothing_and_propagates() {
    let mut app = app();
    let bar = spawn_bar(&mut app, &["a", "b"], Some(0));
    remap(&mut app, &bar);

    press_key(&mut app, KeyCode::Char('9'));

    assert!(activated(&app).is_empty());
    assert_eq!(unconsumed(&app), [Key::Character("9".into())]);
}

#[test]
fn a_disabled_bar_consumes_nothing() {
    let mut app = app();
    let bar = spawn_bar(&mut app, &["a", "b"], Some(0));
    app.world_mut()
        .entity_mut(bar.bar)
        .insert(InteractionDisabled);

    press_key(&mut app, KeyCode::Right);

    assert!(activated(&app).is_empty());
    assert_eq!(unconsumed(&app), [Key::ArrowRight]);
}
