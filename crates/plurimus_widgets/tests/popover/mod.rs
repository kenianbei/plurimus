//! The app and cameras both popover suites place against.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use plurimus_core::bevy_camera::{Camera, ClearColorConfig};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{ComputedUiCamera, CorePlugin, TerminalCamera, TerminalSize};
use plurimus_widgets::WidgetsPlugin;

pub fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 12));
    app
}

pub fn spawn_camera(app: &mut App, viewport: Rect, order: isize) -> Entity {
    app.world_mut()
        .spawn((
            TerminalCamera::Fixed(viewport),
            Camera {
                order,
                clear_color: ClearColorConfig::None,
                ..Camera::default()
            },
        ))
        .id()
}

pub fn camera_of(app: &App, entity: Entity) -> Option<Entity> {
    app.world().get::<ComputedUiCamera>(entity).unwrap().0
}
