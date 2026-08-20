//! Taking a widget over with [`StylistDisabled`] and handing it back.
//!
//! Change detection compares against a system's last run, so an entity that
//! sat outside every stylist query missed whatever landed meanwhile. The
//! removal hook resets its cache, making the repaint a contract rather than
//! a caveat.

use std::sync::Arc;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::{Color, Style};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::widget_content;
use plurimus_ui::{StylistDisabled, UiArea, UiTheme};
use plurimus_widgets::{WidgetsPlugin, button};

const AREA: Rect = Rect::new(0, 0, 12, 1);

fn app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(12, 1));
    app.world_mut().spawn(TerminalCamera::default());
    let button = app
        .world_mut()
        .spawn((button("ok"), UiArea::Fixed(AREA)))
        .id();
    app.update();
    (app, button)
}

fn swap_theme(app: &mut App) {
    app.world_mut().resource_mut::<UiTheme>().normal =
        Style::default().fg(Color::Magenta).bg(Color::Black);
}

#[test]
fn an_entity_handed_back_repaints_even_though_it_missed_the_theme_swap() {
    let (mut app, button) = app();
    app.world_mut().entity_mut(button).insert(StylistDisabled);
    app.update();
    let taken_over = widget_content(&app, button);

    swap_theme(&mut app);
    app.update();
    assert!(
        Arc::ptr_eq(&taken_over, &widget_content(&app, button)),
        "the stylist leaves a taken-over widget alone"
    );

    app.world_mut()
        .entity_mut(button)
        .remove::<StylistDisabled>();
    app.update();

    assert!(!Arc::ptr_eq(&taken_over, &widget_content(&app, button)));
}

// The hook fires on despawn too, where there is no cache left to reset.
#[test]
fn despawning_a_taken_over_entity_is_harmless() {
    let (mut app, button) = app();
    app.world_mut().entity_mut(button).insert(StylistDisabled);
    app.update();

    app.world_mut().entity_mut(button).despawn();
    app.update();

    assert!(app.world().get_entity(button).is_err());
}
