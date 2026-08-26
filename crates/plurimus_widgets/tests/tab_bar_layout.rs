//! Where a tab bar puts its items: the rects the bar assigns, the order it
//! draws them at, and what happens when they do not fit.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::ChildOf;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize, UiHidden, UiOrder};
use plurimus_ui::{ComputedWidgetArea, UiArea, UiLabel};
use plurimus_widgets::{TabBarLook, TabBarOrientation, WidgetsPlugin, tab_bar, tab_item};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(30, 6));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_bar(
    app: &mut App,
    area: Rect,
    look: TabBarLook,
    labels: &[&'static str],
) -> (Entity, Vec<Entity>) {
    let bar = app
        .world_mut()
        .spawn((tab_bar(), look, UiArea::Fixed(area)))
        .id();
    let items = labels
        .iter()
        .map(|label| app.world_mut().spawn((tab_item(*label), ChildOf(bar))).id())
        .collect();
    (bar, items)
}

fn computed(app: &App, entity: Entity) -> Rect {
    app.world().get::<ComputedWidgetArea>(entity).unwrap().0
}

fn local(app: &App, entity: Entity) -> UiArea {
    *app.world().get::<UiArea>(entity).unwrap()
}

#[test]
fn items_are_placed_along_the_bar_in_child_order() {
    let mut app = app();
    let (_, items) = spawn_bar(
        &mut app,
        Rect::new(2, 1, 20, 1),
        TabBarLook::default(),
        &["Diary", "Plan"],
    );
    app.update();

    assert_eq!(computed(&app, items[0]), Rect::new(2, 1, 7, 1));
    assert_eq!(computed(&app, items[1]), Rect::new(9, 1, 6, 1));
    assert_eq!(local(&app, items[0]), UiArea::Fixed(Rect::new(2, 1, 7, 1)));
}

#[test]
fn items_draw_one_order_above_their_bar() {
    let mut app = app();
    let (bar, items) = spawn_bar(
        &mut app,
        Rect::new(0, 0, 20, 1),
        TabBarLook::default(),
        &["Diary"],
    );
    app.world_mut().entity_mut(bar).insert(UiOrder(5));
    app.update();

    assert_eq!(app.world().get::<UiOrder>(items[0]), Some(&UiOrder(6)));
}

#[test]
fn an_item_that_does_not_fit_is_nowhere() {
    let mut app = app();
    let (_, items) = spawn_bar(
        &mut app,
        Rect::new(0, 0, 12, 1),
        TabBarLook::default(),
        &["Diary", "Plan", "Foods"],
    );
    app.update();

    assert_eq!(computed(&app, items[1]), Rect::ZERO);
    assert_eq!(computed(&app, items[2]), Rect::ZERO);
}

#[test]
fn editing_a_label_moves_its_neighbour() {
    let mut app = app();
    let (_, items) = spawn_bar(
        &mut app,
        Rect::new(0, 0, 30, 1),
        TabBarLook::default(),
        &["Diary", "Plan"],
    );
    app.update();
    assert_eq!(computed(&app, items[1]).x, 7);

    app.world_mut()
        .entity_mut(items[0])
        .insert(UiLabel("Journal".into()));
    app.update();

    assert_eq!(computed(&app, items[1]).x, 9);
}

#[test]
fn a_hidden_bar_places_its_items_nowhere() {
    let mut app = app();
    let (bar, items) = spawn_bar(
        &mut app,
        Rect::new(0, 0, 30, 1),
        TabBarLook::default(),
        &["Diary"],
    );
    app.update();
    assert_ne!(computed(&app, items[0]), Rect::ZERO);

    app.world_mut().entity_mut(bar).insert(UiHidden);
    app.update();

    assert_eq!(computed(&app, items[0]), Rect::ZERO);
}

#[test]
fn vertical_items_stack_at_the_bar_width() {
    let mut app = app();
    let (_, items) = spawn_bar(
        &mut app,
        Rect::new(1, 1, 12, 4),
        TabBarLook::default().with_orientation(TabBarOrientation::Vertical),
        &["Diary", "Plan"],
    );
    app.update();

    assert_eq!(computed(&app, items[0]), Rect::new(1, 1, 12, 1));
    assert_eq!(computed(&app, items[1]), Rect::new(1, 2, 12, 1));
}
