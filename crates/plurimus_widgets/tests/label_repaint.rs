//! An edited [`UiLabel`] repaints the widget drawing it. The cache compares
//! interaction state, which a label edit leaves untouched, so the stylists
//! read the label's own change tick.

use std::sync::Arc;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::{composed_frame, widget_content};
use plurimus_ui::{UiArea, UiLabel};
use plurimus_widgets::{WidgetsPlugin, button, checkbox, pane, radio};

const AREA: Rect = Rect::new(0, 0, 12, 1);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(12, 1));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn relabel(app: &mut App, entity: Entity, text: &'static str) {
    app.world_mut()
        .entity_mut(entity)
        .insert(UiLabel(text.into()));
}

// Whether editing the label rebuilt the widget, for a widget spawned by
// `bundle`.
fn repaints_on_relabel(entity: Entity, app: &mut App) -> bool {
    app.update();
    let before = widget_content(app, entity);

    relabel(app, entity, "after");
    app.update();

    !Arc::ptr_eq(&before, &widget_content(app, entity))
}

#[test]
fn a_button_repaints_when_its_label_changes() {
    let mut app = app();
    let button = app
        .world_mut()
        .spawn((button("before"), UiArea::Fixed(AREA)))
        .id();

    assert!(repaints_on_relabel(button, &mut app));
    assert!(composed_frame(&app).contains("after"));
}

#[test]
fn a_pane_repaints_when_its_title_changes() {
    let mut app = app();
    let pane = app
        .world_mut()
        .spawn((pane("before"), UiArea::Fixed(Rect::new(0, 0, 12, 1))))
        .id();

    assert!(repaints_on_relabel(pane, &mut app));
}

#[test]
fn a_checkbox_repaints_when_its_label_changes() {
    let mut app = app();
    let checkbox = app
        .world_mut()
        .spawn((checkbox("before"), UiArea::Fixed(AREA)))
        .id();

    assert!(repaints_on_relabel(checkbox, &mut app));
}

#[test]
fn a_radio_repaints_when_its_label_changes() {
    let mut app = app();
    let radio = app
        .world_mut()
        .spawn((radio("before"), UiArea::Fixed(AREA)))
        .id();

    assert!(repaints_on_relabel(radio, &mut app));
}

// The label tick is an extra reason to repaint, not a reason to repaint
// every frame - an untouched widget still costs a comparison.
#[test]
fn an_untouched_label_still_skips_the_rebuild() {
    let mut app = app();
    let button = app
        .world_mut()
        .spawn((button("steady"), UiArea::Fixed(AREA)))
        .id();
    app.update();
    let before = widget_content(&app, button);

    app.update();

    assert!(Arc::ptr_eq(&before, &widget_content(&app, button)));
}
