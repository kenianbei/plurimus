//! A focus loss ends a pointer gesture without completing it: the release
//! that would have clicked is never reported, and one that arrives after
//! focus returns reaches nothing.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{On, ResMut, Resource};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{MouseButton, MouseKind};
use plurimus_test::{release_at, send_focus, send_mouse, write_focus, write_mouse};
use plurimus_ui::{
    Click, ComputedWidgetArea, Hovered, PointerCancel, PointerPress, PointerRelease, Pressed,
    UiArea, UiPlugin,
};

const AREA: Rect = Rect::new(2, 1, 6, 3);
const X: u16 = 4;
const Y: u16 = 2;

/// Every gesture event seen, in arrival order per kind.
#[derive(Resource, Default)]
struct Seen {
    presses: Vec<u8>,
    cancels: Vec<Entity>,
    releases: usize,
    clicks: usize,
}

fn app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((CorePlugin, UiPlugin));
    app.insert_resource(TerminalSize::new(20, 8));
    app.init_resource::<Seen>();
    app.add_observer(|press: On<PointerPress>, mut seen: ResMut<Seen>| {
        seen.presses.push(press.count);
    });
    app.add_observer(|cancel: On<PointerCancel>, mut seen: ResMut<Seen>| {
        seen.cancels.push(cancel.entity);
    });
    app.add_observer(|_: On<PointerRelease>, mut seen: ResMut<Seen>| {
        seen.releases += 1;
    });
    app.add_observer(|_: On<Click>, mut seen: ResMut<Seen>| {
        seen.clicks += 1;
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
    let seen = app.world().resource::<Seen>();
    assert_eq!(seen.releases, 0, "a cancelled gesture is never released");
    assert_eq!(
        seen.clicks, 0,
        "the release after focus returns clicks nothing"
    );
}

#[test]
fn losing_focus_cancels_a_held_press() {
    let (mut app, target) = app();
    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), X, Y);
    assert!(app.world().get::<Pressed>(target).is_some());

    send_focus(&mut app, false);

    assert_cancelled(&mut app, target);
}

// The cancel is written after the frame's pumped input, so a press in the
// losing frame is already routed when it lands.
#[test]
fn a_press_in_the_losing_frame_is_cancelled_too() {
    let (mut app, target) = app();
    write_mouse(&mut app, MouseKind::Down(MouseButton::Left), X, Y);
    write_focus(&mut app, false);
    app.update();

    assert_cancelled(&mut app, target);
}

#[test]
fn a_press_after_a_cancel_starts_a_new_run() {
    let (mut app, _) = app();
    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), X, Y);
    send_focus(&mut app, false);

    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), X, Y);

    assert_eq!(app.world().resource::<Seen>().presses, [1, 1]);
}
