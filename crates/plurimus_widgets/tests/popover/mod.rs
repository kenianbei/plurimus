//! The app and cameras both popover suites place against.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{
    Background, ComputedUiCamera, CorePlugin, TerminalCamera, TerminalSize, Viewport,
};
use plurimus_widgets::WidgetsPlugin;

pub fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 12));
    app
}

pub fn spawn_camera(app: &mut App, viewport: Rect, order: isize) -> Entity {
    app.world_mut()
        .spawn(
            TerminalCamera::default()
                .with_order(order)
                .with_viewport(Viewport::Fixed(viewport))
                .with_background(Background::Transparent),
        )
        .id()
}

pub fn camera_of(app: &App, entity: Entity) -> Option<Entity> {
    app.world().get::<ComputedUiCamera>(entity).unwrap().0
}
