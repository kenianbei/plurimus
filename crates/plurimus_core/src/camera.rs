//! Terminal cameras: where each view sits in the cell grid, and the buffer
//! it rasterizes into.
//!
//! [`TerminalCamera`] places a bevy [`Camera`] in the terminal; the camera's
//! order, activity and clear colour are the `Camera`'s own fields. Each
//! frame the render world holds one [`ExtractedCamera`] per active camera,
//! carrying a [`CameraBuffer`] sized to its resolved viewport and the
//! [`MainEntity`] it mirrors. Pipelines reach
//! the buffer they should draw into with [`camera_buffer_mut`], and fall back
//! to [`DefaultCamera`] when an entity names no camera of its own.

use bevy_camera::{Camera, ClearColorConfig};
use bevy_ecs::prelude::{Commands, Component, Entity, Query, ResMut, Resource, With};
use bevy_extract::extract_resource::ExtractResource;
use bevy_extract::{Extract, MainEntity, TemporaryEntity};
use ratatui_core::buffer::{Buffer, Cell};
use ratatui_core::layout::Rect;

use crate::TerminalRenderApp;
use crate::raster::linear_cell_color;
use crate::viewport::{Edge, ResolvedViewport};

/// Where a camera's view sits in the terminal, resolved against the current
/// size each frame. Docked strips carve from a shared remaining region in
/// `(order, entity)` sequence; fills take what is left.
///
/// Everything else about the view is the required [`Camera`]'s: `order`
/// composites later on top, `is_active` false skips it entirely, and
/// `clear_color` decides what untouched cells become - `Default` keeps the
/// terminal's own background, `None` shows whatever lower cameras drew, and
/// `Custom` pre-fills the view with that colour. In an app with bevy's render
/// stack installed, a terminal camera that is not also a `Camera3d` logs
/// `bevy_render`'s "no render graph" warning once at spawn and is otherwise
/// ignored by it.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[require(Camera)]
#[non_exhaustive]
pub enum TerminalCamera {
    /// The whole terminal.
    #[default]
    Full,
    /// A fixed cell rect, clamped to the screen.
    Fixed(Rect),
    /// A strip carved off one edge of the remaining region.
    Docked {
        /// Edge the strip is docked to.
        edge: Edge,
        /// Strip thickness in cells, clamped to what remains.
        cells: u16,
    },
    /// Whatever remains after all docked strips.
    Fill,
}

/// Render-world copy of an active [`TerminalCamera`], living for one
/// frame's [`TerminalRender`](crate::TerminalRender).
#[derive(Component, Debug, Clone, Copy)]
#[require(TemporaryEntity<TerminalRenderApp>)]
#[non_exhaustive]
pub struct ExtractedCamera {
    /// Compositing order, from [`Camera::order`].
    pub order: isize,
    /// Resolved terminal region this camera occupies.
    pub viewport: Rect,
    /// Compositing mode, from [`Camera::clear_color`].
    pub clear_color: ClearColorConfig,
}

impl ExtractedCamera {
    /// A render-world camera over `viewport`.
    #[must_use]
    pub const fn new(order: isize, viewport: Rect, clear_color: ClearColorConfig) -> Self {
        Self {
            order,
            viewport,
            clear_color,
        }
    }
}

/// Screen-coordinate cell buffer that pipelines rasterize into.
///
/// The buffer's area is the camera's resolved viewport, so cell positions
/// are terminal positions and the compositor merges buffers in place.
#[derive(Component, Debug)]
pub struct CameraBuffer(pub Buffer);

/// Finds the render-world buffer of the camera mirroring `target`.
pub fn camera_buffer_mut<'a>(
    cameras: &'a mut Query<(MainEntity, &mut CameraBuffer)>,
    target: Entity,
) -> Option<bevy_ecs::prelude::Mut<'a, CameraBuffer>> {
    cameras
        .iter_mut()
        .find(|(source, _)| *source == target)
        .map(|(_, buffer)| buffer)
}

/// Main-world entity of the default camera: the active camera with the
/// lowest `(order, entity)`. Pipelines target it when no explicit camera
/// is given. Kept current in the main world and extracted per frame.
#[derive(Resource, ExtractResource, Debug, Clone, Copy, Default)]
#[extract_app(TerminalRenderApp)]
pub struct DefaultCamera(pub Option<Entity>);

