//! Proves `bevy_ui/taffy` layout resolves in exact terminal cells.

use bevy_app::App;
use bevy_ui::{ComputedNode, FlexDirection, Node, UiRect, Val};
use plurimus_bui::BuiPlugin;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};

fn app(cols: u16, rows: u16) -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, BuiPlugin));
    app.insert_resource(TerminalSize { cols, rows });
    app.world_mut().spawn(TerminalCamera::default());
    app
}

#[test]
fn flex_column_resolves_in_cells() {
    let mut app = app(20, 10);
    let root = app
        .world_mut()
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..Node::default()
        })
        .id();
    let top = app
        .world_mut()
        .spawn((
            Node {
                flex_grow: 1.0,
                ..Node::default()
            },
            bevy_ecs::prelude::ChildOf(root),
        ))
        .id();
    let bottom = app
        .world_mut()
        .spawn((
            Node {
                flex_grow: 1.0,
                ..Node::default()
            },
            bevy_ecs::prelude::ChildOf(root),
        ))
        .id();

    app.update();
    app.update();

    let size = |entity| app.world().get::<ComputedNode>(entity).unwrap().size;
    assert_eq!(size(root).x, 20.0);
    assert_eq!(size(root).y, 10.0);
    assert_eq!(size(top), size(bottom));
    assert_eq!(size(top).y, 5.0);
}

#[test]
fn padding_and_fixed_sizes_resolve_in_cells() {
    let mut app = app(30, 8);
    let root = app
        .world_mut()
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            padding: UiRect::all(Val::Px(2.0)),
            ..Node::default()
        })
        .id();
    let child = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(10.0),
                height: Val::Percent(100.0),
                ..Node::default()
            },
            bevy_ecs::prelude::ChildOf(root),
        ))
        .id();

    app.update();
    app.update();

    let child_node = app.world().get::<ComputedNode>(child).unwrap();
    assert_eq!(child_node.size.x, 10.0);
    assert_eq!(child_node.size.y, 4.0);
}

#[test]
fn text_nodes_measure_by_grapheme_width() {
    let mut app = app(30, 8);
    let auto = app
        .world_mut()
        .spawn(plurimus_bui::Text::from("hello plurimus"))
        .id();
    let wrapped = app
        .world_mut()
        .spawn((
            plurimus_bui::Text::from("hello wide world"),
            Node {
                width: Val::Px(10.0),
                ..Node::default()
            },
        ))
        .id();

    app.update();
    app.update();

    let size = |entity| app.world().get::<ComputedNode>(entity).unwrap().size;
    assert_eq!(size(auto), bevy_math::Vec2::new(14.0, 1.0));
    assert_eq!(size(wrapped), bevy_math::Vec2::new(10.0, 2.0));
}
