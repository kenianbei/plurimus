//! Snapshot tests pinning the stock stylists' rendered widget states.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::ChildOf;
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::{Color, Style};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_input::{MouseButton, MouseKind};
use plurimus_test::{composed_styled_frame, write_mouse};
use plurimus_ui::{InteractionDisabled, StylistDisabled, UiArea, UiStyle};
use plurimus_widgets::{
    RadioGroup, WidgetsPlugin, button, checkbox, checkbox_self_update, radio, radio_self_update,
    slider,
};

fn app(cols: u16, rows: u16) -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize { cols, rows });
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn focus(app: &mut App, entity: Entity) {
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(entity, FocusCause::Navigated);
}

#[test]
fn button_states() {
    let mut app = app(10, 1);
    let button = app
        .world_mut()
        .spawn((button("ok"), UiArea::Fixed(Rect::new(0, 0, 10, 1))))
        .id();

    app.update();
    insta::assert_snapshot!("button_normal", composed_styled_frame(&app));

    write_mouse(&mut app, MouseKind::Moved, 4, 0);
    app.update();
    insta::assert_snapshot!("button_hovered", composed_styled_frame(&app));

    write_mouse(&mut app, MouseKind::Down(MouseButton::Left), 4, 0);
    app.update();
    app.update();
    insta::assert_snapshot!("button_pressed", composed_styled_frame(&app));

    write_mouse(&mut app, MouseKind::Up(MouseButton::Left), 4, 0);
    write_mouse(&mut app, MouseKind::Moved, 0, 0);
    focus(&mut app, button);
    app.update();
    app.update();
    insta::assert_snapshot!("button_focused", composed_styled_frame(&app));

    app.world_mut()
        .entity_mut(button)
        .insert(InteractionDisabled);
    app.update();
    app.update();
    insta::assert_snapshot!("button_disabled", composed_styled_frame(&app));
}

#[test]
fn checkbox_states() {
    let mut app = app(12, 1);
    let toggle = app
        .world_mut()
        .spawn((checkbox("dark"), UiArea::Fixed(Rect::new(0, 0, 12, 1))))
        .id();
    app.world_mut()
        .entity_mut(toggle)
        .observe(checkbox_self_update);

    app.update();
    insta::assert_snapshot!("checkbox_unchecked", composed_styled_frame(&app));

    write_mouse(&mut app, MouseKind::Moved, 2, 0);
    write_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 0);
    app.update();
    write_mouse(&mut app, MouseKind::Up(MouseButton::Left), 2, 0);
    write_mouse(&mut app, MouseKind::Moved, 11, 0);
    app.update();
    app.update();
    insta::assert_snapshot!("checkbox_checked", composed_styled_frame(&app));
}

#[test]
fn radio_states() {
    let mut app = app(10, 2);
    let group = app.world_mut().spawn(RadioGroup).id();
    let first = app
        .world_mut()
        .spawn((
            radio("one"),
            UiArea::Fixed(Rect::new(0, 0, 10, 1)),
            ChildOf(group),
        ))
        .id();
    app.world_mut().spawn((
        radio("two"),
        UiArea::Fixed(Rect::new(0, 1, 10, 1)),
        ChildOf(group),
    ));
    app.world_mut().entity_mut(group).observe(radio_self_update);
    app.world_mut()
        .entity_mut(first)
        .insert(plurimus_ui::Checked);

    app.update();
    app.update();
    insta::assert_snapshot!("radio_first_selected", composed_styled_frame(&app));
}

#[test]
fn slider_states() {
    let mut app = app(11, 1);
    let track = app
        .world_mut()
        .spawn((
            slider(0.0, 100.0, 0.0),
            UiArea::Fixed(Rect::new(0, 0, 11, 1)),
        ))
        .id();

    app.update();
    insta::assert_snapshot!("slider_at_0", composed_styled_frame(&app));

    app.world_mut()
        .get_mut::<plurimus_widgets::SliderValue>(track)
        .unwrap()
        .0 = 50.0;
    app.update();
    insta::assert_snapshot!("slider_at_50", composed_styled_frame(&app));

    app.world_mut()
        .get_mut::<plurimus_widgets::SliderValue>(track)
        .unwrap()
        .0 = 100.0;
    focus(&mut app, track);
    app.update();
    insta::assert_snapshot!("slider_at_100_focused", composed_styled_frame(&app));
}

