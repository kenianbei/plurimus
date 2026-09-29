//! A node cut by its camera's edges paints as cut, never shifted into view.

use bevy_app::App;
use bevy_color::Color;
use bevy_ecs::hierarchy::ChildOf;
use bevy_math::Vec2;
use bevy_ui::{
    BorderColor, FlexDirection, Node, Overflow, PositionType, ScrollPosition, UiRect, Val,
};
use plurimus_bui::{BuiPlugin, ComputedNodeRect, Text};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::composed_frame;

const COLS: u16 = 10;
const ROWS: u16 = 5;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, BuiPlugin));
    app.insert_resource(TerminalSize::new(COLS, ROWS));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn absolute(left: f32, top: f32, width: f32, height: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(left),
        top: Val::Px(top),
        width: Val::Px(width),
        height: Val::Px(height),
        ..Node::default()
    }
}

fn settle(app: &mut App) {
    app.update();
    app.update();
}

fn frame_rows(app: &App) -> Vec<String> {
    composed_frame(app).lines().map(str::to_owned).collect()
}

#[test]
fn a_node_off_the_left_edge_keeps_its_true_border_and_text_columns() {
    let mut app = app();
    app.world_mut().spawn((
        Node {
            border: UiRect::all(Val::Px(1.0)),
            ..absolute(-3.0, 1.0, 8.0, 3.0)
        },
        BorderColor::all(Color::WHITE),
        Text::from("abcdef".to_owned()),
    ));
    settle(&mut app);

    let rows = frame_rows(&app);

    assert_eq!(rows[1].trim_end(), "────┐");
    assert_eq!(rows[2].trim_end(), "cdef│");
    assert_eq!(rows[3].trim_end(), "────┘");
}

#[test]
fn text_scrolled_off_the_top_shows_its_later_lines() {
    let mut app = app();
    let container = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(f32::from(COLS)),
                height: Val::Px(3.0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                ..Node::default()
            },
            ScrollPosition(Vec2::new(0.0, 1.0)),
        ))
        .id();
    for label in ["r0\nr1\nr2", "s0\ns1\ns2"] {
        app.world_mut().spawn((
            Node {
                height: Val::Px(3.0),
                flex_shrink: 0.0,
                ..Node::default()
            },
            Text::from(label.to_owned()),
            ChildOf(container),
        ));
    }
    settle(&mut app);

    let rows = frame_rows(&app);

    assert_eq!(
        rows[..3]
            .iter()
            .map(|row| row.trim_end())
            .collect::<Vec<_>>(),
        ["r1", "r2", "s0"]
    );
}

// Half a wide character cannot be drawn, so its column stays blank. The
// frame prints a wide character's second cell as a space.
#[test]
fn a_wide_character_cut_by_the_left_edge_leaves_its_column_blank() {
    let mut app = app();
    app.world_mut().spawn((
        absolute(-1.0, 0.0, 6.0, 1.0),
        Text::from("世界x".to_owned()),
    ));
    settle(&mut app);

    let rows = frame_rows(&app);

    assert_eq!(rows[0].trim_end(), " 界 x");
}

#[test]
fn a_node_past_the_right_edge_publishes_only_its_on_screen_cells() {
    let mut app = app();
    let node = app.world_mut().spawn(absolute(6.0, 3.0, 8.0, 4.0)).id();
    settle(&mut app);

    let published = app
        .world()
        .get::<ComputedNodeRect>(node)
        .expect("a laid-out node");
    assert_eq!(published.rect, Rect::new(6, 3, 4, 2));
}
