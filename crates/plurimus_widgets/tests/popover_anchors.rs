//! What a popover attaches to: a whole anchor, one cell of its content, and
//! which camera it is placed against.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::{
    Background, CorePlugin, TerminalCamera, TerminalSize, UiArea, UiCamera, UiWidget, Viewport,
};
use plurimus_ui::{ComputedWidgetArea, ScrollOffset};
use plurimus_widgets::{Popover, PopoverAlign, WidgetsPlugin};
use ratatui_widgets::paragraph::Paragraph;

const FULL: Rect = Rect::new(0, 0, 20, 12);
const SHORT: Rect = Rect::new(0, 0, 20, 6);
const ANCHOR: Rect = Rect::new(2, 1, 10, 4);
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

// The cell case is the whole-rect case against a 1x1 anchor, which is what
// puts a completion list under a caret rather than under the editor.
#[test]
fn a_cell_anchor_opens_below_the_cell() {
    let mut app = app();
    let camera = spawn_camera(&mut app, FULL, 0);
    let anchor = spawn_anchor(&mut app, camera, ANCHOR);
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
    let camera = spawn_camera(&mut app, SHORT, 0);
    let anchor = spawn_anchor(&mut app, camera, ANCHOR);
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
    let camera = spawn_camera(&mut app, FULL, 0);
    let anchor = spawn_anchor(&mut app, camera, ANCHOR);
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
    let camera = spawn_camera(&mut app, FULL, 0);
    let anchor = spawn_anchor(&mut app, camera, ANCHOR);
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
    let camera = spawn_camera(&mut app, FULL, 0);
    let anchor = spawn_anchor(&mut app, camera, ANCHOR);
    let aligned = |app: &mut App, align| {
        let mut popover = Popover::new(anchor, Size::new(3, 2)).with_cell(CELL);
        popover.align = align;
        let entity = spawn_popover(app, popover);
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
    let camera = spawn_camera(&mut app, FULL, 0);
    let anchor = spawn_anchor(&mut app, camera, ANCHOR);
    app.world_mut()
        .entity_mut(anchor)
        .insert(ScrollOffset(Position::new(0, 3)));
    let popover = spawn_popover(&mut app, Popover::new(anchor, Size::new(4, 2)));

    app.update();

    assert_eq!(placed(&app, popover), Rect::new(2, 5, 4, 2));
}
