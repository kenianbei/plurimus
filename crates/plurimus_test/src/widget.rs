//! Access to what a widget currently draws.

use std::sync::Arc;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use plurimus_core::{TerminalWidget, UiWidget};

/// The drawable an entity's [`UiWidget`] currently holds.
///
/// Comparing two of these by [`Arc::ptr_eq`] answers whether a stylist
/// rebuilt the widget between two frames, which is how a test tells a
/// redraw from a skipped one.
///
/// # Panics
///
/// If `entity` has no [`UiWidget`] - it was never spawned as a widget, or
/// its stylist has not run yet.
#[must_use]
pub fn widget_content(app: &App, entity: Entity) -> Arc<dyn TerminalWidget> {
    app.world()
        .entity(entity)
        .get::<UiWidget>()
        .expect("entity carries a UiWidget")
        .content()
}
