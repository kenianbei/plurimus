//! A focus loss ends a pointer gesture without completing it: the widget
//! is sent a cancel, and the release that would have clicked clicks
//! nothing.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{On, ResMut, Resource};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::{release_at, send_focus, send_press_at, write_focus, write_press_at};
use plurimus_ui::bevy_picking::events::{PointerCancel, PointerClick, PointerPress};
use plurimus_ui::{ComputedWidgetArea, Hovered, Pressed, UiArea, UiPlugin};

const AREA: Rect = Rect::new(2, 1, 6, 3);
const X: u16 = 4;
const Y: u16 = 2;

/// Every gesture event seen, in arrival order per kind.
#[derive(Resource, Default)]
struct Seen {
    presses: Vec<u8>,
    cancels: Vec<Entity>,
    clicks: usize,
}

fn app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((CorePlugin, UiPlugin));
    app.insert_resource(TerminalSize::new(20, 8));
    app.init_resource::<Seen>();
    app.add_observer(|press: On<PointerPress>, mut seen: ResMut<Seen>| {
        if press.entity == press.original_event_target() {
            seen.presses.push(press.count);
        }
    });
    app.add_observer(|cancel: On<PointerCancel>, mut seen: ResMut<Seen>| {
        if cancel.entity == cancel.original_event_target() {
            seen.cancels.push(cancel.entity);
        }
    });
    app.add_observer(|click: On<PointerClick>, mut seen: ResMut<Seen>| {
        if click.entity == click.original_event_target() {
            seen.clicks += 1;
        }
    });
    app.world_mut().spawn(TerminalCamera::default());
    let target = app
        .world_mut()
        .spawn((
            UiArea::Fixed(AREA),
            ComputedWidgetArea::default(),
            Hovered::default(),
        ))
        .id();
    app.update();
    (app, target)
}

fn assert_cancelled(app: &mut App, target: Entity) {
    assert!(app.world().get::<Pressed>(target).is_none());
    assert_eq!(app.world().resource::<Seen>().cancels, [target]);

    release_at(app, X, Y);
    assert_eq!(
        app.world().resource::<Seen>().clicks,
        0,
        "the release after focus returns clicks nothing"
    );
}

#[test]
fn losing_focus_cancels_a_held_press() {
    let (mut app, target) = app();
    send_press_at(&mut app, X, Y);
    assert!(app.world().get::<Pressed>(target).is_some());

    send_focus(&mut app, false);

    assert_cancelled(&mut app, target);
}

// The cancel is written after the frame's pumped input, so a press in the
// losing frame is already routed when it lands.
#[test]
fn a_press_in_the_losing_frame_is_cancelled_too() {
    let (mut app, target) = app();
    write_press_at(&mut app, X, Y);
    write_focus(&mut app, false);
    app.update();
    app.update();

    assert_cancelled(&mut app, target);
}

#[test]
fn a_press_after_a_cancel_presses_again() {
    let (mut app, target) = app();
    send_press_at(&mut app, X, Y);
    send_focus(&mut app, false);

    send_press_at(&mut app, X, Y);

    assert_eq!(app.world().resource::<Seen>().presses.len(), 2);
    assert!(app.world().get::<Pressed>(target).is_some());
}
