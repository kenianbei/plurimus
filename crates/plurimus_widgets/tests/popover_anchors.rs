//! What a popover attaches to: a whole anchor, one cell of its content, and
//! which camera it is placed against.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::{
    Background, ComputedUiCamera, CorePlugin, TerminalCamera, TerminalSize, UiArea, UiCamera,
    UiWidget, Viewport,
};
use plurimus_ui::{ComputedWidgetArea, ScrollOffset};
use plurimus_widgets::{Popover, PopoverAlign, PopoverSide, WidgetsPlugin};
use ratatui_widgets::paragraph::Paragraph;

const FULL: Rect = Rect::new(0, 0, 20, 12);
const SHORT: Rect = Rect::new(0, 0, 20, 6);
const STRIP: Rect = Rect::new(0, 11, 20, 1);
const ANCHOR: Rect = Rect::new(2, 1, 10, 4);
const ON_STRIP: Rect = Rect::new(0, 0, 6, 1);
const CELL: Position = Position::new(3, 2);

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
                .with_viewport(Viewport::Fixed(viewport))
                .with_background(Background::Transparent),
        )
        .id()
}

fn spawn_anchor(app: &mut App, camera: Entity, area: Rect) -> Entity {
    app.world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("anchor")),
            UiArea::Fixed(area),
            UiCamera(camera),
        ))
        .id()
}

fn spawn_popover(app: &mut App, popover: Popover) -> Entity {
    app.world_mut()
        .spawn((popover, UiWidget::new(Paragraph::new("pop"))))
        .id()
}

fn placed(app: &App, popover: Entity) -> Rect {
    app.world().get::<ComputedWidgetArea>(popover).unwrap().0
}

fn camera_of(app: &App, entity: Entity) -> Option<Entity> {
    app.world().get::<ComputedUiCamera>(entity).unwrap().0
}

// A widget on a docked command strip, with a full-terminal camera above it
// to escape onto: everything the escape is for.
//
// The strip is the lowest-ordered camera and so the default one, which is
// what lets these tests tell the camera a popover was given from the camera
// everything falls back to.
struct Strip {
    full: Entity,
    anchor: Entity,
}

fn strip(app: &mut App) -> Strip {
    let docked = spawn_camera(app, STRIP, 0);
    Strip {
        full: spawn_camera(app, FULL, 1),
        anchor: spawn_anchor(app, docked, ON_STRIP),
    }
}

fn anchored(app: &mut App, viewport: Rect) -> Entity {
    let camera = spawn_camera(app, viewport, 0);
    spawn_anchor(app, camera, ANCHOR)
}

const fn opening_upward(anchor: Entity) -> Popover {
    Popover::new(anchor, Size::new(4, 5)).with_side(PopoverSide::Top)
}

// The cell case is the whole-rect case against a 1x1 anchor, which is what
// puts a completion list under a caret rather than under the editor.
#[test]
fn a_cell_anchor_opens_below_the_cell() {
    let mut app = app();
    let anchor = anchored(&mut app, FULL);
    let popover = spawn_popover(
        &mut app,
        Popover::new(anchor, Size::new(4, 2)).with_cell(CELL),
    );

    app.update();

    assert_eq!(placed(&app, popover), Rect::new(5, 4, 4, 2));
}

#[test]
fn a_cell_anchor_mirrors_above_when_below_overflows() {
    let mut app = app();
    let anchor = anchored(&mut app, SHORT);
    let popover = spawn_popover(
        &mut app,
        Popover::new(anchor, Size::new(4, 3)).with_cell(CELL),
    );

    app.update();

    let rect = placed(&app, popover);
    assert_eq!(rect, Rect::new(5, 0, 4, 3));
    assert_eq!(rect.bottom(), 3, "the popover ends on the cell's own row");
}

// The offset is applied here rather than by whoever set the cell, so an
// editor names its caret once, in the content space it already thinks in.
#[test]
fn a_cell_anchor_follows_the_scroll_offset() {
    let mut app = app();
    let anchor = anchored(&mut app, FULL);
    app.world_mut()
        .entity_mut(anchor)
        .insert(ScrollOffset(Position::new(0, 1)));
    let popover = spawn_popover(
        &mut app,
        Popover::new(anchor, Size::new(4, 2)).with_cell(CELL),
    );

    app.update();

    assert_eq!(placed(&app, popover), Rect::new(5, 3, 4, 2));
}

#[test]
fn a_cell_scrolled_out_of_view_places_nothing() {
    let mut app = app();
    let anchor = anchored(&mut app, FULL);
    app.world_mut()
        .entity_mut(anchor)
        .insert(ScrollOffset(Position::new(0, 3)));
    let popover = spawn_popover(
        &mut app,
        Popover::new(anchor, Size::new(4, 2)).with_cell(CELL),
    );

    app.update();

    assert_eq!(placed(&app, popover), Rect::ZERO);
}

