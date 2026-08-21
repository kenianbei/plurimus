//! Which camera a widget draws on, resolved through the hierarchy.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{
    ComputedUiCamera, CorePlugin, TerminalCamera, TerminalSize, UiArea, UiCamera, UiWidget,
    Viewport,
};
use plurimus_ui::{ComputedWidgetArea, UiPlugin};
use ratatui_widgets::paragraph::Paragraph;

const LEFT: Rect = Rect::new(0, 0, 4, 1);
const RIGHT: Rect = Rect::new(4, 0, 4, 1);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, UiPlugin));
    app.insert_resource(TerminalSize::new(8, 1));
    app
}

fn spawn_camera(app: &mut App, viewport: Rect, order: isize) -> Entity {
    app.world_mut()
        .spawn(
            TerminalCamera::default()
                .with_order(order)
                .with_viewport(Viewport::Fixed(viewport)),
        )
        .id()
}

fn spawn_widget(app: &mut App) -> Entity {
    app.world_mut()
        .spawn(UiWidget::new(Paragraph::new("x")))
        .id()
}

fn camera_of(app: &App, entity: Entity) -> Option<Entity> {
    app.world().get::<ComputedUiCamera>(entity).unwrap().0
}

fn area_of(app: &App, entity: Entity) -> Rect {
    app.world().get::<ComputedWidgetArea>(entity).unwrap().0
}

#[test]
fn a_widget_with_no_camera_takes_the_default_one() {
    let mut app = app();
    let default_camera = spawn_camera(&mut app, LEFT, 0);
    let widget = spawn_widget(&mut app);

    app.update();

    assert_eq!(camera_of(&app, widget), Some(default_camera));
}

// Forgetting a child's UiCamera used to be a silent misplacement onto the
// default camera, which is the whole reason the hierarchy is consulted.
#[test]
fn a_child_inherits_the_camera_of_its_nearest_titled_ancestor() {
    let mut app = app();
    spawn_camera(&mut app, LEFT, 0);
    let right = spawn_camera(&mut app, RIGHT, 1);
    let parent = app.world_mut().spawn(UiCamera(right)).id();
    let child = spawn_widget(&mut app);
    let grandchild = spawn_widget(&mut app);
    app.world_mut().entity_mut(child).insert(ChildOf(parent));
    app.world_mut()
        .entity_mut(grandchild)
        .insert(ChildOf(child));

    app.update();

    assert_eq!(camera_of(&app, child), Some(right));
    assert_eq!(
        camera_of(&app, grandchild),
        Some(right),
        "the search passes through an ancestor that names no camera of its own"
    );
    assert_eq!(
        area_of(&app, child),
        RIGHT,
        "and the inherited camera is what the area resolves against"
    );
}

#[test]
fn an_explicit_camera_wins_over_an_ancestors() {
    let mut app = app();
    let left = spawn_camera(&mut app, LEFT, 0);
    let right = spawn_camera(&mut app, RIGHT, 1);
    let parent = app.world_mut().spawn(UiCamera(right)).id();
    let child = app
        .world_mut()
        .spawn((UiWidget::new(Paragraph::new("x")), UiCamera(left)))
        .id();
    app.world_mut().entity_mut(child).insert(ChildOf(parent));

    app.update();

    assert_eq!(camera_of(&app, child), Some(left));
}

// The camera is a binding rather than a value copied at spawn, so a parent
// that moves cameras takes its children with it.
#[test]
fn a_child_follows_its_parent_to_another_camera() {
    let mut app = app();
    let left = spawn_camera(&mut app, LEFT, 0);
    let right = spawn_camera(&mut app, RIGHT, 1);
    let parent = app.world_mut().spawn(UiCamera(left)).id();
    let child = spawn_widget(&mut app);
    app.world_mut().entity_mut(child).insert(ChildOf(parent));
    app.update();
    assert_eq!(camera_of(&app, child), Some(left));

    app.world_mut().entity_mut(parent).insert(UiCamera(right));
    app.update();

    assert_eq!(camera_of(&app, child), Some(right));
    assert_eq!(area_of(&app, child), RIGHT);
}

#[test]
fn a_widget_names_no_camera_when_none_is_active() {
    let mut app = app();
    let widget = spawn_widget(&mut app);

    app.update();

    assert_eq!(camera_of(&app, widget), None);
    assert_eq!(area_of(&app, widget), Rect::ZERO);
}

// An entity that only takes input - a scroll pane with no drawable of its
// own - resolves its area against a camera like any other.
#[test]
fn an_entity_with_an_area_but_no_widget_resolves_a_camera() {
    let mut app = app();
    let right = spawn_camera(&mut app, RIGHT, 1);
    spawn_camera(&mut app, LEFT, 0);
    let pane = app
        .world_mut()
        .spawn((
            ComputedWidgetArea::default(),
            UiArea::Fixed(Rect::new(0, 0, 2, 1)),
            UiCamera(right),
        ))
        .id();

    app.update();

    assert_eq!(camera_of(&app, pane), Some(right));
    assert_eq!(area_of(&app, pane), Rect::new(4, 0, 2, 1));
}
