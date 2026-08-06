//! Snapshot tests for the bevy_ui-to-cells rasterizer.

use bevy_app::App;
use bevy_color::Color;
use bevy_ecs::prelude::ChildOf;
use bevy_ui::{
    BackgroundColor, BorderColor, FlexDirection, Node, Overflow, ScrollPosition, UiRect, Val,
};
use plurimus_bui::BuiPlugin;
use plurimus_bui::{Text, TextStyle};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize, Viewport};
use plurimus_input::MouseKind;
use plurimus_test::{composed_styled_frame, send_mouse};

fn app(cols: u16, rows: u16) -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, BuiPlugin));
    app.insert_resource(TerminalSize { cols, rows });
    app.world_mut().spawn(TerminalCamera::default());
    app
}

#[test]
fn flex_column_backgrounds() {
    let mut app = app(8, 4);
    let root = app
        .world_mut()
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..Node::default()
        })
        .id();
    app.world_mut().spawn((
        Node {
            flex_grow: 1.0,
            ..Node::default()
        },
        BackgroundColor(Color::srgb(1.0, 0.0, 0.0)),
        ChildOf(root),
    ));
    app.world_mut().spawn((
        Node {
            flex_grow: 1.0,
            ..Node::default()
        },
        BackgroundColor(Color::srgb(0.0, 0.0, 1.0)),
        ChildOf(root),
    ));

    app.update();
    app.update();

    insta::assert_snapshot!("bui_flex_backgrounds", composed_styled_frame(&app));
}

#[test]
fn bordered_padded_text() {
    let mut app = app(20, 5);
    app.world_mut().spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            padding: UiRect::all(Val::Px(1.0)),
            ..Node::default()
        },
        BorderColor::from(Color::srgb(0.0, 1.0, 0.0)),
        Text::from("hi there terminal"),
        TextStyle(
            plurimus_core::ratatui_core::style::Style::new()
                .fg(plurimus_core::ratatui_core::style::Color::Yellow),
        ),
    ));

    app.update();
    app.update();

    insta::assert_snapshot!("bui_border_padding_text", composed_styled_frame(&app));
}

#[test]
fn overflow_clips_children() {
    let mut app = app(10, 3);
    let root = app
        .world_mut()
        .spawn(Node {
            width: Val::Px(6.0),
            height: Val::Percent(100.0),
            overflow: Overflow::clip(),
            ..Node::default()
        })
        .id();
    app.world_mut().spawn((
        Node {
            width: Val::Px(10.0),
            height: Val::Px(1.0),
            ..Node::default()
        },
        BackgroundColor(Color::srgb(1.0, 0.0, 1.0)),
        Text::from("clipped line"),
        ChildOf(root),
    ));

    app.update();
    app.update();

    insta::assert_snapshot!("bui_overflow_clip", composed_styled_frame(&app));
}

#[test]
fn wheel_scrolls_overflow_scroll_node() {
    let mut app = app(6, 2);
    let root = app
        .world_mut()
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            overflow: Overflow::scroll_y(),
            ..Node::default()
        })
        .id();
    for line in ["r1", "r2", "r3", "r4"] {
        app.world_mut().spawn((
            Node {
                height: Val::Px(1.0),
                flex_shrink: 0.0,
                ..Node::default()
            },
            Text::from(line),
            ChildOf(root),
        ));
    }

    app.update();
    app.update();
    insta::assert_snapshot!("bui_scroll_top", composed_styled_frame(&app));

    for _ in 0..5 {
        send_mouse(&mut app, MouseKind::ScrollDown, 2, 1);
    }
    app.update();

    assert_eq!(app.world().get::<ScrollPosition>(root).unwrap().0.y, 2.0);
    insta::assert_snapshot!("bui_scroll_clamped_bottom", composed_styled_frame(&app));
}

#[test]
fn two_cameras_partition_nodes() {
    let mut app = app(12, 2);
    let right_camera = app
        .world_mut()
        .spawn(TerminalCamera {
            order: 1,
            viewport: Viewport::Fixed(plurimus_core::ratatui_core::layout::Rect::new(6, 0, 6, 2)),
            ..TerminalCamera::default()
        })
        .id();
    app.world_mut().spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..Node::default()
        },
        Text::from("left"),
    ));
    app.world_mut().spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..Node::default()
        },
        Text::from("right"),
        bevy_ui::UiTargetCamera(right_camera),
    ));

    app.update();
    app.update();

    insta::assert_snapshot!("bui_two_cameras", composed_styled_frame(&app));
}

#[test]
fn global_z_index_reorders_trees() {
    let mut app = app(8, 2);
    let spawn_full = |app: &mut App, color: Color, z: Option<i32>| {
        let node = Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..Node::default()
        };
        match z {
            Some(z) => app
                .world_mut()
                .spawn((node, BackgroundColor(color), bevy_ui::GlobalZIndex(z)))
                .id(),
            None => app.world_mut().spawn((node, BackgroundColor(color))).id(),
        }
    };
    spawn_full(&mut app, Color::srgb(1.0, 0.0, 0.0), Some(1));
    spawn_full(&mut app, Color::srgb(0.0, 1.0, 0.0), None);
    spawn_full(&mut app, Color::srgb(0.0, 0.0, 1.0), Some(-1));

    app.update();
    app.update();

    insta::assert_snapshot!("bui_global_z", composed_styled_frame(&app));
}

#[test]
fn styled_spans_paint_per_segment_and_wrap() {
    use plurimus_bui::TextSpan;
    use plurimus_core::ratatui_core::style::{Color as CellColor, Style};

    let mut app = app(10, 3);
    app.world_mut().spawn((
        Node {
            width: Val::Px(10.0),
            ..Node::default()
        },
        Text(vec![
            TextSpan::new("warm wo", Style::new().fg(CellColor::Red)),
            TextSpan::new("rds here", Style::new().fg(CellColor::Blue)),
        ]),
        TextStyle(Style::new().bg(CellColor::Black)),
    ));

    app.update();
    app.update();

    insta::assert_snapshot!("bui_text_spans", composed_styled_frame(&app));
}
