//! What a pointer inside an open modal overlay may reach.
//!
//! The guard answers "inside" from the overlay's own rect, so an overlay
//! confines the pointer to its subtree instead of relying on every child
//! being marked as modal. The overlay here is built on the primitives
//! alone, the way an app's own overlay is: an opener carrying
//! `ModalityToggle` shows it on a press, and a `ModalDismiss` hides it.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::{ChildOf, Children};
use bevy_ecs::prelude::{Commands, Component, Has, On, Query, ResMut, Resource, With};
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::{
    CorePlugin, Hidden, Propagate, TerminalCamera, TerminalSize, UiOrder, UiWidget,
};
use plurimus_test::{click, scroll_at, write_press_at, write_release_at, write_scroll_at};
use plurimus_ui::bevy_input_focus::InputFocus;
use plurimus_ui::bevy_picking::events::PointerPress;
use plurimus_ui::{
    ComputedWidgetArea, Hovered, ModalDismiss, ModalOpen, ModalityToggle, ScrollArea, ScrollOffset,
    UiArea,
};
use plurimus_widgets::WidgetsPlugin;
use plurimus_widgets::ratatui_widgets::paragraph::Paragraph;

#[derive(Resource, Default)]
struct Presses(Vec<Entity>);

/// Shows the overlay beneath it on a press, or hides it if it is shown.
#[derive(Component)]
struct Opener;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 8));
    app.init_resource::<Presses>();
    app.add_observer(|press: On<PointerPress>, mut log: ResMut<Presses>| {
        if press.entity == press.original_event_target() {
            log.0.push(press.entity);
        }
    });
    app.add_observer(toggle_on_press);
    app.add_observer(|dismiss: On<ModalDismiss>, mut commands: Commands| {
        close(dismiss.entity, &mut commands);
    });
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn toggle_on_press(
    press: On<PointerPress>,
    openers: Query<&Children, With<Opener>>,
    overlays: Query<Has<ModalOpen>>,
    mut commands: Commands,
) {
    if press.entity != press.original_event_target() {
        return;
    }
    let Ok(children) = openers.get(press.entity) else {
        return;
    };
    for &overlay in children {
        match overlays.get(overlay) {
            Ok(true) => close(overlay, &mut commands),
            Ok(false) => {
                commands
                    .entity(overlay)
                    .insert(ModalOpen)
                    .remove::<Propagate<Hidden>>();
            }
            Err(_) => {}
        }
    }
}

fn close(overlay: Entity, commands: &mut Commands) {
    commands
        .entity(overlay)
        .remove::<ModalOpen>()
        .insert(Propagate(Hidden));
}

fn spawn_menu(app: &mut App) -> Entity {
    spawn_menu_at(app, Rect::new(1, 0, 8, 1))
}

/// An opener at `button_area` and, hidden beneath it, a framed overlay of
/// two rows directly below it.
fn spawn_menu_at(app: &mut App, button_area: Rect) -> Entity {
    let world = app.world_mut();
    let button = world
        .spawn((
            UiWidget::new(Paragraph::new("File")),
            UiArea::Fixed(button_area),
            Hovered::default(),
            ModalityToggle,
            TabIndex(0),
            Opener,
        ))
        .id();
    let frame = Rect::new(button_area.x, button_area.bottom(), button_area.width, 4);
    let popup = world
        .spawn((
            UiWidget::new(Paragraph::new("")),
            UiArea::Fixed(frame),
            UiOrder::OVERLAY,
            Propagate(Hidden),
            ChildOf(button),
        ))
        .id();
    for row in 1..=2 {
        world.spawn((
            UiWidget::new(Paragraph::new("row")),
            UiArea::Fixed(Rect::new(frame.x + 1, frame.y + row, frame.width - 2, 1)),
            UiOrder(UiOrder::OVERLAY.0 + 1),
            Hovered::default(),
            ChildOf(popup),
        ));
    }
    popup
}

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

// Everything the popup covers, so a press falling through the overlay has
// somewhere to land.
fn spawn_beneath(app: &mut App) -> Entity {
    spawn_pressable(app, COVERED, None)
}

fn is_open(app: &App, popup: Entity) -> bool {
    app.world().entity(popup).contains::<ModalOpen>()
}

