//! Interaction and focus-stack integration tests, fully headless.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input_focus::tab_navigation::{TabGroup, TabIndex};
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{KeyCode, MouseButton, MouseKind};
use plurimus_test::{press_key, send_mouse, write_mouse};
use plurimus_ui::{Click, FocusWithin, Hovered, Pressed, UiArea, UiHidden, UiWidget};
use plurimus_widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus_widgets::{Slider, SliderRange, SliderValue, WidgetsPlugin, slider_self_update};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 6));
    app
}

#[test]
fn hover_tracks_cursor_over_computed_area() {
    let mut app = app();
    app.world_mut().spawn(TerminalCamera::default());
    let widget = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("btn")),
            UiArea::Fixed(Rect::new(2, 1, 5, 1)),
            Hovered::default(),
        ))
        .id();

    send_mouse(&mut app, MouseKind::Moved, 3, 1);
    assert_eq!(app.world().get::<Hovered>(widget), Some(&Hovered(true)));

    send_mouse(&mut app, MouseKind::Moved, 0, 0);
    assert_eq!(app.world().get::<Hovered>(widget), Some(&Hovered(false)));
}

fn spawn_slider(app: &mut App) -> Entity {
    let slider = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("track")),
            UiArea::Fixed(Rect::new(0, 0, 10, 1)),
            Slider,
            SliderRange::new(0.0, 100.0),
            SliderValue(50.0),
        ))
        .id();
    app.world_mut()
        .entity_mut(slider)
        .observe(slider_self_update);
    slider
}

#[test]
fn press_lands_when_the_cursor_leaves_in_the_same_frame() {
    let mut app = app();
    app.world_mut().spawn(TerminalCamera::default());
    let slider = spawn_slider(&mut app);

    write_mouse(&mut app, MouseKind::Down(MouseButton::Left), 9, 0);
    write_mouse(&mut app, MouseKind::Moved, 18, 4);
    app.update();

    assert!(app.world().get::<Pressed>(slider).is_some());
    let value = app.world().get::<SliderValue>(slider).unwrap().0;
    assert!(
        (value - 100.0).abs() < f32::EPSILON,
        "seeks to the press cell"
    );
}

#[test]
fn press_outside_is_not_delivered_when_the_cursor_arrives_later() {
    let mut app = app();
    app.world_mut().spawn(TerminalCamera::default());
    let slider = spawn_slider(&mut app);

    write_mouse(&mut app, MouseKind::Down(MouseButton::Left), 25, 6);
    write_mouse(&mut app, MouseKind::Moved, 5, 0);
    app.update();

    assert!(app.world().get::<Pressed>(slider).is_none());
    let value = app.world().get::<SliderValue>(slider).unwrap().0;
    assert!(
        (value - 50.0).abs() < f32::EPSILON,
        "untouched by an outside press"
    );
}

#[test]
fn hidden_widget_ignores_pointer_until_unhidden() {
    let mut app = app();
    app.world_mut().spawn(TerminalCamera::default());
    let widget = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("btn")),
            UiArea::Fixed(Rect::new(2, 1, 5, 1)),
            Hovered::default(),
            UiHidden,
        ))
        .id();

    send_mouse(&mut app, MouseKind::Moved, 3, 1);
    assert_eq!(app.world().get::<Hovered>(widget), Some(&Hovered(false)));

    app.world_mut().entity_mut(widget).remove::<UiHidden>();
    app.update();
    assert_eq!(app.world().get::<Hovered>(widget), Some(&Hovered(true)));
}

#[derive(Resource, Default)]
struct Clicks(u32);

#[test]
fn click_presses_focuses_and_triggers() {
    let mut app = app();
    app.init_resource::<Clicks>();
    app.world_mut().spawn(TerminalCamera::default());
    let widget = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("btn")),
            UiArea::Fixed(Rect::new(2, 1, 5, 1)),
            Hovered::default(),
            TabIndex(0),
        ))
        .id();
    app.world_mut()
        .entity_mut(widget)
        .observe(|_click: On<Click>, mut clicks: ResMut<Clicks>| clicks.0 += 1);

    send_mouse(&mut app, MouseKind::Moved, 3, 1);
    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 3, 1);
    assert!(app.world().get::<Pressed>(widget).is_some());
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(widget));

    send_mouse(&mut app, MouseKind::Up(MouseButton::Left), 3, 1);
    assert!(app.world().get::<Pressed>(widget).is_none());
    assert_eq!(app.world().resource::<Clicks>().0, 1);
}

#[test]
fn tab_cycles_focus_through_a_group() {
    let mut app = app();
    app.world_mut().spawn(TerminalCamera::default());
    let root = app.world_mut().spawn(TabGroup::new(0)).id();
    let first = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("a")),
            TabIndex(0),
            ChildOf(root),
        ))
        .id();
    let second = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("b")),
            TabIndex(1),
            ChildOf(root),
        ))
        .id();
    app.update();

    press_key(&mut app, KeyCode::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(first));

    press_key(&mut app, KeyCode::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(second));
}

#[test]
fn focus_within_tracks_the_ancestor_chain() {
    let mut app = app();
    let pane_a = app.world_mut().spawn_empty().id();
    let pane_b = app.world_mut().spawn_empty().id();
    let widget_a = app.world_mut().spawn(ChildOf(pane_a)).id();
    let widget_b = app.world_mut().spawn(ChildOf(pane_b)).id();
    app.update();

    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(widget_a, FocusCause::Navigated);
    app.update();
    assert!(app.world().get::<FocusWithin>(pane_a).is_some());
    assert!(app.world().get::<FocusWithin>(pane_b).is_none());

    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(widget_b, FocusCause::Navigated);
    app.update();
    assert!(app.world().get::<FocusWithin>(pane_a).is_none());
    assert!(app.world().get::<FocusWithin>(pane_b).is_some());

    app.world_mut().resource_mut::<InputFocus>().clear();
    app.update();
    assert!(app.world().get::<FocusWithin>(pane_b).is_none());
}
