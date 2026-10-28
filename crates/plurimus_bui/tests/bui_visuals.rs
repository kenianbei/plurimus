//! Outlines, visibility and partial borders on `bevy_ui` nodes.

use bevy_app::App;
use bevy_camera::visibility::Visibility;
use bevy_color::Color;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_ui::widget::Text;
use bevy_ui::{
    BackgroundColor, BorderColor, FlexDirection, Node, Outline, PositionType, UiRect, Val,
};
use plurimus_bui::BuiPlugin;
use plurimus_bui::bevy_ui_widgets::Button;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::{click, composed_frame};
use plurimus_ui::Activate;

#[derive(Resource, Default)]
struct Activations(u32);

fn app(cols: u16, rows: u16) -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, BuiPlugin));
    app.insert_resource(TerminalSize::new(cols, rows));
    app.init_resource::<Activations>();
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn settle(app: &mut App) {
    app.update();
    app.update();
}

fn lines(app: &App) -> Vec<String> {
    composed_frame(app).lines().map(str::to_owned).collect()
}

#[test]
fn an_outline_rings_the_node_outside_its_rect() {
    let mut app = app(8, 5);
    app.world_mut().spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(2.0),
            top: Val::Px(1.0),
            width: Val::Px(4.0),
            height: Val::Px(3.0),
            ..Node::default()
        },
        BackgroundColor(Color::BLACK),
        Outline {
            width: Val::Px(1.0),
            offset: Val::ZERO,
            color: Color::srgb(1.0, 0.0, 0.0),
        },
    ));
    settle(&mut app);

    assert_eq!(
        lines(&app),
        [" ┌────┐ ", " │    │ ", " │    │ ", " │    │ ", " └────┘ "]
    );
}

#[test]
fn a_hidden_subtree_is_neither_drawn_nor_picked() {
    let mut app = app(10, 4);
    let root = app
        .world_mut()
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            ..Node::default()
        })
        .id();
    app.world_mut()
        .spawn((Node::default(), Text::new("shown"), ChildOf(root)));
    let hidden = app
        .world_mut()
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..Node::default()
            },
            Visibility::Hidden,
            ChildOf(root),
        ))
        .id();
    let deep = app
        .world_mut()
        .spawn((Node::default(), Text::new("deep"), Button, ChildOf(hidden)))
        .id();
    app.world_mut()
        .entity_mut(deep)
        .observe(|_: On<Activate>, mut count: ResMut<Activations>| count.0 += 1);
    app.world_mut().spawn((
        Node::default(),
        Text::new("forced"),
        Visibility::Visible,
        ChildOf(hidden),
    ));
    settle(&mut app);

    let frame = composed_frame(&app);
    assert!(frame.contains("shown"), "{frame}");
    assert!(frame.contains("forced"), "{frame}");
    assert!(!frame.contains("deep"), "{frame}");

    click(&mut app, 1, 1);
    assert_eq!(app.world().resource::<Activations>().0, 0);

    app.world_mut()
        .entity_mut(hidden)
        .insert(Visibility::Inherited);
    settle(&mut app);
    assert!(composed_frame(&app).contains("deep"));
    click(&mut app, 1, 1);
    assert_eq!(app.world().resource::<Activations>().0, 1);
}

fn bordered(app: &mut App, border: UiRect) -> Entity {
    app.world_mut()
        .spawn((
            Node {
                width: Val::Px(6.0),
                height: Val::Px(3.0),
                border,
                ..Node::default()
            },
            BorderColor::all(Color::WHITE),
        ))
        .id()
}

#[test]
fn a_top_only_border_runs_end_to_end_without_corners() {
    let mut app = app(6, 3);
    bordered(&mut app, UiRect::top(Val::Px(1.0)));
    settle(&mut app);

    assert_eq!(lines(&app), ["──────", "      ", "      "]);
}

#[test]
fn a_left_only_border_runs_the_full_height() {
    let mut app = app(6, 3);
    bordered(&mut app, UiRect::left(Val::Px(1.0)));
    settle(&mut app);

    assert_eq!(lines(&app), ["│     ", "│     ", "│     "]);
}

#[test]
fn two_sides_meet_in_a_corner() {
    let mut app = app(6, 3);
    bordered(
        &mut app,
        UiRect {
            top: Val::Px(1.0),
            left: Val::Px(1.0),
            ..UiRect::ZERO
        },
    );
    settle(&mut app);

    assert_eq!(lines(&app), ["┌─────", "│     ", "│     "]);
}
