//! One snapshot per look and orientation, plus the border that cannot join.

use bevy_app::App;
use bevy_ecs::prelude::ChildOf;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, Edge, TerminalCamera, TerminalSize};
use plurimus_test::composed_frame;
use plurimus_ui::UiArea;
use plurimus_widgets::ratatui_widgets::borders::BorderType;
use plurimus_widgets::{
    ControlOrientation, SelectedTab, TabBarLook, TabList, WidgetsPlugin, tab_bar, tab_item,
};

fn render(cols: u16, rows: u16, look: TabBarLook) -> String {
    render_along(ControlOrientation::Horizontal, (cols, rows), look)
}

fn render_vertical(cols: u16, rows: u16, look: TabBarLook) -> String {
    render_along(ControlOrientation::Vertical, (cols, rows), look)
}

// The second tab is selected through the list, which upstream mirrors onto
// the tab in `PostUpdate`, so the frame after shows it.
fn render_along(
    orientation: ControlOrientation,
    (cols, rows): (u16, u16),
    look: TabBarLook,
) -> String {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(cols, rows));
    app.world_mut().spawn(TerminalCamera::default());
    let list = TabList {
        orientation,
        ..TabList::default()
    };
    let bar = app
        .world_mut()
        .spawn((
            tab_bar(),
            list,
            look,
            UiArea::Fixed(Rect::new(0, 0, cols, rows)),
        ))
        .id();
    let items: Vec<_> = ["Diary", "Plan", "Foods"]
        .into_iter()
        .map(|label| app.world_mut().spawn((tab_item(label), ChildOf(bar))).id())
        .collect();
    app.world_mut()
        .entity_mut(bar)
        .insert(SelectedTab(Some(items[1])));
    app.update();
    app.update();
    composed_frame(&app)
}

#[test]
fn plain() {
    insta::assert_snapshot!(render(24, 1, TabBarLook::default()));
}

#[test]
fn divided() {
    insta::assert_snapshot!(render(
        24,
        1,
        TabBarLook::default().with_divider(Some("│".into()))
    ));
}

#[test]
fn padded() {
    insta::assert_snapshot!(render(30, 1, TabBarLook::default().with_padding(2)));
}

#[test]
fn boxed_plain() {
    let look = TabBarLook::default().with_border(Some(BorderType::Plain));
    insta::assert_snapshot!(render(24, 3, look));
}

#[test]
fn boxed_rounded_divided() {
    let look = TabBarLook::default()
        .with_border(Some(BorderType::Rounded))
        .with_divider(Some(" ".into()));
    insta::assert_snapshot!(render(26, 3, look));
}

#[test]
fn joined_bottom() {
    let look = TabBarLook::default()
        .with_border(Some(BorderType::Rounded))
        .with_joined(Some(Edge::Bottom));
    insta::assert_snapshot!(render(28, 3, look));
}

#[test]
fn joined_top() {
    let look = TabBarLook::default()
        .with_border(Some(BorderType::Plain))
        .with_joined(Some(Edge::Top));
    insta::assert_snapshot!(render(28, 3, look));
}

#[test]
fn joined_on_an_edge_along_the_bar_stays_closed() {
    let look = TabBarLook::default()
        .with_border(Some(BorderType::Plain))
        .with_joined(Some(Edge::Left));
    assert_eq!(
        render(28, 3, look),
        render(
            28,
            3,
            TabBarLook::default().with_border(Some(BorderType::Plain))
        )
    );
}

#[test]
fn quadrant_borders_cannot_join() {
    let look = TabBarLook::default()
        .with_border(Some(BorderType::QuadrantOutside))
        .with_joined(Some(Edge::Bottom));
    insta::assert_snapshot!(render(24, 3, look));
}

#[test]
fn vertical_plain() {
    insta::assert_snapshot!(render_vertical(9, 3, TabBarLook::default()));
}

#[test]
fn vertical_divided() {
    insta::assert_snapshot!(render_vertical(
        9,
        5,
        TabBarLook::default().with_divider(Some("─────────".into()))
    ));
}

#[test]
fn vertical_boxed() {
    insta::assert_snapshot!(render_vertical(
        9,
        9,
        TabBarLook::default().with_border(Some(BorderType::Plain))
    ));
}

#[test]
fn vertical_joined_right() {
    let look = TabBarLook::default()
        .with_border(Some(BorderType::Plain))
        .with_joined(Some(Edge::Right));
    insta::assert_snapshot!(render_vertical(9, 11, look));
}

#[test]
fn vertical_joined_left() {
    let look = TabBarLook::default()
        .with_border(Some(BorderType::Rounded))
        .with_joined(Some(Edge::Left));
    insta::assert_snapshot!(render_vertical(9, 11, look));
}
