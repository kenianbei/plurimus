//! bui nodes as targets of the shared wheel router, fully headless.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::ChildOf;
use bevy_math::Vec2;
use bevy_ui::{FlexDirection, Node, Overflow, ScrollPosition, Val};
use plurimus_bui::BuiPlugin;
use plurimus_bui::Text;
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::MouseKind;
use plurimus_test::send_mouse;
use plurimus_ui::{InteractionDisabled, ScrollArea, ScrollOffset, UiArea, UiOrder, UiWidget};
use plurimus_widgets::WidgetsPlugin;
use plurimus_widgets::ratatui_widgets::paragraph::Paragraph;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, BuiPlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(8, 3));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

/// A scroll-overflow node of `size` cells, with no children yet.
fn scroll_node(app: &mut App, overflow: Overflow, size: (f32, f32)) -> Entity {
    app.world_mut()
        .spawn(Node {
            width: Val::Px(size.0),
            height: Val::Px(size.1),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            overflow,
            ..Node::default()
        })
        .id()
}

/// Appends `count` one-cell rows, enough of them to overflow `parent`.
fn add_rows(app: &mut App, parent: Entity, count: usize) {
    for row in 0..count {
        app.world_mut().spawn((
            Node {
                height: Val::Px(1.0),
                flex_shrink: 0.0,
                ..Node::default()
            },
            Text::from(format!("r{row}")),
            ChildOf(parent),
        ));
    }
}

fn spawn_node(app: &mut App, overflow: Overflow, size: (f32, f32)) -> Entity {
    let node = scroll_node(app, overflow, size);
    add_rows(app, node, 5);
    node
}

fn scrolled(app: &App, node: Entity) -> Vec2 {
    app.world().get::<ScrollPosition>(node).unwrap().0
}

fn settle(app: &mut App) {
    app.update();
    app.update();
}

#[test]
fn a_widget_stacked_over_a_node_takes_the_tick_alone() {
    let mut app = app();
    let node = spawn_node(&mut app, Overflow::scroll_y(), (8.0, 3.0));
    let widget = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("over")),
            UiArea::Fixed(Rect::new(0, 0, 8, 3)),
            ScrollArea::new(Size::new(8, 30)),
        ))
        .id();
    settle(&mut app);

    send_mouse(&mut app, MouseKind::ScrollDown, 2, 1);

    assert_eq!(
        app.world().get::<ScrollOffset>(widget).unwrap().0,
        Position::new(0, 1)
    );
    assert_eq!(scrolled(&app, node), Vec2::ZERO, "bui sits below widgets");
}

#[test]
fn a_disabled_node_falls_through_to_the_one_beneath() {
    let mut app = app();
    let below = spawn_node(&mut app, Overflow::scroll_y(), (8.0, 3.0));
    let above = spawn_node(&mut app, Overflow::scroll_y(), (8.0, 3.0));
    settle(&mut app);
    app.world_mut()
        .entity_mut(above)
        .insert(InteractionDisabled);

    send_mouse(&mut app, MouseKind::ScrollDown, 2, 1);

    assert_eq!(scrolled(&app, above), Vec2::ZERO);
    assert_eq!(scrolled(&app, below).y, 1.0);
}

#[test]
fn a_node_that_stops_scrolling_stops_taking_ticks() {
    let mut app = app();
    let below = spawn_node(&mut app, Overflow::scroll_y(), (8.0, 3.0));
    let above = spawn_node(&mut app, Overflow::scroll_y(), (8.0, 3.0));
    settle(&mut app);

    app.world_mut().entity_mut(above).insert(Node {
        width: Val::Px(8.0),
        height: Val::Px(3.0),
        flex_direction: FlexDirection::Column,
        flex_shrink: 0.0,
        overflow: Overflow::clip(),
        ..Node::default()
    });
    settle(&mut app);
    send_mouse(&mut app, MouseKind::ScrollDown, 2, 1);

    assert_eq!(scrolled(&app, above), Vec2::ZERO, "deregistered");
    assert_eq!(scrolled(&app, below).y, 1.0);
}

#[test]
fn an_explicit_order_on_a_node_survives_the_bridge() {
    let mut app = app();
    let node = spawn_node(&mut app, Overflow::scroll_y(), (8.0, 3.0));
    app.world_mut().entity_mut(node).insert(UiOrder(7));
    settle(&mut app);

    assert_eq!(app.world().get::<UiOrder>(node), Some(&UiOrder(7)));
}

// The inner node is child 0 so it sits at the top-left, where the tick
// lands; the outer node's own rows follow and make it overflow too.
fn spawn_nested(app: &mut App, inner_overflow: Overflow) -> (Entity, Entity) {
    let outer = scroll_node(app, Overflow::scroll_y(), (8.0, 3.0));
    let inner = scroll_node(app, inner_overflow, (4.0, 2.0));
    app.world_mut().entity_mut(inner).insert(ChildOf(outer));
    add_rows(app, inner, 5);
    add_rows(app, outer, 4);
    (outer, inner)
}

#[test]
fn a_nested_node_wins_over_its_scrolling_ancestor() {
    let mut app = app();
    let (outer, inner) = spawn_nested(&mut app, Overflow::scroll_y());
    settle(&mut app);

    send_mouse(&mut app, MouseKind::ScrollDown, 1, 0);

    assert_eq!(scrolled(&app, inner).y, 1.0, "innermost consumes it");
    assert_eq!(scrolled(&app, outer), Vec2::ZERO);
}

#[test]
fn a_tick_falls_through_a_node_that_scrolls_the_other_axis() {
    let mut app = app();
    let (outer, inner) = spawn_nested(&mut app, Overflow::scroll_x());
    settle(&mut app);

    send_mouse(&mut app, MouseKind::ScrollDown, 1, 0);

    assert_eq!(scrolled(&app, inner), Vec2::ZERO, "no y overflow to use");
    assert_eq!(scrolled(&app, outer).y, 1.0);
}
