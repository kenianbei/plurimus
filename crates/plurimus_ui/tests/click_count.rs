//! What a run of presses counts to, driven headlessly through the real
//! pointer path.

use core::time::Duration;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, On, Query, ResMut, Resource};
use plurimus_core::ratatui_core::layout::{Position, Rect};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize, UiOrder};
use plurimus_term::{MouseButton, MouseKind, MultiClickWindow};
use plurimus_test::{click, send_mouse, write_mouse};
use plurimus_ui::{
    Click, ComputedWidgetArea, Hovered, InteractionDisabled, ModalDismiss, ModalOpen, PointerDrag,
    PointerPress, Pressed, UiArea, UiPlugin,
};

const AREA: Rect = Rect::new(2, 1, 6, 3);
const NEIGHBOUR: Rect = Rect::new(10, 1, 6, 3);
/// Inside [`AREA`], and away from its origin so a run keyed by the wrong
/// cell would still be keyed by a real one.
const CELL: Position = Position::new(4, 2);
const OTHER_CELL: Position = Position::new(5, 2);
const ON_NEIGHBOUR: Position = Position::new(12, 2);
/// Outside every widget spawned here.
const NOWHERE: Position = Position::new(18, 6);

/// Every count reported, in the order the events arrived.
#[derive(Resource, Default)]
struct Counts {
    presses: Vec<u8>,
    clicks: Vec<u8>,
    drags: Vec<u8>,
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, UiPlugin));
    app.insert_resource(TerminalSize::new(20, 8));
    app.init_resource::<Counts>();
    app.add_observer(|press: On<PointerPress>, mut counts: ResMut<Counts>| {
        counts.presses.push(press.count);
    });
    app.add_observer(|click: On<Click>, mut counts: ResMut<Counts>| {
        counts.clicks.push(click.count);
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

fn presses(app: &App) -> Vec<u8> {
    app.world().resource::<Counts>().presses.clone()
}

fn clicks(app: &App) -> Vec<u8> {
    app.world().resource::<Counts>().clicks.clone()
}

fn drags(app: &App) -> Vec<u8> {
    app.world().resource::<Counts>().drags.clone()
}

fn click_at(app: &mut App, cell: Position) {
    click(app, cell.x, cell.y);
}

fn send_at(app: &mut App, kind: MouseKind, cell: Position) {
    send_mouse(app, kind, cell.x, cell.y);
}

#[test]
fn clicks_running_together_count_up() {
    let mut app = app();
    spawn_target(&mut app, AREA);

    click_at(&mut app, CELL);
    click_at(&mut app, CELL);
    click_at(&mut app, CELL);

    assert_eq!(presses(&app), vec![1, 2, 3]);
    assert_eq!(clicks(&app), vec![1, 2, 3], "the click reports its press");
}

#[test]
fn a_zero_window_leaves_every_click_alone() {
    let mut app = app();
    app.insert_resource(MultiClickWindow(Duration::ZERO));
    spawn_target(&mut app, AREA);

    click_at(&mut app, CELL);
    click_at(&mut app, CELL);

    assert_eq!(presses(&app), vec![1, 1]);
    assert_eq!(clicks(&app), vec![1, 1]);
}

#[test]
fn a_click_on_another_cell_starts_over() {
    let mut app = app();
    spawn_target(&mut app, AREA);

    click_at(&mut app, CELL);
    click_at(&mut app, OTHER_CELL);

    assert_eq!(presses(&app), vec![1, 1]);
}

#[test]
fn a_click_on_a_neighbour_starts_over() {
    let mut app = app();
    spawn_target(&mut app, AREA);
    spawn_target(&mut app, NEIGHBOUR);

    click_at(&mut app, CELL);
    click_at(&mut app, ON_NEIGHBOUR);

    assert_eq!(presses(&app), vec![1, 1]);
}

// The run is the pointer's, not the widget's: a press the widget never
// saw still ended it.
#[test]
fn a_click_on_nothing_between_two_ends_the_run() {
    let mut app = app();
    spawn_target(&mut app, AREA);

    click_at(&mut app, CELL);
    click_at(&mut app, NOWHERE);
    click_at(&mut app, CELL);

    assert_eq!(presses(&app), vec![1, 1]);
}

#[test]
fn a_press_a_disabled_widget_absorbs_ends_the_run() {
    let mut app = app();
    spawn_target(&mut app, AREA);
    let cover = spawn_target(&mut app, NEIGHBOUR);
    app.world_mut()
        .entity_mut(cover)
        .insert(InteractionDisabled);

    click_at(&mut app, CELL);
    click_at(&mut app, ON_NEIGHBOUR);
    click_at(&mut app, CELL);

    assert_eq!(presses(&app), vec![1, 1], "the absorbed press reports none");
}

// Dismissal consumes the press whole, so the press that closed an overlay
// neither counts itself nor leaves the run it interrupted standing.
#[test]
fn a_press_that_dismisses_an_overlay_ends_the_run() {
    let mut app = app();
    spawn_target(&mut app, AREA);
    let overlay = spawn_target(&mut app, NEIGHBOUR);
    app.world_mut().entity_mut(overlay).insert(UiOrder(1));
    // What an overlay's owner does: the guard asks, it closes.
    app.add_observer(|dismiss: On<ModalDismiss>, mut commands: Commands| {
        commands.entity(dismiss.entity).remove::<ModalOpen>();
    });

    click_at(&mut app, CELL);
    app.world_mut().entity_mut(overlay).insert(ModalOpen);
    app.update();
    click_at(&mut app, CELL);
    click_at(&mut app, CELL);

    assert_eq!(
        presses(&app),
        vec![1, 1],
        "the dismissing press reports nothing, and the one after it starts over"
    );
}

// Down and up in one drained batch: the release reads the count from the
// router's own run rather than from the component, which has not landed.
#[test]
fn a_click_completed_in_one_batch_carries_its_count() {
    let mut app = app();
    spawn_target(&mut app, AREA);

    click_at(&mut app, CELL);
    send_at(&mut app, MouseKind::Moved, CELL);
    write_mouse(&mut app, MouseKind::Down(MouseButton::Left), CELL.x, CELL.y);
    write_mouse(&mut app, MouseKind::Up(MouseButton::Left), CELL.x, CELL.y);
    app.update();

    assert_eq!(presses(&app), vec![1, 2]);
    assert_eq!(clicks(&app), vec![1, 2]);
}

// The press of the second click is a frame ahead of its release, so the
// count crosses the frame on the pressed widget rather than in the router.
#[test]
fn a_click_released_a_frame_later_carries_its_count() {
    let mut app = app();
    spawn_target(&mut app, AREA);

    click_at(&mut app, CELL);
    send_at(&mut app, MouseKind::Down(MouseButton::Left), CELL);
    send_at(&mut app, MouseKind::Up(MouseButton::Left), CELL);

    assert_eq!(presses(&app), vec![1, 2]);
    assert_eq!(clicks(&app), vec![1, 2]);
}

// The count rests on the entity, so a drag captured to a widget can tell a
// drag through the second press of a run from one through the first - the
// gesture no event carries a count for.
#[test]
fn a_drag_reads_the_count_of_the_press_holding_it() {
    let mut app = app();
    spawn_target(&mut app, AREA);
    app.add_observer(
        |drag: On<PointerDrag>, pressed: Query<&Pressed>, mut counts: ResMut<Counts>| {
            let held = pressed
                .get(drag.entity)
                .expect("a drag is captured to a pressed widget");
            counts.drags.push(held.0);
        },
    );

    click_at(&mut app, CELL);
    send_at(&mut app, MouseKind::Down(MouseButton::Left), CELL);
    send_at(&mut app, MouseKind::Drag(MouseButton::Left), CELL);

    assert_eq!(presses(&app), vec![1, 2]);
    assert_eq!(drags(&app), vec![2], "the drag is through the second press");
}

#[test]
fn a_lone_press_is_the_first_of_its_run() {
    assert_eq!(
        PointerPress::new(Entity::PLACEHOLDER, Position::ORIGIN).count,
        1
    );
    assert_eq!(Click::new(Entity::PLACEHOLDER, Position::ORIGIN).count, 1);
    assert_eq!(
        PointerPress::new(Entity::PLACEHOLDER, Position::ORIGIN)
            .with_count(2)
            .count,
        2
    );
    assert_eq!(
        Click::new(Entity::PLACEHOLDER, Position::ORIGIN)
            .with_count(3)
            .count,
        3
    );
}
