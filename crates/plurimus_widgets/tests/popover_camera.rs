//! Which camera an anchored popover, and a menu built from one, draws on.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::{Changed, Query, ResMut, Resource, With};
use plurimus_core::ratatui_core::layout::{Rect, Size};
use plurimus_core::{
    ComputedUiCamera, CorePlugin, TerminalCamera, TerminalSize, UiArea, UiCamera, UiWidget,
    Viewport,
};
use plurimus_ui::ComputedWidgetArea;
use plurimus_widgets::{Popover, WidgetsPlugin, menu_item, menu_popup};
use ratatui_widgets::paragraph::Paragraph;

const MAIN: Rect = Rect::new(0, 0, 20, 6);
const SIDE: Rect = Rect::new(0, 6, 20, 6);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 12));
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

fn camera_of(app: &App, entity: Entity) -> Option<Entity> {
    app.world().get::<ComputedUiCamera>(entity).unwrap().0
}

fn spawn_anchor(app: &mut App, camera: Entity) -> Entity {
    app.world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("anchor")),
            UiArea::Fixed(Rect::new(0, 0, 6, 1)),
            UiCamera(camera),
        ))
        .id()
}

// A popover is placed against something it need not be parented to, so the
// hierarchy cannot tell it which camera to use - the anchor does.
#[test]
fn a_popover_with_no_parent_draws_on_its_anchors_camera() {
    let mut app = app();
    spawn_camera(&mut app, MAIN, 0);
    let side = spawn_camera(&mut app, SIDE, 1);
    let anchor = spawn_anchor(&mut app, side);
    let popover = app
        .world_mut()
        .spawn((
            Popover::new(anchor, Size::new(4, 2)),
            UiWidget::new(Paragraph::new("pop")),
        ))
        .id();

    app.update();

    assert_eq!(camera_of(&app, popover), Some(side));
}

// The popover's camera is a real one, so its own children inherit it the
// way they inherit any camera - no second copier per widget family.
#[test]
fn a_popovers_children_inherit_the_anchored_camera() {
    let mut app = app();
    spawn_camera(&mut app, MAIN, 0);
    let side = spawn_camera(&mut app, SIDE, 1);
    let anchor = spawn_anchor(&mut app, side);
    let popover = app
        .world_mut()
        .spawn((
            Popover::new(anchor, Size::new(4, 2)),
            UiWidget::new(Paragraph::new("pop")),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((UiWidget::new(Paragraph::new("in")), ChildOf(popover)))
        .id();

    app.update();
    app.update();

    assert_eq!(camera_of(&app, child), Some(side));
}

#[test]
fn a_popover_follows_its_anchor_to_another_camera() {
    let mut app = app();
    let main = spawn_camera(&mut app, MAIN, 0);
    let side = spawn_camera(&mut app, SIDE, 1);
    let anchor = spawn_anchor(&mut app, main);
    let popover = app
        .world_mut()
        .spawn((
            Popover::new(anchor, Size::new(4, 2)),
            UiWidget::new(Paragraph::new("pop")),
        ))
        .id();
    app.update();
    assert_eq!(camera_of(&app, popover), Some(main));

    app.world_mut().entity_mut(anchor).insert(UiCamera(side));
    app.update();

    assert_eq!(camera_of(&app, popover), Some(side));
}

// Menu items are children of the popup, so they take the camera the popup
// took from its anchor rather than the one the hierarchy alone would give.
#[test]
fn menu_items_take_the_popups_anchored_camera() {
    let mut app = app();
    spawn_camera(&mut app, MAIN, 0);
    let side = spawn_camera(&mut app, SIDE, 1);
    let anchor = spawn_anchor(&mut app, side);
    let popup = app.world_mut().spawn(menu_popup(anchor)).id();
    let item = app.world_mut().spawn(menu_item("one")).id();
    app.world_mut().entity_mut(item).insert(ChildOf(popup));

    app.update();
    app.update();

    assert_eq!(camera_of(&app, popup), Some(side));
    assert_eq!(camera_of(&app, item), Some(side));
}

#[derive(Resource, Default)]
struct AreaChanges(usize);

// Two systems write a popover's placement each frame; if they disagree, all
// the work gated on Changed<ComputedWidgetArea> reruns forever.
#[test]
fn a_placed_popover_settles_instead_of_flipping_every_frame() {
    let mut app = app();
    spawn_camera(&mut app, MAIN, 0);
    let side = spawn_camera(&mut app, SIDE, 1);
    let anchor = spawn_anchor(&mut app, side);
    app.world_mut().spawn((
        Popover::new(anchor, Size::new(4, 2)),
        UiWidget::new(Paragraph::new("pop")),
    ));
    app.init_resource::<AreaChanges>();
    app.add_systems(
        bevy_app::Update,
        |moved: Query<(), (Changed<ComputedWidgetArea>, With<Popover>)>,
         mut count: ResMut<AreaChanges>| {
            count.0 += moved.iter().count();
        },
    );

    for _ in 0..4 {
        app.update();
    }
    let settled = app.world().resource::<AreaChanges>().0;
    app.update();
    app.update();

    assert_eq!(
        app.world().resource::<AreaChanges>().0,
        settled,
        "an idle frame must not move a placed popover"
    );
}
