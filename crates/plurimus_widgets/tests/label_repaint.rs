//! An edited [`UiLabel`] repaints the widget drawing it. The cache compares
//! interaction state, which a label edit leaves untouched, so the stylists
//! read the label's own change tick.

use std::sync::Arc;

use bevy_app::App;
use bevy_ecs::bundle::Bundle;
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

fn spawn(app: &mut App, widget: impl Bundle) -> Entity {
    app.world_mut().spawn((widget, UiArea::Fixed(AREA))).id()
}

fn repaints_on_relabel(app: &mut App, entity: Entity) -> bool {
    app.update();
    let before = widget_content(app, entity);

    app.world_mut()
        .entity_mut(entity)
        .insert(UiLabel("after".into()));
    app.update();

    !Arc::ptr_eq(&before, &widget_content(app, entity))
}

#[test]
fn a_button_repaints_when_its_label_changes() {
    let mut app = app();
    let button = spawn(&mut app, button("before"));

    assert!(repaints_on_relabel(&mut app, button));
    assert!(composed_frame(&app).contains("after"));
}

#[test]
fn a_pane_repaints_when_its_title_changes() {
    let mut app = app();
    let pane = spawn(&mut app, pane("before"));

    assert!(repaints_on_relabel(&mut app, pane));
}

#[test]
fn a_checkbox_repaints_when_its_label_changes() {
    let mut app = app();
    let checkbox = spawn(&mut app, checkbox("before"));

    assert!(repaints_on_relabel(&mut app, checkbox));
}

#[test]
fn a_radio_repaints_when_its_label_changes() {
    let mut app = app();
    let radio = spawn(&mut app, radio("before"));

    assert!(repaints_on_relabel(&mut app, radio));
}

// The label tick is an extra reason to repaint, not a reason to repaint
// every frame - an untouched widget still costs a comparison.
#[test]
fn an_untouched_label_still_skips_the_rebuild() {
    let mut app = app();
    let button = spawn(&mut app, button("steady"));
    app.update();
    let before = widget_content(&app, button);

    app.update();

    assert!(Arc::ptr_eq(&before, &widget_content(&app, button)));
}