const COVERED: Rect = Rect::new(0, 1, 20, 7);

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

// The overlay has no `Hovered`, so it never wins press arbitration: before
// the guard hit-tested geometry, a press on its own border resolved to
// whatever sat beneath and dismissed it.
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
    let beneath = spawn_scroller(&mut app, COVERED, None);
    click(&mut app, 2, 0);
    app.update();
    let frame = popup_area(&app, popup);
    // Deliberately wider than the widget it covers: z-order gives the tick
    // to the smaller rect, so only confinement can put it here.
    let inside = spawn_scroller(&mut app, Rect::new(0, 0, 20, 8), Some(popup));
    app.update();

    scroll_at(&mut app, frame.x + 1, frame.y + 1, 1);

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
    let beneath = spawn_scroller(&mut app, COVERED, None);
    click(&mut app, 2, 0);
    app.update();
    let frame = popup_area(&app, popup);

    scroll_at(&mut app, frame.x + 1, frame.y + 1, 1);

    assert_eq!(offset_of(&app, beneath), Position::new(0, 0));
    assert!(is_open(&app, popup), "and it dismissed nothing");
}

// The tick sits behind the click that opens the menu, so it waits for the
// open with the rest of the batch rather than scrolling what the popup is
// about to cover.
#[test]
fn a_tick_behind_an_opening_click_lands_on_the_open_menu() {
    let mut app = app();
    let popup = spawn_menu(&mut app);
    let beneath = spawn_scroller(&mut app, COVERED, None);
    app.update();

    write_press_at(&mut app, 2, 0);
    write_release_at(&mut app, 2, 0);
    write_scroll_at(&mut app, 3, 2, 1);
    app.update();
    app.update();

    assert!(is_open(&app, popup));
    assert_eq!(offset_of(&app, beneath), Position::new(0, 0));
}

// Admission is the union of the modals containing the pointer, not of
// every open modal: an entity belonging to the menu next door is as
// unreachable as one belonging to no modal at all. Clicking the second
// button would close the first, so the second is opened by hand.
#[test]
fn a_modal_the_pointer_is_outside_admits_nothing() {
    let mut app = app();
    let near = spawn_menu(&mut app);
    let far = spawn_menu_at(&mut app, Rect::new(11, 0, 8, 1));
    click(&mut app, 2, 0);
    app.world_mut()
        .entity_mut(far)
        .insert(ModalOpen)
        .remove::<Propagate<Hidden>>();
    app.update();
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

fn button_of(app: &App, popup: Entity) -> Entity {
    app.world().get::<ChildOf>(popup).unwrap().parent()
}

fn focused(app: &App) -> Option<Entity> {
    app.world().resource::<InputFocus>().get()
}

// A toggle owns only the modals beneath it, so the other menu's button
// closes this one and opens its own in the one click.
#[test]
fn a_press_on_another_menus_button_switches_to_it() {
    let mut app = app();
    let near = spawn_menu(&mut app);
    let far = spawn_menu_at(&mut app, Rect::new(11, 0, 8, 1));
    click(&mut app, 2, 0);
    app.update();

    click(&mut app, 12, 0);
    app.update();

    assert!(!is_open(&app, near), "the first overlay closed");
    assert!(is_open(&app, far), "and the second opened");
    assert_eq!(focused(&app), Some(button_of(&app, far)));
}

// The closing menu is still open until its dismissal lands, so a press
// inside it later in the same batch must wait rather than reach its rows.
#[test]
fn a_switching_press_defers_the_rest_of_the_batch() {
    let mut app = app();
    let near = spawn_menu(&mut app);
    spawn_menu_at(&mut app, Rect::new(11, 0, 8, 1));
    click(&mut app, 2, 0);
    app.update();
    let row = app.world().get::<Children>(near).unwrap()[0];
    let cell = popup_area(&app, row).as_position();

    write_press_at(&mut app, 12, 0);
    write_release_at(&mut app, 15, 6);
    write_press_at(&mut app, cell.x, cell.y);
    write_release_at(&mut app, cell.x, cell.y);
    app.update();
    app.update();

    assert!(!is_open(&app, near));
    assert!(
        !was_pressed(&app, row),
        "the closing menu's row was reached"
    );
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
