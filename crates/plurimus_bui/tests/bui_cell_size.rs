//! A tree laid out in a window's pixels lands in cells through `CellSize`.

use bevy_app::App;
use bevy_color::Color;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_ui::widget::Text;
use bevy_ui::{AlignItems, BackgroundColor, BorderColor, FlexDirection, Node, UiRect, Val};
use plurimus_bui::bevy_ui_widgets::Button;
use plurimus_bui::{BuiPlugin, CellSize, ComputedNodeRect};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::{click, composed_frame};
use plurimus_ui::Activate;

const CELL: CellSize = CellSize { x: 8.0, y: 16.0 };

#[derive(Resource, Default)]
struct Activations(u32);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, BuiPlugin));
    app.insert_resource(TerminalSize::new(80, 4));
    app.insert_resource(CELL);
    app.init_resource::<Activations>();
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn settle(app: &mut App) {
    app.update();
    app.update();
}

fn rect(app: &App, entity: Entity) -> Rect {
    app.world().get::<ComputedNodeRect>(entity).unwrap().rect
}

#[test]
fn pixels_rems_and_text_resolve_against_the_cell_size() {
    let mut app = app();
    let column = app
        .world_mut()
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Start,
            ..Node::default()
        })
        .id();
    let card = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(560.0),
                height: Val::Px(16.0),
                ..Node::default()
            },
            ChildOf(column),
        ))
        .id();
    let line = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(80.0),
                height: Val::Rem(1.0),
                ..Node::default()
            },
            ChildOf(column),
        ))
        .id();
    let text = app
        .world_mut()
        .spawn((Node::default(), Text::new("hello"), ChildOf(column)))
        .id();
    settle(&mut app);

    assert_eq!(rect(&app, card), Rect::new(0, 0, 70, 1));
    assert_eq!(rect(&app, line), Rect::new(0, 1, 10, 1));
    assert_eq!(rect(&app, text), Rect::new(0, 2, 5, 1));
}

#[test]
fn a_thin_border_paints_in_the_outer_cell() {
    let mut app = app();
    app.world_mut().spawn((
        Node {
            width: Val::Px(160.0),
            height: Val::Px(48.0),
            border: UiRect::all(Val::Px(1.0)),
            ..Node::default()
        },
        BorderColor::all(Color::WHITE),
        BackgroundColor(Color::BLACK),
    ));
    settle(&mut app);

    let frame = composed_frame(&app);
    let rows: Vec<String> = frame
        .lines()
        .map(|row| row.chars().take(20).collect())
        .collect();
    assert_eq!(rows[0], "┌──────────────────┐", "{frame}");
    assert_eq!(rows[1], "│                  │", "{frame}");
    assert_eq!(rows[2], "└──────────────────┘", "{frame}");
}

#[test]
fn a_click_lands_on_the_node_whose_pixels_cover_the_cell() {
    let mut app = app();
    let row = app.world_mut().spawn(Node::default()).id();
    app.world_mut().spawn((
        Node {
            width: Val::Px(80.0),
            height: Val::Px(16.0),
            ..Node::default()
        },
        ChildOf(row),
    ));
    let button = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(160.0),
                height: Val::Px(16.0),
                ..Node::default()
            },
            Button,
            ChildOf(row),
        ))
        .id();
    app.world_mut()
        .entity_mut(button)
        .observe(|_: On<Activate>, mut count: ResMut<Activations>| count.0 += 1);
    settle(&mut app);
    assert_eq!(rect(&app, button), Rect::new(10, 0, 20, 1));

    click(&mut app, 15, 0);
    assert_eq!(app.world().resource::<Activations>().0, 1);
    click(&mut app, 5, 0);
    assert_eq!(app.world().resource::<Activations>().0, 1, "the spacer");
}