pub(crate) fn update_default_camera(
    cameras: Query<(Entity, &Camera), With<TerminalCamera>>,
    mut default: ResMut<DefaultCamera>,
) {
    let lowest = cameras
        .iter()
        .filter(|(_, camera)| camera.is_active)
        .min_by_key(|(entity, camera)| (camera.order, *entity))
        .map(|(entity, _)| entity);
    if default.0 != lowest {
        default.0 = lowest;
    }
}

pub(crate) fn extract_cameras(
    cameras: Extract<Query<(Entity, &Camera, &ResolvedViewport), With<TerminalCamera>>>,
    mut commands: Commands,
) {
    for (source, camera, resolved) in &cameras {
        if !camera.is_active {
            continue;
        }
        let viewport = resolved.0;
        commands.spawn((
            ExtractedCamera::new(camera.order, viewport, camera.clear_color),
            MainEntity::from(source),
            CameraBuffer(camera_buffer(viewport, camera.clear_color)),
        ));
    }
}

fn camera_buffer(viewport: Rect, clear_color: ClearColorConfig) -> Buffer {
    match clear_color {
        ClearColorConfig::Custom(color) => {
            let mut cell = Cell::EMPTY;
            cell.set_bg(linear_cell_color(color.into()));
            Buffer::filled(viewport, cell)
        }
        ClearColorConfig::Default | ClearColorConfig::None => Buffer::empty(viewport),
    }
}

#[cfg(test)]
mod tests {
    use bevy_app::App;
    use bevy_camera::Camera;
    use bevy_ecs::prelude::{Entity, Query, ResMut, Resource};
    use bevy_extract::MainEntity;
    use ratatui_core::layout::Rect;

    use super::{CameraBuffer, DefaultCamera, ExtractedCamera, TerminalCamera};
    use crate::{
        CorePlugin, TerminalRenderApp, TerminalRenderAppExt, TerminalRenderSystems, TerminalSize,
    };

    // Extracted cameras live for one frame's render, so they are read from
    // inside it.
    #[derive(Resource, Default)]
    struct Seen(Vec<(Entity, Rect, Rect)>);

    fn record(
        cameras: Query<(MainEntity, &ExtractedCamera, &CameraBuffer)>,
        mut seen: ResMut<Seen>,
    ) {
        seen.0 = cameras
            .iter()
            .map(|(source, camera, buffer)| (source, camera.viewport, buffer.0.area))
            .collect();
        seen.0
            .sort_by_key(|(_, viewport, _)| (viewport.x, viewport.y));
    }

    fn recording_app() -> App {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.sub_app_mut(TerminalRenderApp).init_resource::<Seen>();
        app.add_terminal_systems(TerminalRenderSystems::Rasterize, record);
        app
    }

    fn seen(app: &App) -> &[(Entity, Rect, Rect)] {
        &app.sub_app(TerminalRenderApp).world().resource::<Seen>().0
    }

    #[test]
    fn extracts_active_cameras_with_clamped_viewports() {
        let mut app = recording_app();
        app.insert_resource(TerminalSize::new(10, 5));
        app.world_mut().spawn(TerminalCamera::default());
        app.world_mut()
            .spawn(TerminalCamera::Fixed(Rect::new(8, 3, 10, 10)));
        app.world_mut().spawn((
            TerminalCamera::default(),
            Camera {
                is_active: false,
                ..Camera::default()
            },
        ));

        // Two frames: cameras that outlived theirs would be counted twice.
        app.update();
        app.update();

        let extracted: Vec<(Rect, Rect)> = seen(&app)
            .iter()
            .map(|&(_, viewport, buffer)| (viewport, buffer))
            .collect();
        assert_eq!(
            extracted,
            vec![
                (Rect::new(0, 0, 10, 5), Rect::new(0, 0, 10, 5)),
                (Rect::new(8, 3, 2, 2), Rect::new(8, 3, 2, 2)),
            ]
        );
    }

    #[test]
    fn default_camera_is_lowest_order_active() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.world_mut().spawn((
            TerminalCamera::default(),
            Camera {
                order: 1,
                ..Camera::default()
            },
        ));
        let lowest = app.world_mut().spawn(TerminalCamera::default()).id();

        app.update();

        let render_world = app.sub_app(TerminalRenderApp).world();
        assert_eq!(render_world.resource::<DefaultCamera>().0, Some(lowest));
    }

    #[test]
    fn extracted_cameras_link_back_to_their_source_entity() {
        let mut app = recording_app();
        let spawned = app.world_mut().spawn(TerminalCamera::default()).id();

        app.update();

        let sources: Vec<Entity> = seen(&app).iter().map(|&(source, ..)| source).collect();
        assert_eq!(sources, vec![spawned]);
    }
}
