//! Upstream widgets on `bevy_ui` nodes, driven through the terminal's mouse
//! and keys. Upstream's own tests cover each widget's state machine; these
//! pin the numbers that hold only when the pointer lands in cells and rects
//! are read after upstream's post-layout writes.

use std::time::Duration;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Bundle, ChildOf, On, ResMut, Resource};
use bevy_input_focus::InputFocus;
use bevy_input_focus::tab_navigation::{NavAction, TabIndex};
use bevy_time::TimeUpdateStrategy;
use bevy_ui::{
    FixedNode, FlexDirection, Node, Overflow, PositionType, ScrollPosition, Selected, Val,
};
use plurimus_bui::bevy_ui_widgets::popover::{Popover, PopoverPlacement, PopoverSide};
use plurimus_bui::bevy_ui_widgets::{
    Checkbox, ControlOrientation, ListBox, ListItem, MenuAction, MenuEvent, MenuFocusState,
    MenuItem, MenuPopup, ModalDialog, RadioButton, RadioGroup, Scrollbar, ScrollbarThumb,
    SelectedTab, Slider, SliderRange, SliderThumb, SliderValue, Tab, TabList, checkbox_self_update,
    listbox_update_selection, radio_self_update, slider_self_update, tablist_self_update,
};
use plurimus_bui::{BuiPlugin, ComputedNodeRect};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{KeyCode, MouseButton, MouseKind};
use plurimus_test::{click, press_at, press_key, release_at, send_mouse};
use plurimus_ui::Checked;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, BuiPlugin));
    app.insert_resource(TerminalSize::new(40, 12));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn settle(app: &mut App) {
    app.update();
    app.update();
}

fn px(width: f32, height: f32) -> Node {
    Node {
        width: Val::Px(width),
        height: Val::Px(height),
        flex_shrink: 0.0,
        ..Node::default()
    }
}

fn root(app: &mut App) -> Entity {
    app.world_mut()
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..Node::default()
        })
        .id()
}

fn children<B: Bundle>(
    app: &mut App,
    parent: Entity,
    count: usize,
    child: impl Fn() -> B,
) -> Vec<Entity> {
    (0..count)
        .map(|_| app.world_mut().spawn((child(), ChildOf(parent))).id())
        .collect()
}

fn rect(app: &App, node: Entity) -> ComputedNodeRect {
    *app.world().get::<ComputedNodeRect>(node).unwrap()
}

fn focus(app: &App) -> Option<Entity> {
    app.world().resource::<InputFocus>().get()
}

/// A drag over a 20-cell track with a 1-cell thumb moves 100 / 19 per cell.
#[test]
fn a_slider_drags_and_steps_by_key() {
    let mut app = app();
    let root = root(&mut app);
    let slider = app
        .world_mut()
        .spawn((
            px(20.0, 1.0),
            Slider::default(),
            SliderRange::new(0.0, 100.0),
            TabIndex(0),
            ChildOf(root),
        ))
        .observe(slider_self_update)
        .id();
    app.world_mut().spawn((
        Node {
            position_type: PositionType::Absolute,
            ..px(1.0, 1.0)
        },
        SliderThumb,
        ChildOf(slider),
    ));
    settle(&mut app);

    press_at(&mut app, 2, 0);
    for x in 3..=12 {
        send_mouse(&mut app, MouseKind::Drag(MouseButton::Left), x, 0);
    }
    release_at(&mut app, 12, 0);
    let hundredths =
        |app: &App| (app.world().get::<SliderValue>(slider).unwrap().0 * 100.0).round();
    assert_eq!(hundredths(&app), 5263.0, "ten cells of nineteen");

    press_key(&mut app, KeyCode::Right);
    assert_eq!(hundredths(&app), 5363.0);
}

