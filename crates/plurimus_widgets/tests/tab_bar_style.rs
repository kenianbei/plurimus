//! How a tab bar's items are styled: the active one with and without the
//! bar's focus, overrides and exemptions, and nothing on an idle frame.

use std::sync::Arc;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::ChildOf;
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::{Color, Style};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::{composed_styled_frame, widget_content};
use plurimus_ui::{Checked, InteractionDisabled, StylistDisabled, UiArea, UiStyle};
use plurimus_widgets::ratatui_widgets::borders::BorderType;
use plurimus_widgets::{TabBarActiveStyle, TabBarLook, WidgetsPlugin, tab_bar, tab_item};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 1));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

struct Bar {
    bar: Entity,
    items: Vec<Entity>,
}

fn spawn_bar(app: &mut App) -> Bar {
    let bar = app
        .world_mut()
        .spawn((tab_bar(), UiArea::Fixed(Rect::new(0, 0, 20, 1))))
        .id();
    let items: Vec<Entity> = ["Diary", "Plan"]
        .iter()
        .map(|label| app.world_mut().spawn((tab_item(*label), ChildOf(bar))).id())
        .collect();
    app.world_mut().entity_mut(items[1]).insert(Checked);
    Bar { bar, items }
}

fn focus(app: &mut App, entity: Entity) {
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(entity, FocusCause::Navigated);
}

#[test]
fn active_item_reads_without_focus_and_gains_focus_with_the_bar() {
    let mut app = app();
    let bar = spawn_bar(&mut app);
    app.update();
    insta::assert_snapshot!("tab_bar_active_unfocused", composed_styled_frame(&app));

    focus(&mut app, bar.bar);
    app.update();
    insta::assert_snapshot!("tab_bar_active_focused", composed_styled_frame(&app));
}

#[test]
fn active_style_is_the_bars_and_sits_under_the_items_own() {
    let mut app = app();
    let bar = spawn_bar(&mut app);
    app.world_mut()
        .entity_mut(bar.bar)
        .insert(TabBarActiveStyle(Style::new().bg(Color::Blue)));
    app.world_mut()
        .entity_mut(bar.items[1])
        .insert(UiStyle(Style::new().fg(Color::Red)));
    app.update();

    insta::assert_snapshot!("tab_bar_active_overridden", composed_styled_frame(&app));
}

#[test]
fn a_disabled_bar_draws_every_item_disabled() {
    let mut app = app();
    let bar = spawn_bar(&mut app);
    app.world_mut()
        .entity_mut(bar.bar)
        .insert(InteractionDisabled);
    app.update();

    insta::assert_snapshot!("tab_bar_disabled", composed_styled_frame(&app));
}

#[test]
fn an_exempted_item_keeps_what_it_holds() {
    let mut app = app();
    let bar = spawn_bar(&mut app);
    app.world_mut()
        .entity_mut(bar.items[0])
        .insert(StylistDisabled);
    app.update();

    assert_eq!(
        composed_styled_frame(&app).lines().next(),
        Some("        Plan        ")
    );
}

#[test]
fn an_idle_frame_redraws_nothing() {
    let mut app = app();
    let bar = spawn_bar(&mut app);
    app.update();
    app.update();
    let before = (
        widget_content(&app, bar.bar),
        widget_content(&app, bar.items[1]),
    );

    app.update();

    assert!(Arc::ptr_eq(&before.0, &widget_content(&app, bar.bar)));
    assert!(Arc::ptr_eq(&before.1, &widget_content(&app, bar.items[1])));
}

#[test]
fn moving_the_active_item_repaints_both_items() {
    let mut app = app();
    let bar = spawn_bar(&mut app);
    app.update();
    let before = (
        widget_content(&app, bar.items[0]),
        widget_content(&app, bar.items[1]),
    );

    app.world_mut().entity_mut(bar.items[1]).remove::<Checked>();
    app.world_mut().entity_mut(bar.items[0]).insert(Checked);
    app.update();

    assert!(!Arc::ptr_eq(&before.0, &widget_content(&app, bar.items[0])));
    assert!(!Arc::ptr_eq(&before.1, &widget_content(&app, bar.items[1])));
}

#[test]
fn a_boxed_active_item_styles_its_frame_with_its_label() {
    let mut app = app();
    app.insert_resource(TerminalSize::new(20, 3));
    let bar = spawn_bar(&mut app);
    app.world_mut().entity_mut(bar.bar).insert((
        TabBarLook::default().with_border(Some(BorderType::Plain)),
        UiArea::Fixed(Rect::new(0, 0, 20, 3)),
    ));
    app.update();

    insta::assert_snapshot!("tab_bar_boxed_active", composed_styled_frame(&app));
}
