//! A tab bar's keys, upstream's: the arrows along the bar move focus among
//! the tabs, wrapping, Enter or space select the focused one, and what the
//! bar does not bind propagates past it.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::{FocusedInput, InputFocus};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::{character, press_key, repeat_key, set_focus};
use plurimus_ui::{InteractionDisabled, Selected, UiArea, ValueChange};
use plurimus_widgets::{
    ControlOrientation, SelectedTab, TabActivation, TabList, WidgetsPlugin, tab_bar, tab_item,
    tablist_self_update,
};

#[derive(Resource, Default)]
struct Requested(Vec<Option<Entity>>);

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
    app.init_resource::<Requested>();
    app.init_resource::<Unconsumed>();
    app
}

/// A bar inside a form that records what propagates past it, with the tab
/// at `selected` selected and focused, and every request recorded and
/// applied.
fn spawn_bar(app: &mut App, labels: &[&'static str], list: TabList, selected: usize) -> Bar {
    let form = app.world_mut().spawn_empty().id();
    app.world_mut().entity_mut(form).observe(
        |input: On<FocusedInput<KeyboardInput>>, mut seen: ResMut<Unconsumed>| {
            if input.input.state.is_pressed() {
                seen.0.push(input.input.logical_key.clone());
            }
        },
    );
    let bar = app
        .world_mut()
        .spawn((
            tab_bar(),
            list,
            UiArea::Fixed(Rect::new(0, 0, 40, 3)),
            ChildOf(form),
        ))
        .id();
    app.world_mut()
        .entity_mut(bar)
        .observe(
            |change: On<ValueChange<Option<Entity>>>, mut seen: ResMut<Requested>| {
                seen.0.push(change.value);
            },
        )
        .observe(tablist_self_update);
    let items: Vec<Entity> = labels
        .iter()
        .map(|label| app.world_mut().spawn((tab_item(*label), ChildOf(bar))).id())
        .collect();
    app.world_mut()
        .entity_mut(bar)
        .insert(SelectedTab(Some(items[selected])));
    set_focus(app, items[selected]);
    app.update();
    Bar { bar, items }
}

fn horizontal(app: &mut App, labels: &[&'static str], selected: usize) -> Bar {
    spawn_bar(app, labels, TabList::default(), selected)
}

fn requested(app: &App) -> Vec<Option<Entity>> {
    app.world().resource::<Requested>().0.clone()
}

fn unconsumed(app: &App) -> Vec<Key> {
    app.world().resource::<Unconsumed>().0.clone()
}

fn focused(app: &App) -> Option<Entity> {
    app.world().resource::<InputFocus>().get()
}

#[test]
fn the_arrows_along_the_bar_move_focus_and_wrap() {
    let mut app = app();
    let bar = horizontal(&mut app, &["a", "b", "c"], 1);

    press_key(&mut app, Key::ArrowRight);
    assert_eq!(focused(&app), Some(bar.items[2]));
    press_key(&mut app, Key::ArrowRight);
    assert_eq!(
        focused(&app),
        Some(bar.items[0]),
        "Right wraps to the first"
    );
    press_key(&mut app, Key::ArrowLeft);
    assert_eq!(focused(&app), Some(bar.items[2]), "and Left to the last");
    press_key(&mut app, Key::Home);
    assert_eq!(focused(&app), Some(bar.items[0]));
    press_key(&mut app, Key::End);
    assert_eq!(focused(&app), Some(bar.items[2]));

    assert_eq!(
        requested(&app),
        [],
        "a manual bar selects nothing by moving"
    );
    assert_eq!(unconsumed(&app), []);
}

#[test]
fn enter_or_space_selects_the_focused_tab_once() {
    let mut app = app();
    let bar = horizontal(&mut app, &["a", "b", "c"], 0);

    press_key(&mut app, Key::ArrowRight);
    press_key(&mut app, Key::Enter);
    press_key(&mut app, character(' '));
    app.update();

    assert_eq!(
        requested(&app),
        [Some(bar.items[1])],
        "space on the tab Enter selected requests nothing more"
    );
    assert!(app.world().get::<Selected>(bar.items[1]).is_some());
    assert!(app.world().get::<Selected>(bar.items[0]).is_none());
}

#[test]
fn an_automatic_bar_selects_as_focus_moves() {
    let mut app = app();
    let list = TabList {
        activation: TabActivation::Automatic,
        ..TabList::default()
    };
    let bar = spawn_bar(&mut app, &["a", "b"], list, 0);

    press_key(&mut app, Key::ArrowRight);

    assert_eq!(requested(&app), [Some(bar.items[1])]);
}

#[test]
fn the_cross_axis_arrows_propagate() {
    let mut app = app();
    let bar = horizontal(&mut app, &["a", "b"], 0);

    press_key(&mut app, Key::ArrowDown);
    press_key(&mut app, Key::ArrowUp);

    assert_eq!(unconsumed(&app), [Key::ArrowDown, Key::ArrowUp]);
    assert_eq!(focused(&app), Some(bar.items[0]));
}

#[test]
fn a_vertical_bar_steps_on_up_and_down() {
    let mut app = app();
    let list = TabList {
        orientation: ControlOrientation::Vertical,
        ..TabList::default()
    };
    let bar = spawn_bar(&mut app, &["a", "b"], list, 0);

    press_key(&mut app, Key::ArrowDown);
    assert_eq!(focused(&app), Some(bar.items[1]));
    press_key(&mut app, Key::ArrowRight);
    assert_eq!(unconsumed(&app), [Key::ArrowRight]);
}

#[test]
fn a_disabled_tab_is_stepped_over() {
    let mut app = app();
    let bar = horizontal(&mut app, &["a", "b", "c"], 0);
    app.world_mut()
        .entity_mut(bar.items[1])
        .insert(InteractionDisabled);
    app.update();

    press_key(&mut app, Key::ArrowRight);

    assert_eq!(focused(&app), Some(bar.items[2]));
}

#[test]
fn a_held_arrow_steps_once() {
    let mut app = app();
    let bar = horizontal(&mut app, &["a", "b", "c"], 0);

    press_key(&mut app, Key::ArrowRight);
    repeat_key(&mut app, Key::ArrowRight);

    assert_eq!(focused(&app), Some(bar.items[1]));
}

#[test]
fn a_disabled_bar_consumes_nothing() {
    let mut app = app();
    let bar = horizontal(&mut app, &["a", "b"], 0);
    app.world_mut()
        .entity_mut(bar.bar)
        .insert(InteractionDisabled);
    app.update();

    press_key(&mut app, Key::ArrowRight);

    assert_eq!(requested(&app), []);
    assert_eq!(unconsumed(&app), [Key::ArrowRight]);
}
