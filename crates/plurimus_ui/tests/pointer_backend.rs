//! The picking backend and the pointer behind it, driven headlessly: what a
//! press does to the frame's batch, to focus, and to an open overlay.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, On, ResMut, Resource};
use bevy_input_focus::InputFocus;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::layout::{Position, Rect};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize, UiOrder};
use plurimus_term::MouseButton;
use plurimus_test::{
    click, move_to, press_button_at, scroll_at, send_focus, send_press_at, set_focus,
    write_press_at, write_release_at,
};
use plurimus_ui::bevy_picking::events::{PointerCancel, PointerPress};
use plurimus_ui::bevy_picking::hover::PointerCaptureMap;
use plurimus_ui::bevy_picking::pointer::PointerId;
use plurimus_ui::{
    ComputedWidgetArea, Hovered, ModalDismiss, ModalOpen, Pressed, UiArea, UiPlugin,
};

const AREA: Rect = Rect::new(2, 1, 6, 3);
const NEIGHBOUR: Rect = Rect::new(10, 1, 6, 3);
const CELL: Position = Position::new(4, 2);
const ON_NEIGHBOUR: Position = Position::new(12, 2);
const NOWHERE: Position = Position::new(18, 6);

#[derive(Resource, Default)]
struct Seen {
    presses: Vec<Entity>,
    cancels: Vec<Entity>,
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, UiPlugin));
    app.insert_resource(TerminalSize::new(20, 8));
    app.init_resource::<Seen>();
    app.add_observer(|press: On<PointerPress>, mut seen: ResMut<Seen>| {
        if press.entity == press.original_event_target() {
            seen.presses.push(press.entity);
        }
    });
    app.add_observer(|cancel: On<PointerCancel>, mut seen: ResMut<Seen>| {
        if cancel.entity == cancel.original_event_target() {
            seen.cancels.push(cancel.entity);
        }
    });
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_target(app: &mut App, area: Rect) -> Entity {
    app.world_mut()
        .spawn((
            UiArea::Fixed(area),
            ComputedWidgetArea::default(),
            Hovered::default(),
        ))
        .id()
}

fn presses(app: &App) -> Vec<Entity> {
    app.world().resource::<Seen>().presses.clone()
}

fn focused(app: &App) -> Option<Entity> {
    app.world().resource::<InputFocus>().get()
}

// bevy hit-tests once a frame, at the pointer's final position, so what
// follows a press or a release in the batch waits for the next frame to
// land at its own cell, against the state the click left.
#[test]
fn a_press_and_a_release_each_end_their_frame_of_the_batch() {
    let mut app = app();
    let first = spawn_target(&mut app, AREA);
    let second = spawn_target(&mut app, NEIGHBOUR);
    app.update();

    write_press_at(&mut app, CELL.x, CELL.y);
    write_release_at(&mut app, CELL.x, CELL.y);
    write_press_at(&mut app, ON_NEIGHBOUR.x, ON_NEIGHBOUR.y);
    app.update();
    assert_eq!(presses(&app), vec![first], "the rest of the batch waited");
    app.update();
    assert_eq!(presses(&app), vec![first], "and the release took a frame");
    app.update();

    assert_eq!(presses(&app), vec![first, second]);
    assert!(app.world().entity(second).contains::<Pressed>());
}

#[test]
fn a_press_on_nothing_clears_focus() {
    let mut app = app();
    let widget = spawn_target(&mut app, AREA);
    app.world_mut().entity_mut(widget).insert(TabIndex(0));
    app.update();
    set_focus(&mut app, widget);

    click(&mut app, NOWHERE.x, NOWHERE.y);

    assert_eq!(focused(&app), None);
}

#[test]
fn a_press_on_an_unfocusable_widget_clears_focus() {
    let mut app = app();
    let widget = spawn_target(&mut app, AREA);
    app.world_mut().entity_mut(widget).insert(TabIndex(0));
    spawn_target(&mut app, NEIGHBOUR);
    app.update();
    set_focus(&mut app, widget);

    click(&mut app, ON_NEIGHBOUR.x, ON_NEIGHBOUR.y);

    assert_eq!(focused(&app), None);
}

#[test]
fn a_tick_outside_every_open_overlay_dismisses() {
    let mut app = app();
    spawn_target(&mut app, AREA);
    let overlay = spawn_target(&mut app, NEIGHBOUR);
    app.world_mut()
        .entity_mut(overlay)
        .insert((UiOrder(1), ModalOpen));
    app.add_observer(|dismiss: On<ModalDismiss>, mut commands: Commands| {
        commands.entity(dismiss.entity).remove::<ModalOpen>();
    });
    app.update();

    scroll_at(&mut app, CELL.x, CELL.y, 1);

    assert!(
        !app.world().entity(overlay).contains::<ModalOpen>(),
        "the tick dismissed the overlay"
    );
    assert_eq!(presses(&app), Vec::new());
}

#[test]
fn losing_focus_mid_drag_cancels_the_gesture() {
    let mut app = app();
    let target = spawn_target(&mut app, AREA);
    app.update();
    send_press_at(&mut app, CELL.x, CELL.y);
    move_to(&mut app, NOWHERE.x, NOWHERE.y);
    assert!(app.world().entity(target).contains::<Pressed>());

    send_focus(&mut app, false);

    assert!(!app.world().entity(target).contains::<Pressed>());
    assert_eq!(app.world().resource::<Seen>().cancels, [target]);
}

#[test]
fn a_secondary_press_focuses_without_starting_a_gesture() {
    let mut app = app();
    let widget = spawn_target(&mut app, AREA);
    app.world_mut().entity_mut(widget).insert(TabIndex(0));
    app.update();

    press_button_at(&mut app, MouseButton::Right, CELL.x, CELL.y);

    assert_eq!(focused(&app), Some(widget));
    assert!(!app.world().entity(widget).contains::<Pressed>());
    let capture = app.world().resource::<PointerCaptureMap>();
    assert!(!capture.is_captured(&PointerId::Mouse));
}

#[test]
fn a_secondary_press_outside_an_open_overlay_dismisses_it() {
    let mut app = app();
    let overlay = spawn_target(&mut app, NEIGHBOUR);
    app.world_mut().entity_mut(overlay).insert(ModalOpen);
    app.add_observer(|dismiss: On<ModalDismiss>, mut commands: Commands| {
        commands.entity(dismiss.entity).remove::<ModalOpen>();
    });
    app.update();

    press_button_at(&mut app, MouseButton::Right, NOWHERE.x, NOWHERE.y);

    assert!(!app.world().entity(overlay).contains::<ModalOpen>());
}