/// Twelve rows in a four-row area: the thumb is a third of the track, and
/// dragging it the rest of the way scrolls to the last eight rows.
#[test]
fn a_scrollbar_thumb_follows_the_offset_and_drags_it() {
    let mut app = app();
    let root = root(&mut app);
    let area = app
        .world_mut()
        .spawn((
            Node {
                overflow: Overflow::scroll_y(),
                flex_direction: FlexDirection::Column,
                ..px(10.0, 4.0)
            },
            ScrollPosition::default(),
            ChildOf(root),
        ))
        .id();
    children(&mut app, area, 12, || px(10.0, 1.0));
    let bar = app
        .world_mut()
        .spawn((
            px(1.0, 4.0),
            Scrollbar::new(area, ControlOrientation::Vertical, 1.0),
            ChildOf(root),
        ))
        .id();
    let thumb = app
        .world_mut()
        .spawn((ScrollbarThumb::default(), ChildOf(bar)))
        .id();
    settle(&mut app);
    assert_eq!(rect(&app, thumb).rect, Rect::new(10, 0, 1, 1));

    press_at(&mut app, 10, 0);
    for y in 1..=3 {
        send_mouse(&mut app, MouseKind::Drag(MouseButton::Left), 10, y);
    }

    assert_eq!(app.world().get::<ScrollPosition>(area).unwrap().y, 8.0);
    assert_eq!(rect(&app, thumb).rect, Rect::new(10, 3, 1, 1));
}

#[test]
fn a_list_box_selects_by_click_and_by_key() {
    let mut app = app();
    let root = root(&mut app);
    let list = app
        .world_mut()
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..px(10.0, 3.0)
            },
            ListBox,
            TabIndex(0),
            ChildOf(root),
        ))
        .observe(listbox_update_selection)
        .id();
    let rows = children(&mut app, list, 3, || (px(10.0, 1.0), ListItem));
    settle(&mut app);
    let selected = |app: &App| {
        rows.iter()
            .position(|&row| app.world().get::<Selected>(row).is_some())
    };

    click(&mut app, 1, 1);
    assert_eq!(selected(&app), Some(1));
    assert_eq!(focus(&app), Some(list), "a press focuses the list");

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);
    assert_eq!(selected(&app), Some(2));
}

#[test]
fn a_tab_list_selects_by_click_and_by_key() {
    let mut app = app();
    let root = root(&mut app);
    let tabs = app
        .world_mut()
        .spawn((px(15.0, 1.0), TabList::default(), ChildOf(root)))
        .observe(tablist_self_update)
        .id();
    let tab = children(&mut app, tabs, 3, || (px(5.0, 1.0), Tab));
    settle(&mut app);
    let selected_tab = |app: &App| app.world().get::<SelectedTab>(tabs).unwrap().0;

    click(&mut app, 6, 0);
    assert_eq!(selected_tab(&app), Some(tab[1]));
    assert_eq!(focus(&app), Some(tab[1]), "a press focuses the tab");

    press_key(&mut app, KeyCode::Right);
    assert_eq!(focus(&app), Some(tab[2]));
    press_key(&mut app, KeyCode::Enter);
    assert_eq!(selected_tab(&app), Some(tab[2]));
}

#[test]
fn a_radio_group_checks_by_click_and_by_key() {
    let mut app = app();
    let root = root(&mut app);
    let group = app
        .world_mut()
        .spawn((px(15.0, 1.0), RadioGroup, TabIndex(0), ChildOf(root)))
        .observe(radio_self_update)
        .id();
    let buttons = children(&mut app, group, 3, || (px(5.0, 1.0), RadioButton));
    settle(&mut app);
    let checked = |app: &App| {
        buttons
            .iter()
            .position(|&button| app.world().get::<Checked>(button).is_some())
    };

    click(&mut app, 6, 0);
    assert_eq!(checked(&app), Some(1));
    assert_eq!(focus(&app), Some(group), "a press focuses the group");

    press_key(&mut app, KeyCode::Down);
    assert_eq!(checked(&app), Some(2));
}

#[test]
fn a_checkbox_toggles_by_click_and_by_key() {
    let mut app = app();
    let root = root(&mut app);
    let checkbox = app
        .world_mut()
        .spawn((px(3.0, 1.0), Checkbox, TabIndex(0), ChildOf(root)))
        .observe(checkbox_self_update)
        .id();
    settle(&mut app);

    click(&mut app, 1, 0);
    assert!(app.world().get::<Checked>(checkbox).is_some());

    press_key(&mut app, KeyCode::Char(' '));
    assert!(app.world().get::<Checked>(checkbox).is_none());
}

