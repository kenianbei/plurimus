//! Snapshot tests for the bevy_ui-to-cells rasterizer.

use bevy_app::App;
use bevy_color::Color;
use bevy_ecs::prelude::ChildOf;
use bevy_ui::widget::Text;
use bevy_ui::{
    BackgroundColor, BorderColor, FlexDirection, Node, Overflow, ScrollPosition, UiRect, Val,
};
use plurimus_bui::bevy_text::{
    FontStyle, FontWeight, Justify, LineBreak, Strikethrough, TextColor, TextFont, TextLayout,
    TextSpan, Underline,
};
use plurimus_bui::{BuiPlugin, ComputedNodeRect};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize, Viewport};
use plurimus_term::MouseKind;
use plurimus_test::{composed_frame, composed_styled_frame, send_mouse};

fn app(cols: u16, rows: u16) -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, BuiPlugin));
    app.insert_resource(TerminalSize::new(cols, rows));
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
        TextColor(Color::srgb(1.0, 1.0, 0.0)),
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
        .spawn(
            TerminalCamera::default()
                .with_order(1)
                .with_viewport(Viewport::Fixed(
                    plurimus_core::ratatui_core::layout::Rect::new(6, 0, 6, 2),
                )),
        )
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
    let mut app = app(10, 3);
    let root = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(10.0),
                ..Node::default()
            },
            BackgroundColor(Color::BLACK),
            Text::new("warm wo"),
            TextColor(Color::srgb(1.0, 0.0, 0.0)),
        ))
        .id();
    app.world_mut().spawn((
        TextSpan::new("rds here"),
        TextColor(Color::srgb(0.0, 0.0, 1.0)),
        TextFont {
            weight: FontWeight::BOLD,
            style: FontStyle::Italic,
            ..TextFont::default()
        },
        Underline,
        Strikethrough,
        ChildOf(root),
    ));

    app.update();
    app.update();

    insta::assert_snapshot!("bui_text_spans", composed_styled_frame(&app));
}

#[test]
fn a_span_edit_relayouts_its_root() {
    let mut app = app(10, 3);
    let root = app
        .world_mut()
        .spawn((Node::default(), Text::new("ab")))
        .id();
    let span = app
        .world_mut()
        .spawn((TextSpan::new("c"), ChildOf(root)))
        .id();
    app.update();
    app.update();
    assert_eq!(composed_frame(&app).lines().next(), Some("abc       "));

    app.world_mut().get_mut::<TextSpan>(span).unwrap().0 = "cdef".to_owned();
    app.update();
    app.update();
    assert_eq!(composed_frame(&app).lines().next(), Some("abcdef    "));
}

#[test]
fn transparent_text_paints_no_glyphs() {
    let mut app = app(6, 1);
    app.world_mut().spawn((
        Node::default(),
        BackgroundColor(Color::BLACK),
        Text::new("gone"),
        TextColor(Color::NONE),
    ));

    app.update();
    app.update();

    let frame = composed_styled_frame(&app);
    assert!(!frame.contains("gone"), "{frame}");
    assert!(frame.contains("bg:Some(Rgb(0, 0, 0))"), "{frame}");
}

#[test]
fn justify_and_nowrap_follow_the_layout() {
    let mut app = app(6, 3);
    let column = app
        .world_mut()
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Px(6.0),
            ..Node::default()
        })
        .id();
    for (text, layout) in [
        (
            "ab",
            TextLayout::new(Justify::Right, LineBreak::WordBoundary),
        ),
        (
            "cd",
            TextLayout::new(Justify::Center, LineBreak::WordBoundary),
        ),
        (
            "hello world",
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
        ),
    ] {
        app.world_mut()
            .spawn((Node::default(), Text::new(text), layout, ChildOf(column)));
    }

    app.update();
    app.update();

    assert_eq!(composed_frame(&app), "    ab\n  cd  \nhello ");
}

#[test]
fn an_em_and_a_rem_are_each_one_cell() {
    let mut app = app(10, 4);
    let node = app
        .world_mut()
        .spawn(Node {
            width: Val::Em(2.0),
            height: Val::Rem(3.0),
            ..Node::default()
        })
        .id();

    app.update();
    app.update();

    let rect = app.world().get::<ComputedNodeRect>(node).unwrap().rect;
    assert_eq!((rect.width, rect.height), (2, 3));
}
