//! Upstream widgets on `bevy_ui` nodes, picked through the terminal's mouse.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input_focus::InputFocus;
use bevy_input_focus::tab_navigation::TabIndex;
use bevy_ui::{FlexDirection, Node, PositionType, UiRect, Val};
use plurimus_bui::BuiPlugin;
use plurimus_bui::bevy_picking::Pickable;
use plurimus_bui::bevy_ui_widgets::Button;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize, UiArea, UiWidget};
use plurimus_term::{KeyCode, MouseButton, MouseKind};
use plurimus_test::{click, press_key, release_at, send_mouse, set_focus};
use plurimus_ui::{Activate, Hovered, InteractionDisabled, ModalOpen, UiLabel};
use plurimus_widgets::WidgetsPlugin;

#[derive(Resource, Default)]
struct Activations(Vec<Entity>);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, BuiPlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 6));
    app.init_resource::<Activations>();
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn count_activations(app: &mut App, entity: Entity) {
    app.world_mut()
        .entity_mut(entity)
        .observe(|on: On<Activate>, mut log: ResMut<Activations>| {
            log.0.push(on.entity);
        });
}

fn activations(app: &App) -> &[Entity] {
    &app.world().resource::<Activations>().0
}

fn full_screen() -> Node {
    Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        ..Node::default()
    }
}

/// A one-row button at the top-left of `parent`, hoverable and a tab stop.
fn button(app: &mut App, parent: Entity) -> Entity {
    let button = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(8.0),
                height: Val::Px(1.0),
                ..Node::default()
            },
            Button,
            TabIndex(0),
            Hovered::default(),
            ChildOf(parent),
        ))
        .id();
    count_activations(app, button);
    button
}

fn settle(app: &mut App) {
    app.update();
    app.update();
}

#[test]
fn a_node_button_activates_by_click_and_enter() {
    let mut app = app();
    let root = app
        .world_mut()
        .spawn(Node {
            padding: UiRect::all(Val::Px(1.0)),
            ..full_screen()
        })
        .id();
    let button = button(&mut app, root);
    settle(&mut app);

    send_mouse(&mut app, MouseKind::Moved, 2, 1);
    assert_eq!(app.world().get::<Hovered>(button), Some(&Hovered(true)));

    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 1);
    assert_eq!(
        app.world().resource::<InputFocus>().get(),
        Some(button),
        "a press focuses the node"
    );
    release_at(&mut app, 2, 1);
    assert_eq!(activations(&app), [button]);

    set_focus(&mut app, button);
    press_key(&mut app, KeyCode::Enter);
    assert_eq!(activations(&app), [button, button]);
}

#[test]
fn a_plurimus_widget_over_a_node_takes_the_click() {
    let mut app = app();
    let root = app.world_mut().spawn(full_screen()).id();
    let node = button(&mut app, root);
    let widget = app
        .world_mut()
        .spawn((
            plurimus_widgets::Button,
            UiLabel("over".into()),
            UiWidget::default(),
            UiArea::Fixed(Rect::new(0, 0, 20, 3)),
        ))
        .id();
    count_activations(&mut app, widget);
    settle(&mut app);

    click(&mut app, 2, 0);

    assert_eq!(activations(&app), [widget]);
    assert_eq!(
        app.world().get::<Hovered>(node),
        Some(&Hovered(false)),
        "the node beneath is never hovered"
    );
}

#[test]
fn an_open_plurimus_overlay_blocks_node_hits() {
    let mut app = app();
    let root = app.world_mut().spawn(full_screen()).id();
    button(&mut app, root);
    app.world_mut().spawn((
        ModalOpen,
        UiWidget::default(),
        UiArea::Fixed(Rect::new(12, 4, 8, 2)),
    ));
    settle(&mut app);

    click(&mut app, 2, 0);

    assert_eq!(activations(&app), &[] as &[Entity]);
}

/// An overlay built of nodes, as a form is, over a page of nodes: a press
/// on it reaches its own buttons and none it is drawn over.
#[test]
fn a_press_inside_an_open_overlay_reaches_only_its_own_nodes() {
    let mut app = app();
    let page = app
        .world_mut()
        .spawn(Node {
            padding: UiRect::top(Val::Px(2.0)),
            ..full_screen()
        })
        .id();
    let beneath = button(&mut app, page);
    let overlay = app
        .world_mut()
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(20.0),
                height: Val::Px(6.0),
                flex_direction: FlexDirection::Column,
                ..Node::default()
            },
            ModalOpen,
            UiWidget::default(),
            UiArea::Fixed(Rect::new(0, 0, 20, 6)),
        ))
        .id();
    let own = button(&mut app, overlay);
    settle(&mut app);

    click(&mut app, 2, 0);
    click(&mut app, 2, 2);

    assert_eq!(activations(&app), &[own], "beneath {beneath:?} is covered");
}

#[test]
fn a_pickable_ignore_node_falls_through() {
    let mut app = app();
    let root = app.world_mut().spawn(full_screen()).id();
    let node = button(&mut app, root);
    app.world_mut().spawn((
        Node {
            position_type: PositionType::Absolute,
            ..full_screen()
        },
        Pickable::IGNORE,
        ChildOf(root),
    ));
    settle(&mut app);

    click(&mut app, 2, 0);

    assert_eq!(activations(&app), [node]);
}

#[test]
fn content_rect_excludes_border_and_padding() {
    let mut app = app();
    let node = app
        .world_mut()
        .spawn(Node {
            width: Val::Px(10.0),
            height: Val::Px(5.0),
            border: UiRect::all(Val::Px(1.0)),
            padding: UiRect::all(Val::Px(1.0)),
            ..Node::default()
        })
        .id();
    settle(&mut app);

    let rects = *app
        .world()
        .get::<plurimus_bui::ComputedNodeRect>(node)
        .unwrap();
    assert_eq!(rects.rect, Rect::new(0, 0, 10, 5));
    assert_eq!(rects.content, Rect::new(2, 2, 6, 1));
}

// A disabled node still blocks what is beneath it, as on desktop: the
// press reaches it and stops there.
#[test]
fn a_disabled_node_absorbs_the_press() {
    let mut app = app();
    let root = app.world_mut().spawn((full_screen(), Button)).id();
    count_activations(&mut app, root);
    let cover = button(&mut app, root);
    app.world_mut()
        .entity_mut(cover)
        .insert(InteractionDisabled);
    settle(&mut app);

    click(&mut app, 2, 0);

    assert_eq!(activations(&app), &[] as &[Entity]);
}
