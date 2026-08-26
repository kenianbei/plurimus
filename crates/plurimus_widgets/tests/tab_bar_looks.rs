//! One snapshot per look and orientation, plus the border that cannot join.

use bevy_app::App;
use bevy_ecs::prelude::ChildOf;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, Edge, TerminalCamera, TerminalSize};
use plurimus_test::composed_frame;
use plurimus_ui::{Checked, UiArea};
use plurimus_widgets::ratatui_widgets::borders::BorderType;
use plurimus_widgets::{TabBarLook, TabBarOrientation, WidgetsPlugin, tab_bar, tab_item};

fn render(cols: u16, rows: u16, look: TabBarLook) -> String {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(cols, rows));
    app.world_mut().spawn(TerminalCamera::default());
    let bar = app
        .world_mut()
        .spawn((tab_bar(), look, UiArea::Fixed(Rect::new(0, 0, cols, rows))))
        .id();
    for (index, label) in ["Diary", "Plan", "Foods"].into_iter().enumerate() {
        let item = app.world_mut().spawn((tab_item(label), ChildOf(bar))).id();
        if index == 1 {
            app.world_mut().entity_mut(item).insert(Checked);
        }
    }
    app.update();
    composed_frame(&app)
}

fn vertical() -> TabBarLook {
    TabBarLook::default().with_orientation(TabBarOrientation::Vertical)
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
    insta::assert_snapshot!(render(9, 3, vertical()));
}

#[test]
fn vertical_divided() {
    insta::assert_snapshot!(render(
        9,
        5,
        vertical().with_divider(Some("─────────".into()))
    ));
}

#[test]
fn vertical_boxed() {
    insta::assert_snapshot!(render(
        9,
        9,
        vertical().with_border(Some(BorderType::Plain))
    ));
}

#[test]
fn vertical_joined_right() {
    let look = vertical()
        .with_border(Some(BorderType::Plain))
        .with_joined(Some(Edge::Right));
    insta::assert_snapshot!(render(9, 11, look));
}

#[test]
fn vertical_joined_left() {
    let look = vertical()
        .with_border(Some(BorderType::Rounded))
        .with_joined(Some(Edge::Left));
    insta::assert_snapshot!(render(9, 11, look));
}
