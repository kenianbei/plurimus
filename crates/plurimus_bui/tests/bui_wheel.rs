//! Upstream `ScrollArea`s on `bevy_ui` nodes, fed by the terminal's wheel.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::ChildOf;
use bevy_math::Vec2;
use bevy_ui::widget::Text;
use bevy_ui::{FlexDirection, Node, Overflow, ScrollPosition, Val};
use plurimus_bui::BuiPlugin;
use plurimus_bui::bevy_picking::Pickable;
use plurimus_bui::bevy_ui_widgets::ScrollArea as NodeScrollArea;
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::MouseKind;
use plurimus_test::send_mouse;
use plurimus_ui::{ModalOpen, ScrollArea, ScrollOffset, UiArea, UiWidget};
use plurimus_widgets::WidgetsPlugin;
use plurimus_widgets::ratatui_widgets::paragraph::Paragraph;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, BuiPlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(8, 3));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

/// A scroll area of `size` cells, with no children yet.
fn scroll_node(app: &mut App, overflow: Overflow, size: (f32, f32)) -> Entity {
    app.world_mut()
        .spawn((
            Node {
                width: Val::Px(size.0),
                height: Val::Px(size.1),
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
                overflow,
                ..Node::default()
            },
            NodeScrollArea,
        ))
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
fn a_tick_scrolls_one_row_and_clamps_at_the_end() {
    let mut app = app();
    let node = spawn_node(&mut app, Overflow::scroll_y(), (8.0, 3.0));
    settle(&mut app);

    send_mouse(&mut app, MouseKind::ScrollDown, 2, 1);
    assert_eq!(scrolled(&app, node).y, 1.0);
    send_mouse(&mut app, MouseKind::ScrollDown, 2, 1);
    send_mouse(&mut app, MouseKind::ScrollDown, 2, 1);
    assert_eq!(scrolled(&app, node).y, 2.0, "five rows in three");
    send_mouse(&mut app, MouseKind::ScrollUp, 2, 1);
    assert_eq!(scrolled(&app, node).y, 1.0);
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
fn an_open_plurimus_overlay_blocks_node_ticks() {
    let mut app = app();
    let node = spawn_node(&mut app, Overflow::scroll_y(), (8.0, 3.0));
    app.world_mut().spawn((
        ModalOpen,
        UiWidget::default(),
        UiArea::Fixed(Rect::new(6, 2, 2, 1)),
    ));
    settle(&mut app);

    send_mouse(&mut app, MouseKind::ScrollDown, 2, 1);

    assert_eq!(scrolled(&app, node), Vec2::ZERO);
}

/// A childless node covering the screen; a child of its own would be
/// picked in its place.
#[test]
fn a_pickable_ignore_node_lets_the_tick_through() {
    let mut app = app();
    let below = spawn_node(&mut app, Overflow::scroll_y(), (8.0, 3.0));
    let above = scroll_node(&mut app, Overflow::clip(), (8.0, 3.0));
    app.world_mut().entity_mut(above).insert(Pickable::IGNORE);
    settle(&mut app);

    send_mouse(&mut app, MouseKind::ScrollDown, 2, 1);

    assert_eq!(scrolled(&app, below).y, 1.0, "the cover is not picked");
}

#[test]
fn a_node_that_stops_being_a_scroll_area_stops_scrolling() {
    let mut app = app();
    let node = spawn_node(&mut app, Overflow::scroll_y(), (8.0, 3.0));
    settle(&mut app);
    app.world_mut().entity_mut(node).remove::<NodeScrollArea>();

    send_mouse(&mut app, MouseKind::ScrollDown, 2, 1);

    assert_eq!(scrolled(&app, node), Vec2::ZERO);
}

// The inner node is child 0 so it sits at the top-left, where the tick
// lands; the outer node's own rows follow and make it overflow too.
#[test]
fn a_nested_scroll_area_wins_over_its_ancestor() {
    let mut app = app();
    let outer = scroll_node(&mut app, Overflow::scroll_y(), (8.0, 3.0));
    let inner = scroll_node(&mut app, Overflow::scroll_y(), (4.0, 2.0));
    app.world_mut().entity_mut(inner).insert(ChildOf(outer));
    add_rows(&mut app, inner, 5);
    add_rows(&mut app, outer, 4);
    settle(&mut app);

    send_mouse(&mut app, MouseKind::ScrollDown, 1, 0);

    assert_eq!(scrolled(&app, inner).y, 1.0, "innermost consumes it");
    assert_eq!(scrolled(&app, outer), Vec2::ZERO);
}
