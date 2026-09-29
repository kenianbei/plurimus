//! Interaction and focus-stack integration tests, fully headless.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input_focus::InputFocus;
use bevy_input_focus::tab_navigation::{TabGroup, TabIndex};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{KeyCode, MouseButton, MouseKind};
use plurimus_test::{
    press_at, press_key, release_at, send_focus, send_mouse, set_focus, write_mouse,
};
use plurimus_ui::Key;
use plurimus_ui::{Click, FocusWithin, Hovered, Pressed, UiArea, UiHidden, UiWidget, ValueChange};
use plurimus_widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus_widgets::{
    Slider, SliderAction, SliderKeys, SliderRange, SliderValue, WidgetsPlugin, slider_self_update,
};

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
    let value = value(&app, slider);
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
    let value = value(&app, slider);
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

    press_at(&mut app, 3, 1);
    assert!(app.world().get::<Pressed>(widget).is_some());
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(widget));

    release_at(&mut app, 3, 1);
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

    set_focus(&mut app, widget_a);
    app.update();
    assert!(app.world().get::<FocusWithin>(pane_a).is_some());
    assert!(app.world().get::<FocusWithin>(pane_b).is_none());

    set_focus(&mut app, widget_b);
    app.update();
    assert!(app.world().get::<FocusWithin>(pane_a).is_none());
    assert!(app.world().get::<FocusWithin>(pane_b).is_some());

    app.world_mut().resource_mut::<InputFocus>().clear();
    app.update();
    assert!(app.world().get::<FocusWithin>(pane_b).is_none());
}

#[test]
fn a_remapped_slider_steps_on_its_own_keys() {
    let mut app = app();
    app.world_mut().spawn(TerminalCamera::default());
    let slider = spawn_slider(&mut app);
    app.world_mut().entity_mut(slider).insert((
        TabIndex(0),
        SliderKeys(vec![
            (Key::Character("h".into()).into(), SliderAction::Decrease),
            (Key::Character("l".into()).into(), SliderAction::Increase),
        ]),
    ));
    app.update();
    set_focus(&mut app, slider);

    press_key(&mut app, KeyCode::Char('l'));
    assert!(value(&app, slider) > 50.0, "the bound key steps");

    let stepped = value(&app, slider);
    press_key(&mut app, KeyCode::Right);
    assert_eq!(
        value(&app, slider),
        stepped,
        "and the key it replaced does nothing"
    );

    press_key(&mut app, KeyCode::Char('h'));
    assert_eq!(value(&app, slider), 50.0);
}

#[derive(Resource, Default)]
struct FinalValues(Vec<f32>);

// The drag's own changes are not final, so the one final change is the
// cancel's, and an app committing on `is_final` still gets to.
#[test]
fn a_drag_cut_off_by_a_focus_loss_commits_where_it_reached() {
    let mut app = app();
    app.world_mut().spawn(TerminalCamera::default());
    app.init_resource::<FinalValues>();
    let slider = spawn_slider(&mut app);
    app.world_mut().entity_mut(slider).observe(
        |change: On<ValueChange<f32>>, mut finals: ResMut<FinalValues>| {
            if change.is_final {
                finals.0.push(change.value);
            }
        },
    );

    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 0, 0);
    send_mouse(&mut app, MouseKind::Drag(MouseButton::Left), 9, 0);
    send_focus(&mut app, false);

    assert!(app.world().get::<Pressed>(slider).is_none());
    assert_eq!(app.world().resource::<FinalValues>().0, [100.0]);
}

fn value(app: &App, slider: Entity) -> f32 {
    app.world().get::<SliderValue>(slider).unwrap().0
}