// The style legend line covering the widget's own cells.
fn legend_of(frame: &str, letter: char) -> String {
    frame
        .lines()
        .find(|line| line.starts_with(&format!("{letter}: ")))
        .expect("the letter is in the legend")
        .to_owned()
}

#[test]
fn a_ui_style_patches_the_theme_rather_than_replacing_it() {
    let mut app = app(10, 1);
    let target = app
        .world_mut()
        .spawn((
            button("ok"),
            UiStyle(Style::new().bg(Color::Indexed(236))),
            UiArea::Fixed(Rect::new(0, 0, 10, 1)),
        ))
        .id();
    focus(&mut app, target);
    app.update();
    app.update();

    let frame = composed_styled_frame(&app);
    let styled = legend_of(&frame, 'a');
    assert!(styled.contains("bg:Some(Indexed(236))"), "{styled}");
    assert!(
        styled.contains("fg:Some(Yellow)") && styled.contains("BOLD"),
        "the theme's focused style survives the patch: {styled}"
    );
}

#[test]
fn changing_a_ui_style_rebuilds_the_widget() {
    let mut app = app(10, 1);
    let target = app
        .world_mut()
        .spawn((
            button("ok"),
            UiStyle(Style::new().bg(Color::Red)),
            UiArea::Fixed(Rect::new(0, 0, 10, 1)),
        ))
        .id();
    app.update();
    let before = composed_styled_frame(&app);

    app.world_mut()
        .entity_mut(target)
        .insert(UiStyle(Style::new().bg(Color::Blue)));
    app.update();

    assert_ne!(
        before,
        composed_styled_frame(&app),
        "the cache must notice an override change with no other state change"
    );
}

#[test]
fn a_disabled_stylist_leaves_the_widget_to_the_app() {
    use plurimus_core::UiWidget;
    use plurimus_widgets::ratatui_widgets::paragraph::Paragraph;

    let mut app = app(10, 1);
    let button = app
        .world_mut()
        .spawn((
            button("ok"),
            StylistDisabled,
            UiArea::Fixed(Rect::new(0, 0, 10, 1)),
        ))
        .id();
    app.update();

    app.world_mut()
        .entity_mut(button)
        .insert(UiWidget::new(Paragraph::new("mine")));
    app.update();
    let owned = composed_styled_frame(&app);

    focus(&mut app, button);
    write_mouse(&mut app, MouseKind::Moved, 4, 0);
    app.update();
    app.update();

    assert_eq!(
        owned,
        composed_styled_frame(&app),
        "focus and hover must not restyle an exempt widget"
    );
}

#[test]
fn pane_border_follows_focus_within() {
    use plurimus_widgets::pane;

    let mut app = app(10, 3);
    let side = app
        .world_mut()
        .spawn((pane("side"), UiArea::Fixed(Rect::new(0, 0, 10, 3))))
        .id();
    let field = app
        .world_mut()
        .spawn((
            button("hi"),
            UiArea::Fixed(Rect::new(2, 1, 6, 1)),
            ChildOf(side),
        ))
        .id();

    app.update();
    let idle = composed_styled_frame(&app);
    insta::assert_snapshot!("pane_idle", &idle);

    focus(&mut app, field);
    app.update();
    insta::assert_snapshot!("pane_focus_within", composed_styled_frame(&app));

    app.world_mut().resource_mut::<InputFocus>().clear();
    app.update();
    assert_eq!(
        idle,
        composed_styled_frame(&app),
        "clearing focus restores the idle border"
    );
}