/// Placement reads the terminal as the popover's window: below the anchor's
/// last-but-one row does not fit, so it mirrors above.
#[test]
fn a_popover_mirrors_to_the_side_that_fits() {
    let mut app = app();
    let root = root(&mut app);
    let anchor = app
        .world_mut()
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(20.0),
                top: Val::Px(10.0),
                ..px(6.0, 1.0)
            },
            ChildOf(root),
        ))
        .id();
    let popover = app
        .world_mut()
        .spawn((
            px(12.0, 4.0),
            Popover {
                positions: [PopoverSide::Bottom, PopoverSide::Top]
                    .map(|side| PopoverPlacement {
                        side,
                        ..PopoverPlacement::default()
                    })
                    .into(),
                window_margin: 0.0,
            },
            ChildOf(anchor),
        ))
        .id();
    settle(&mut app);

    assert_eq!(rect(&app, popover).rect, Rect::new(20, 6, 12, 4));
}

/// Upstream focuses a modal's last field after a tenth of a second, so the
/// clock is stepped rather than waited on.
#[test]
fn a_modal_dialog_focuses_its_last_field() {
    let mut app = app();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        200,
    )));
    let dialog = app
        .world_mut()
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                ..px(10.0, 4.0)
            },
            ModalDialog,
        ))
        .id();
    let fields = children(&mut app, dialog, 2, || (px(10.0, 1.0), TabIndex(0)));
    settle(&mut app);

    assert_eq!(focus(&app), Some(fields[1]));
}

/// A `FixedNode` lays out against the viewport and leaves its parent's clip.
#[test]
fn a_fixed_node_lays_out_against_the_viewport_unclipped() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(10.0),
            top: Val::Px(4.0),
            overflow: Overflow::clip(),
            ..px(2.0, 2.0)
        })
        .id();
    let fixed = app
        .world_mut()
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(1.0),
                top: Val::Px(1.0),
                ..px(6.0, 2.0)
            },
            FixedNode,
            ChildOf(parent),
        ))
        .id();
    settle(&mut app);

    let rects = rect(&app, fixed);
    assert_eq!(rects.rect, Rect::new(1, 1, 6, 2));
    assert_eq!(rects.visible, rects.rect);
}

/// A lazy menu spawns its popup as it opens, so the frame that spawns it is
/// the frame the menu looks for an item to focus.
#[test]
fn a_menu_opened_as_it_spawns_focuses_its_item() {
    let mut app = app();
    let root = root(&mut app);
    settle(&mut app);

    let popup = app
        .world_mut()
        .spawn((
            Node::default(),
            MenuPopup::default(),
            MenuFocusState::Opening(NavAction::First),
            ChildOf(root),
        ))
        .id();
    let item = app
        .world_mut()
        .spawn((Node::default(), MenuItem, TabIndex(0), ChildOf(popup)))
        .id();
    app.update();

    assert_eq!(focus(&app), Some(item));
    assert_eq!(
        app.world().get::<MenuFocusState>(popup),
        Some(&MenuFocusState::Open),
        "closing is what fires CloseAll"
    );
}

#[derive(Resource, Default)]
struct CloseRequests(usize);

/// Esc only asks the app to close the menu, so the request is what is counted.
#[test]
fn a_menu_closes_from_outside_and_on_escape() {
    let mut app = app();
    app.init_resource::<CloseRequests>();
    let root = root(&mut app);
    let popup = app
        .world_mut()
        .spawn((
            px(10.0, 3.0),
            MenuPopup::default(),
            MenuFocusState::Opening(NavAction::First),
            ChildOf(root),
        ))
        .observe(|on: On<MenuEvent>, mut closes: ResMut<CloseRequests>| {
            if matches!(on.action, MenuAction::CloseAll) {
                closes.0 += 1;
            }
        })
        .id();
    let item = app
        .world_mut()
        .spawn((px(10.0, 1.0), MenuItem, TabIndex(0), ChildOf(popup)))
        .id();
    settle(&mut app);
    assert_eq!(focus(&app), Some(item));
    let closes = |app: &App| app.world().resource::<CloseRequests>().0;

    click(&mut app, 30, 10);
    assert_eq!(focus(&app), None);
    assert_eq!(
        app.world().get::<MenuFocusState>(popup),
        Some(&MenuFocusState::Closed)
    );
    assert_eq!(closes(&app), 1);

    app.world_mut()
        .entity_mut(popup)
        .insert(MenuFocusState::Opening(NavAction::First));
    app.update();
    assert_eq!(focus(&app), Some(item));
    press_key(&mut app, KeyCode::Esc);
    assert_eq!(closes(&app), 2);
}
