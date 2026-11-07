//! What a run of presses counts to, driven headlessly through the real
//! pointer path: `bevy_picking` keeps the run, per widget, against its
//! own `multi_click_interval`.

use core::time::Duration;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{On, ResMut, Resource};
use plurimus_core::ratatui_core::layout::{Position, Rect};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{MouseButton, MouseKind};
use plurimus_test::{click, release_at, send_mouse, write_mouse};
use plurimus_ui::bevy_picking::PickingSettings;
use plurimus_ui::bevy_picking::events::{PointerClick, PointerPress};
use plurimus_ui::{ComputedWidgetArea, Hovered, UiArea, UiPlugin};

const AREA: Rect = Rect::new(2, 1, 6, 3);
const NEIGHBOUR: Rect = Rect::new(10, 1, 6, 3);
const CELL: Position = Position::new(4, 2);
const OTHER_CELL: Position = Position::new(5, 2);
const ON_NEIGHBOUR: Position = Position::new(12, 2);

/// Every count reported, in the order the events arrived.
#[derive(Resource, Default)]
struct Counts {
    presses: Vec<u8>,
    clicks: Vec<u8>,
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, UiPlugin));
    app.insert_resource(TerminalSize::new(20, 8));
    app.init_resource::<Counts>();
    // Each event bubbles to the window; the count is read where it landed.
    app.add_observer(|press: On<PointerPress>, mut counts: ResMut<Counts>| {
        if press.entity == press.original_event_target() {
            counts.presses.push(press.count);
        }
    });
    app.add_observer(|click: On<PointerClick>, mut counts: ResMut<Counts>| {
        if click.entity == click.original_event_target() {
            counts.clicks.push(click.count);
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

fn presses(app: &App) -> Vec<u8> {
    app.world().resource::<Counts>().presses.clone()
}

fn clicks(app: &App) -> Vec<u8> {
    app.world().resource::<Counts>().clicks.clone()
}

fn click_at(app: &mut App, cell: Position) {
    click(app, cell.x, cell.y);
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
fn a_zero_interval_leaves_every_click_alone() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<PickingSettings>()
        .multi_click_interval = Duration::ZERO;
    spawn_target(&mut app, AREA);

    click_at(&mut app, CELL);
    click_at(&mut app, CELL);

    assert_eq!(presses(&app), vec![1, 1]);
    assert_eq!(clicks(&app), vec![1, 1]);
}

// The run is the widget's, not the cell's.
#[test]
fn a_click_on_another_cell_of_the_widget_counts_on() {
    let mut app = app();
    spawn_target(&mut app, AREA);

    click_at(&mut app, CELL);
    click_at(&mut app, OTHER_CELL);

    assert_eq!(presses(&app), vec![1, 2]);
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

// Down and up in one drained batch: the press ends its frame and the
// release lands in the next, against the map that holds the pressed widget.
#[test]
fn a_click_completed_in_one_batch_carries_its_count() {
    let mut app = app();
    spawn_target(&mut app, AREA);

    click_at(&mut app, CELL);
    write_mouse(&mut app, MouseKind::Down(MouseButton::Left), CELL.x, CELL.y);
    release_at(&mut app, CELL.x, CELL.y);
    app.update();

    assert_eq!(presses(&app), vec![1, 2]);
    assert_eq!(clicks(&app), vec![1, 2]);
}

#[test]
fn a_click_released_a_frame_later_carries_its_count() {
    let mut app = app();
    spawn_target(&mut app, AREA);

    click_at(&mut app, CELL);
    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), CELL.x, CELL.y);
    release_at(&mut app, CELL.x, CELL.y);

    assert_eq!(presses(&app), vec![1, 2]);
    assert_eq!(clicks(&app), vec![1, 2]);
}
