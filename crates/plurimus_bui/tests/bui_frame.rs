//! A node beneath a widget in the tree draws over that widget.

use bevy_app::App;
use bevy_color::Color;
use bevy_ecs::prelude::{ChildOf, Entity};
use bevy_ui::widget::Text;
use bevy_ui::{BackgroundColor, Node, UiRect, Val};
use plurimus_bui::BuiPlugin;
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::widgets::Widget;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize, UiArea, UiOrder, UiWidget};
use plurimus_test::composed_frame;
use plurimus_widgets::ratatui_widgets::block::Block;
use plurimus_widgets::ratatui_widgets::clear::Clear;
use plurimus_widgets::ratatui_widgets::paragraph::Paragraph;

const COLS: u16 = 8;
const ROWS: u16 = 3;

/// An overlay's frame: clears what it covers, then draws its border.
struct Cover;

impl Widget for &Cover {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Clear.render(area, buffer);
        Block::bordered().render(area, buffer);
    }
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, BuiPlugin));
    app.insert_resource(TerminalSize::new(COLS, ROWS));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn frame(app: &mut App, order: UiOrder) -> Entity {
    app.world_mut()
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(1.0)),
                ..Node::default()
            },
            UiWidget::new(Cover),
            UiArea::Fill,
            order,
        ))
        .id()
}

fn row() -> Node {
    Node {
        width: Val::Percent(100.0),
        height: Val::Px(1.0),
        ..Node::default()
    }
}

fn settle(app: &mut App) -> Vec<String> {
    app.update();
    app.update();
    composed_frame(app).lines().map(str::to_owned).collect()
}

#[test]
fn a_text_inside_a_clearing_frame_draws_over_it() {
    let mut app = app();
    let root = frame(&mut app, UiOrder::OVERLAY);
    app.world_mut()
        .spawn((row(), Text::new("hi"), ChildOf(root)));

    let lines = settle(&mut app);

    assert_eq!(lines[1], "│hi    │");
}

#[test]
fn a_widget_above_the_frame_still_covers_its_text() {
    let mut app = app();
    let root = frame(&mut app, UiOrder(0));
    app.world_mut()
        .spawn((row(), Text::new("hi"), ChildOf(root)));
    app.world_mut().spawn((
        UiWidget::new(Paragraph::new("XXXXXXXX")),
        UiArea::Fixed(Rect::new(0, 1, COLS, 1)),
        UiOrder(1),
    ));

    let lines = settle(&mut app);

    assert_eq!(lines[1], "XXXXXXXX");
}

#[test]
fn a_widget_on_the_node_itself_keeps_covering_it() {
    let mut app = app();
    let root = frame(&mut app, UiOrder::OVERLAY);
    app.world_mut().spawn((
        row(),
        Text::new("node"),
        UiWidget::new(Paragraph::new("wdgt")),
        UiArea::Fixed(Rect::new(1, 1, 4, 1)),
        UiOrder(UiOrder::OVERLAY.0 + 1),
        ChildOf(root),
    ));

    let lines = settle(&mut app);

    assert_eq!(lines[1], "│wdgt  │");
}

#[test]
fn a_node_drawing_itself_is_not_lifted_over_its_frame() {
    let mut app = app();
    let root = frame(&mut app, UiOrder::OVERLAY);
    app.world_mut().spawn((
        row(),
        Text::new("no"),
        UiWidget::default(),
        UiArea::Fixed(Rect::new(1, 1, 2, 1)),
        ChildOf(root),
    ));

    let lines = settle(&mut app);

    assert_eq!(lines[1], "│      │", "the frame's clear covers it");
}

#[test]
fn framed_nodes_keep_their_stack_order() {
    let mut app = app();
    let root = frame(&mut app, UiOrder::OVERLAY);
    let band = app
        .world_mut()
        .spawn((
            row(),
            BackgroundColor(Color::srgb(1.0, 0.0, 0.0)),
            ChildOf(root),
        ))
        .id();
    app.world_mut()
        .spawn((row(), Text::new("ab"), ChildOf(band)));

    let lines = settle(&mut app);

    assert_eq!(lines[1], "│ab    │");
}
