//! Clipped bui nodes must not take input where they are not drawn.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use bevy_math::Vec2;
use bevy_ui::{FlexDirection, Node, Overflow, ScrollPosition, Val};
use plurimus_bui::BuiPlugin;
use plurimus_bui::Text;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{MouseButton, MouseKind};
use plurimus_test::{composed_frame, send_mouse};
use plurimus_ui::{Hovered, Pressed};

const COLS: f32 = 8.0;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, BuiPlugin));
    app.insert_resource(TerminalSize::new(8, 5));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn column(app: &mut App, height: f32) -> Entity {
    app.world_mut()
        .spawn(Node {
            width: Val::Px(COLS),
            height: Val::Px(height),
            flex_direction: FlexDirection::Column,
            ..Node::default()
        })
        .id()
}

fn scroll_container(app: &mut App, parent: Entity, height: f32) -> Entity {
    app.world_mut()
        .spawn((
            Node {
                width: Val::Px(COLS),
                height: Val::Px(height),
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
                overflow: Overflow::scroll_y(),
                ..Node::default()
            },
            ChildOf(parent),
        ))
        .id()
}

fn row(app: &mut App, parent: Entity, label: &str, height: f32) -> Entity {
    app.world_mut()
        .spawn((
            Node {
                height: Val::Px(height),
                flex_shrink: 0.0,
                ..Node::default()
            },
            Text::from(label.to_owned()),
            Hovered::default(),
            ChildOf(parent),
        ))
        .id()
}

fn is_pressed(app: &App, entity: Entity) -> bool {
    app.world().entity(entity).contains::<Pressed>()
}

fn press(app: &mut App, x: u16, y: u16) {
    send_mouse(app, MouseKind::Moved, x, y);
    send_mouse(app, MouseKind::Down(MouseButton::Left), x, y);
}

/// A header row above a two-row scroll container holding four rows,
/// scrolled down two: rows 0-1 are clipped away, and row 1's unclipped
/// rect lands exactly over the header.
fn scrolled_scene(app: &mut App) -> (Entity, Vec<Entity>) {
    let root = column(app, 5.0);
    let header = row(app, root, "header", 1.0);
    let container = scroll_container(app, root, 2.0);
    let rows = (0..4)
        .map(|index| row(app, container, &format!("row{index}"), 1.0))
        .collect();
    app.update();
    app.update();
    app.world_mut()
        .entity_mut(container)
        .insert(ScrollPosition(Vec2::new(0.0, 2.0)));
    app.update();
    app.update();
    (header, rows)
}

#[test]
fn a_scrolled_out_row_takes_no_press() {
    let mut app = app();
    let (header, rows) = scrolled_scene(&mut app);
    app.world_mut().entity_mut(header).remove::<Hovered>();
    press(&mut app, 2, 0);
    for (index, id) in rows.iter().enumerate() {
        assert!(!is_pressed(&app, *id), "row{index} is invisible there");
    }
}

#[test]
fn the_header_wins_its_own_cell_over_an_invisible_row() {
    let mut app = app();
    let (header, rows) = scrolled_scene(&mut app);
    press(&mut app, 2, 0);
    assert!(is_pressed(&app, header));
    assert!(!is_pressed(&app, rows[1]));
}

#[test]
fn a_visible_scrolled_row_is_still_pressable() {
    let mut app = app();
    let (_, rows) = scrolled_scene(&mut app);
    press(&mut app, 2, 1);
    assert!(
        is_pressed(&app, rows[2]),
        "row2 is drawn at the container top"
    );
}

#[test]
fn a_partially_clipped_row_presses_only_where_drawn() {
    let mut app = app();
    let root = column(&mut app, 5.0);
    let container = scroll_container(&mut app, root, 2.0);
    let tall = row(&mut app, container, "tall", 3.0);
    // Plain node, no Hovered: nothing else claims the cell below the
    // container, so only the leak could take the press there.
    app.world_mut().spawn((
        Node {
            height: Val::Px(1.0),
            flex_shrink: 0.0,
            ..Node::default()
        },
        Text::from("below".to_owned()),
        ChildOf(root),
    ));
    app.update();
    app.update();

    press(&mut app, 2, 1);
    assert!(is_pressed(&app, tall), "inside the container");

    send_mouse(&mut app, MouseKind::Up(MouseButton::Left), 2, 1);
    press(&mut app, 2, 2);
    assert!(
        !is_pressed(&app, tall),
        "its third row is clipped by the container"
    );
}

#[test]
fn rendering_clips_scroll_content_to_the_container() {
    let mut app = app();
    let root = column(&mut app, 5.0);
    let container = scroll_container(&mut app, root, 2.0);
    let mut tall = Node {
        height: Val::Px(3.0),
        flex_shrink: 0.0,
        ..Node::default()
    };
    tall.width = Val::Px(COLS);
    app.world_mut().spawn((
        tall,
        Text::from("AAAAAAAA\nBBBBBBBB\nCCCCCCCC".to_owned()),
        ChildOf(container),
    ));
    row(&mut app, root, "below", 1.0);
    app.update();
    app.update();
    let frame = composed_frame(&app);
    assert!(frame.contains("BBBBBBBB"), "second row is inside: {frame}");
    assert!(!frame.contains("CCCCCCCC"), "third row is clipped: {frame}");
    assert!(
        frame.contains("below"),
        "the sibling shows through: {frame}"
    );
}
