//! Widget logic driven by bevy_ui layout: nodes activate like widgets.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input_focus::tab_navigation::TabIndex;
use bevy_input_focus::{FocusCause, InputFocus};
use bevy_ui::{FlexDirection, Node, Val};
use plurimus_bui::BuiPlugin;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_input::{KeyCode, MouseButton, MouseKind};
use plurimus_test::{press_key, send_mouse};
use plurimus_widgets::{Activate, Button, WidgetsPlugin};

#[derive(Resource, Default)]
struct Activations(u32);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, BuiPlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize { cols: 20, rows: 6 });
    app.init_resource::<Activations>();
    app.world_mut().spawn(TerminalCamera::default());
    app
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one arrange-drive-assert body; splitting it hides what the test drives"
)]
fn node_button_activates_by_click_and_enter() {
    let mut app = app();

    let root = app
        .world_mut()
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            padding: bevy_ui::UiRect::all(Val::Px(1.0)),
            ..Node::default()
        })
        .id();
    let button: Entity = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(8.0),
                height: Val::Px(1.0),
                ..Node::default()
            },
            Button,
            TabIndex(0),
            ChildOf(root),
        ))
        .id();
    app.world_mut()
        .entity_mut(button)
        .observe(|_on: On<Activate>, mut count: ResMut<Activations>| count.0 += 1);

    app.update();
    app.update();

    send_mouse(&mut app, MouseKind::Moved, 2, 1);
    assert_eq!(
        app.world()
            .get::<plurimus_ui::ComputedWidgetArea>(button)
            .map(|area| area.0),
        Some(plurimus_core::ratatui_core::layout::Rect::new(1, 1, 8, 1)),
        "bridge should mirror the node rect"
    );
    assert_eq!(
        app.world().get::<plurimus_ui::Hovered>(button),
        Some(&plurimus_ui::Hovered(true)),
        "cursor at 2,1 should hover the node"
    );
    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 1);
    assert_eq!(
        app.world().resource::<InputFocus>().get(),
        Some(button),
        "click on the node should focus it"
    );
    send_mouse(&mut app, MouseKind::Up(MouseButton::Left), 2, 1);
    assert_eq!(app.world().resource::<Activations>().0, 1);

    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(button, FocusCause::Navigated);
    press_key(&mut app, KeyCode::Enter);
    assert_eq!(app.world().resource::<Activations>().0, 2);
}

#[test]
fn node_text_field_edits_after_click_focus() {
    let mut app = app();
    let field = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(10.0),
                height: Val::Px(1.0),
                ..Node::default()
            },
            plurimus_widgets::EditableText,
            TabIndex(0),
        ))
        .id();
    app.update();
    app.update();

    send_mouse(&mut app, MouseKind::Moved, 2, 0);
    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 0);
    assert_eq!(
        app.world().resource::<InputFocus>().get(),
        Some(field),
        "click on the node should focus the field"
    );

    press_key(&mut app, KeyCode::Char('z'));
    let text = app
        .world()
        .get::<plurimus_widgets::TextInput>(field)
        .unwrap();
    assert_eq!(text.value(), "z");
}

#[test]
fn tab_index_only_nodes_join_directional_navigation() {
    let mut app = app();
    let root = app
        .world_mut()
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(2.0),
            ..Node::default()
        })
        .id();
    let focusable = |app: &mut App, index: i32| {
        app.world_mut()
            .spawn((
                Node {
                    width: Val::Px(4.0),
                    height: Val::Px(1.0),
                    ..Node::default()
                },
                TabIndex(index),
                ChildOf(root),
            ))
            .id()
    };
    let left = focusable(&mut app, 0);
    let right = focusable(&mut app, 1);
    app.update();
    app.update();

    assert_eq!(
        app.world()
            .get::<plurimus_bui::ComputedNodeRect>(left)
            .map(|rect| rect.rect),
        Some(plurimus_core::ratatui_core::layout::Rect::new(0, 0, 4, 1)),
        "layout should publish the laid-out cell rect"
    );
    assert!(
        app.world().get::<plurimus_ui::Hovered>(left).is_none(),
        "node never opted into hover"
    );

    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(left, FocusCause::Navigated);
    press_key(&mut app, KeyCode::Right);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(right));
}

#[test]
fn content_rect_excludes_border_and_padding() {
    use bevy_ui::UiRect;

    let mut app = app();
    let node = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(10.0),
                height: Val::Px(5.0),
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::all(Val::Px(1.0)),
                ..Node::default()
            },
            TabIndex(0),
        ))
        .id();
    app.update();
    app.update();

    let rects = *app
        .world()
        .get::<plurimus_bui::ComputedNodeRect>(node)
        .unwrap();
    assert_eq!(
        rects.rect,
        plurimus_core::ratatui_core::layout::Rect::new(0, 0, 10, 5)
    );
    assert_eq!(
        rects.content,
        plurimus_core::ratatui_core::layout::Rect::new(2, 2, 6, 1)
    );
}
