//! What a pointer inside an open modal overlay may reach.
//!
//! The guard answers "inside" from the overlay's own rect, so an overlay
//! confines the pointer to its subtree instead of relying on every child
//! being marked as modal.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::{On, ResMut, Resource};
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize, UiWidget};
use plurimus_term::{MouseButton, MouseKind};
use plurimus_test::{click, send_mouse, write_mouse};
use plurimus_ui::{ComputedWidgetArea, Hovered, PointerPress, ScrollArea, ScrollOffset, UiArea};
use plurimus_widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus_widgets::{MenuOpen, WidgetsPlugin, menu_button, menu_item, menu_popup};

#[derive(Resource, Default)]
struct Presses(Vec<Entity>);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 8));
    app.init_resource::<Presses>();
    app.add_observer(|press: On<PointerPress>, mut log: ResMut<Presses>| log.0.push(press.entity));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_menu(app: &mut App) -> Entity {
    spawn_menu_at(app, Rect::new(1, 0, 8, 1))
}

fn spawn_menu_at(app: &mut App, button_area: Rect) -> Entity {
    let world = app.world_mut();
    let button = world
        .spawn((menu_button("File"), UiArea::Fixed(button_area)))
        .id();
    let popup = world.spawn((menu_popup(button), ChildOf(button))).id();
    world.spawn((menu_item("Open"), ChildOf(popup)));
    world.spawn((menu_item("Save"), ChildOf(popup)));
    popup
}

// Everything the popup covers, so a press falling through the overlay has
// somewhere to land.
fn spawn_pressable(app: &mut App, area: Rect, parent: Option<Entity>) -> Entity {
    let mut pressable = app.world_mut().spawn((
        UiWidget::new(Paragraph::new("row")),
        UiArea::Fixed(area),
        Hovered::default(),
    ));
    if let Some(parent) = parent {
        pressable.insert(ChildOf(parent));
    }
    pressable.id()
}

fn spawn_beneath(app: &mut App) -> Entity {
    app.world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("beneath")),
            UiArea::Fixed(Rect::new(0, 1, 20, 7)),
            Hovered::default(),
        ))
        .id()
}

fn is_open(app: &App, popup: Entity) -> bool {
    app.world().entity(popup).contains::<MenuOpen>()
}

// The overlay's bottom border row: inside the popup, and never a row an
// item was placed on.
const fn footer_of(frame: Rect) -> Rect {
    Rect::new(frame.x, frame.y + frame.height - 1, frame.width, 1)
}

fn popup_area(app: &App, popup: Entity) -> Rect {
    app.world().get::<ComputedWidgetArea>(popup).unwrap().0
}

fn was_pressed(app: &App, entity: Entity) -> bool {
    app.world().resource::<Presses>().0.contains(&entity)
}

// The popup carries `ModalityToggle` but no `Hovered`, so it never wins
// press arbitration: before the guard hit-tested geometry, a press on its
// own border resolved to whatever sat beneath and dismissed the menu.
#[test]
fn a_press_on_the_popup_frame_keeps_the_menu_open() {
    let mut app = app();
    let popup = spawn_menu(&mut app);
    let beneath = spawn_beneath(&mut app);
    click(&mut app, 2, 0);
    assert!(is_open(&app, popup));
    // The popup's rect settles the frame after it stops being hidden,
    // which is the frame the toggle click's deferral already waits for.
    app.update();
    let frame = popup_area(&app, popup);

    click(&mut app, frame.x, frame.y);

    assert!(is_open(&app, popup), "the frame is not a dismiss button");
    assert!(
        !was_pressed(&app, beneath),
        "and nothing beneath was pressed"
    );
}

#[test]
fn a_press_inside_the_popup_routes_to_an_unmarked_child() {
    let mut app = app();
    let popup = spawn_menu(&mut app);
    let beneath = spawn_beneath(&mut app);
    click(&mut app, 2, 0);
    app.update();
    let frame = popup_area(&app, popup);
    let footer = footer_of(frame);
    let child = spawn_pressable(&mut app, footer, Some(popup));
    app.update();

    click(&mut app, footer.x, footer.y);

    assert!(was_pressed(&app, child), "an unmarked child still routes");
    assert!(is_open(&app, popup), "and pressing it dismissed nothing");
    assert!(!was_pressed(&app, beneath));
}