#[test]
fn a_cell_anchor_aligns_on_the_cell() {
    let mut app = app();
    let anchor = anchored(&mut app, FULL);
    let aligned = |app: &mut App, align| {
        let entity = spawn_popover(
            app,
            Popover::new(anchor, Size::new(3, 2))
                .with_cell(CELL)
                .with_align(align),
        );
        app.update();
        placed(app, entity)
    };

    assert_eq!(
        aligned(&mut app, PopoverAlign::Center),
        Rect::new(4, 4, 3, 2)
    );
    assert_eq!(aligned(&mut app, PopoverAlign::End), Rect::new(3, 4, 3, 2));
}

// The `None` path: naming no cell attaches to the whole area, as it always
// has, and a scroll offset on the anchor says nothing about it.
#[test]
fn a_whole_rect_anchor_ignores_the_offset() {
    let mut app = app();
    let anchor = anchored(&mut app, FULL);
    app.world_mut()
        .entity_mut(anchor)
        .insert(ScrollOffset(Position::new(0, 3)));
    let popover = spawn_popover(&mut app, Popover::new(anchor, Size::new(4, 2)));

    app.update();

    assert_eq!(placed(&app, popover), Rect::new(2, 5, 4, 2));
}

#[test]
fn a_popover_draws_on_the_camera_it_names() {
    let mut app = app();
    let strip = strip(&mut app);
    let popover = spawn_popover(
        &mut app,
        opening_upward(strip.anchor).with_camera(strip.full),
    );
    let child = app
        .world_mut()
        .spawn((UiWidget::new(Paragraph::new("in")), ChildOf(popover)))
        .id();

    // Two frames: adoption writes a real `UiCamera`, and the propagation
    // that carries it to the children ran earlier in the same one.
    app.update();
    app.update();

    assert_eq!(camera_of(&app, popover), Some(strip.full));
    assert_eq!(camera_of(&app, child), Some(strip.full));
}

// The whole point of naming a camera: a ten-row box anchored to a one-row
// strip has nowhere to be within that row, and everywhere to be above it.
#[test]
fn a_named_camera_is_the_clamp_bound() {
    let mut app = app();
    let strip = strip(&mut app);
    let escaped = spawn_popover(
        &mut app,
        opening_upward(strip.anchor).with_camera(strip.full),
    );
    let confined = spawn_popover(&mut app, opening_upward(strip.anchor));

    app.update();

    assert_eq!(placed(&app, escaped), Rect::new(0, 6, 4, 5));
    assert_eq!(
        placed(&app, confined),
        Rect::new(0, 11, 4, 1),
        "without a camera of its own it is still flattened into the strip"
    );
}

// The two fields compose because the anchor's rect is resolved in screen
// space before any camera is consulted: a cell means the same thing
// wherever the popover ends up being drawn.
#[test]
fn a_cell_anchor_and_a_named_camera_compose() {
    let mut app = app();
    let strip = strip(&mut app);
    let popover = spawn_popover(
        &mut app,
        Popover::new(strip.anchor, Size::new(4, 5))
            .with_side(PopoverSide::Top)
            .with_cell(Position::new(2, 0))
            .with_camera(strip.full),
    );

    app.update();

    assert_eq!(placed(&app, popover), Rect::new(2, 6, 4, 5));
}

// Adoption is the anchor's to grant: a popover with nowhere to be placed
// takes no camera either, rather than holding a `UiCamera` for a frame that
// never draws it.
#[test]
fn a_popover_with_no_anchor_adopts_nothing() {
    let mut app = app();
    let default_camera = spawn_camera(&mut app, FULL, 0);
    let docked = spawn_camera(&mut app, STRIP, 1);
    let popover = spawn_popover(
        &mut app,
        Popover::new(Entity::PLACEHOLDER, Size::new(4, 2)).with_camera(docked),
    );

    app.update();

    assert_eq!(camera_of(&app, popover), Some(default_camera));
}

#[test]
fn a_named_camera_without_a_viewport_places_nothing() {
    let mut app = app();
    let strip = strip(&mut app);
    let popover = spawn_popover(
        &mut app,
        opening_upward(strip.anchor).with_camera(strip.full),
    );
    app.update();
    app.world_mut().entity_mut(strip.full).despawn();

    app.update();

    assert_eq!(placed(&app, popover), Rect::ZERO);
    assert_eq!(
        camera_of(&app, popover),
        Some(strip.full),
        "the camera it asked for is still what it says it draws on"
    );
}
