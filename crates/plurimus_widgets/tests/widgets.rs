//! Standard-widget behavior tests, fully headless.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input_focus::tab_navigation::TabIndex;
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_input::{KeyCode, MouseButton, MouseKind};
use plurimus_test::{write_key, write_mouse};
use plurimus_ui::{Checked, Hovered, UiArea, UiWidget, ValueChange};
use plurimus_widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus_widgets::{
    Activate, Button, Checkbox, RadioButton, RadioGroup, Slider, SliderRange, SliderStep,
    SliderValue, WidgetsPlugin, checkbox_self_update, radio_self_update, slider_self_update,
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize { cols: 30, rows: 8 });
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn focus(app: &mut App, entity: Entity) {
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(entity, FocusCause::Navigated);
}

#[derive(Resource, Default)]
struct Activations(u32);

#[test]
fn button_activates_on_click_and_enter() {
    let mut app = app();
    app.init_resource::<Activations>();
    let button = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("ok")),
            UiArea::Fixed(Rect::new(1, 1, 6, 1)),
            Button,
            Hovered::default(),
            TabIndex(0),
        ))
        .id();
    app.world_mut()
        .entity_mut(button)
        .observe(|_on: On<Activate>, mut count: ResMut<Activations>| count.0 += 1);

    write_mouse(&mut app, MouseKind::Moved, 2, 1);
    write_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 1);
    app.update();
    write_mouse(&mut app, MouseKind::Up(MouseButton::Left), 2, 1);
    app.update();
    assert_eq!(app.world().resource::<Activations>().0, 1);

    write_key(&mut app, KeyCode::Enter);
    app.update();
    assert_eq!(app.world().resource::<Activations>().0, 2);
}

#[derive(Resource, Default)]
struct LastChange(Option<(f32, bool)>);

#[test]
fn slider_steps_seeks_and_scrubs() {
    let mut app = app();
    app.init_resource::<LastChange>();
    let slider = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("track")),
            UiArea::Fixed(Rect::new(2, 2, 11, 1)),
            Slider,
            SliderRange::new(0.0, 100.0),
            SliderValue(50.0),
            SliderStep(10.0),
            Hovered::default(),
            TabIndex(0),
        ))
        .id();
    app.world_mut()
        .entity_mut(slider)
        .observe(slider_self_update);
    app.world_mut().entity_mut(slider).observe(
        |on: On<ValueChange<f32>>, mut last: ResMut<LastChange>| {
            last.0 = Some((on.value, on.is_final));
        },
    );
    focus(&mut app, slider);

    write_key(&mut app, KeyCode::Right);
    app.update();
    assert_eq!(app.world().resource::<LastChange>().0, Some((60.0, true)));
    assert!((app.world().get::<SliderValue>(slider).unwrap().0 - 60.0).abs() < f32::EPSILON);

    write_mouse(&mut app, MouseKind::Moved, 2, 2);
    write_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 2);
    app.update();
    assert_eq!(app.world().resource::<LastChange>().0, Some((0.0, false)));

    write_mouse(&mut app, MouseKind::Drag(MouseButton::Left), 7, 2);
    app.update();
    assert_eq!(app.world().resource::<LastChange>().0, Some((50.0, false)));

    write_mouse(&mut app, MouseKind::Up(MouseButton::Left), 12, 2);
    app.update();
    assert_eq!(app.world().resource::<LastChange>().0, Some((100.0, true)));
    assert!((app.world().get::<SliderValue>(slider).unwrap().0 - 100.0).abs() < f32::EPSILON);
}

#[test]
fn slider_scrub_holds_capture_outside_the_track() {
    let mut app = app();
    app.init_resource::<LastChange>();
    let slider = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("track")),
            UiArea::Fixed(Rect::new(2, 2, 11, 1)),
            Slider,
            SliderRange::new(0.0, 100.0),
            SliderValue(50.0),
            Hovered::default(),
        ))
        .id();
    app.world_mut().entity_mut(slider).observe(
        |on: On<ValueChange<f32>>, mut last: ResMut<LastChange>| {
            last.0 = Some((on.value, on.is_final));
        },
    );

    write_mouse(&mut app, MouseKind::Moved, 2, 2);
    write_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 2);
    app.update();
    assert_eq!(app.world().resource::<LastChange>().0, Some((0.0, false)));

    write_mouse(&mut app, MouseKind::Drag(MouseButton::Left), 25, 6);
    app.update();
    assert!(!app.world().get::<Hovered>(slider).unwrap().0);
    assert_eq!(app.world().resource::<LastChange>().0, Some((100.0, false)));
}

#[test]
fn checkbox_toggles_via_self_update() {
    let mut app = app();
    let checkbox = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("[ ]")),
            UiArea::Fixed(Rect::new(1, 1, 5, 1)),
            Checkbox,
            Hovered::default(),
            TabIndex(0),
        ))
        .id();
    app.world_mut()
        .entity_mut(checkbox)
        .observe(checkbox_self_update);

    write_mouse(&mut app, MouseKind::Moved, 2, 1);
    write_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 1);
    app.update();
    write_mouse(&mut app, MouseKind::Up(MouseButton::Left), 2, 1);
    app.update();
    assert!(app.world().get::<Checked>(checkbox).is_some());

    focus(&mut app, checkbox);
    write_key(&mut app, KeyCode::Char(' '));
    app.update();
    app.update();
    assert!(app.world().get::<Checked>(checkbox).is_none());
}

#[test]
fn radio_group_selects_exclusively() {
    let mut app = app();
    let group = app.world_mut().spawn(RadioGroup).id();
    let first = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("(x) a")),
            UiArea::Fixed(Rect::new(1, 1, 6, 1)),
            RadioButton,
            Checked,
            Hovered::default(),
            ChildOf(group),
        ))
        .id();
    let second = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("( ) b")),
            UiArea::Fixed(Rect::new(1, 2, 6, 1)),
            RadioButton,
            Hovered::default(),
            ChildOf(group),
        ))
        .id();
    app.world_mut().entity_mut(group).observe(radio_self_update);

    write_mouse(&mut app, MouseKind::Moved, 2, 2);
    write_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 2);
    app.update();
    write_mouse(&mut app, MouseKind::Up(MouseButton::Left), 2, 2);
    app.update();

    assert!(app.world().get::<Checked>(second).is_some());
    assert!(app.world().get::<Checked>(first).is_none());
}