fn spawn_scroller(app: &mut App, area: Rect, parent: Option<Entity>) -> Entity {
    let mut scroller = app.world_mut().spawn((
        UiWidget::new(Paragraph::new("scrollable")),
        UiArea::Fixed(area),
        ScrollArea::new(Size::new(area.width, area.height * 4)),
    ));
    if let Some(parent) = parent {
        scroller.insert(ChildOf(parent));
    }
    scroller.id()
}

fn offset_of(app: &App, scroller: Entity) -> Position {
    app.world().get::<ScrollOffset>(scroller).unwrap().0
}

// The overlay covers the scroller beneath it, so the tick belongs to
// whatever the overlay itself holds - previously it belonged to nobody.
#[test]
fn a_wheel_tick_inside_a_modal_scrolls_that_modal_and_nothing_under_it() {
    let mut app = app();
    let popup = spawn_menu(&mut app);
    let beneath = spawn_scroller(&mut app, Rect::new(0, 1, 20, 7), None);
    click(&mut app, 2, 0);
    app.update();
    let frame = popup_area(&app, popup);
    let inside = spawn_scroller(&mut app, frame, Some(popup));
    app.update();

    send_mouse(&mut app, MouseKind::ScrollDown, frame.x + 1, frame.y + 1);

    assert_eq!(offset_of(&app, inside), Position::new(0, 1), "it scrolled");
    assert_eq!(offset_of(&app, beneath), Position::new(0, 0), "it did not");
    assert!(is_open(&app, popup), "and the tick dismissed nothing");
}

// The rationale the wheel path always had: a tick the overlay covers with
// nothing of its own to scroll dies rather than reaching through.
#[test]
fn a_wheel_tick_inside_a_modal_with_nothing_to_scroll_dies() {
    let mut app = app();
    let popup = spawn_menu(&mut app);
    let beneath = spawn_scroller(&mut app, Rect::new(0, 1, 20, 7), None);
    click(&mut app, 2, 0);
    app.update();
    let frame = popup_area(&app, popup);

    send_mouse(&mut app, MouseKind::ScrollDown, frame.x + 1, frame.y + 1);

    assert_eq!(offset_of(&app, beneath), Position::new(0, 0));
    assert!(is_open(&app, popup), "and it dismissed nothing");
}

// A press the overlay swallows is not a modal flip, so the rest of the
// batch still hit-tests a state nothing is about to change.
#[test]
fn a_swallowed_press_does_not_defer_the_rest_of_the_batch() {
    let mut app = app();
    let popup = spawn_menu(&mut app);
    click(&mut app, 2, 0);
    app.update();
    let frame = popup_area(&app, popup);
    let child = spawn_pressable(&mut app, footer_of(frame), Some(popup));
    app.update();

    for (x, y) in [(frame.x, frame.y), (frame.x, footer_of(frame).y)] {
        write_mouse(&mut app, MouseKind::Down(MouseButton::Left), x, y);
        write_mouse(&mut app, MouseKind::Up(MouseButton::Left), x, y);
    }
    app.update();

    assert!(
        was_pressed(&app, child),
        "the second press landed this frame"
    );
}

// Admission is the union of the modals containing the pointer, not of
// every open modal: an entity belonging to the menu next door is as
// unreachable as one belonging to no modal at all.
#[test]
fn a_modal_the_pointer_is_outside_admits_nothing() {
    let mut app = app();
    let near = spawn_menu(&mut app);
    let far = spawn_menu_at(&mut app, Rect::new(11, 0, 8, 1));
    click(&mut app, 2, 0);
    click(&mut app, 12, 0);
    app.update();
    assert!(is_open(&app, near) && is_open(&app, far), "both are open");
    let frame = popup_area(&app, near);
    let stray = spawn_pressable(&mut app, footer_of(frame), Some(far));
    app.update();

    click(&mut app, frame.x, footer_of(frame).y);

    assert!(
        !was_pressed(&app, stray),
        "the far menu's child is not here"
    );
    assert!(is_open(&app, near) && is_open(&app, far), "and none closed");
}

#[test]
fn a_press_deep_in_the_modal_subtree_routes() {
    let mut app = app();
    let popup = spawn_menu(&mut app);
    click(&mut app, 2, 0);
    app.update();
    let frame = popup_area(&app, popup);
    let group = app.world_mut().spawn(ChildOf(popup)).id();
    let nested = spawn_pressable(&mut app, footer_of(frame), Some(group));
    app.update();

    click(&mut app, frame.x, footer_of(frame).y);

    assert!(was_pressed(&app, nested), "ancestry admits at any depth");
}
