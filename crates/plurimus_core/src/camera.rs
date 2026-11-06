//! Terminal cameras: where each view sits in the cell grid, and the buffer
//! it rasterizes into.
//!
//! [`TerminalCamera`] places a bevy [`Camera`] in the terminal; the camera's
//! order, activity and clear colour are the `Camera`'s own fields. Each
//! frame the render world holds one [`ExtractedCamera`] per active camera,
//! carrying a [`CameraBuffer`] sized to its resolved viewport and a
//! [`SourceCamera`] pointing back at the entity it mirrors. Pipelines reach
//! the buffer they should draw into with [`camera_buffer_mut`], and fall back
//! to [`DefaultCamera`] when an entity names no camera of its own.

use bevy_camera::{Camera, ClearColorConfig};
use bevy_ecs::prelude::{Commands, Component, Entity, Query, ResMut, Resource, With};
use ratatui_core::buffer::{Buffer, Cell};
use ratatui_core::layout::Rect;

use crate::extract::MainWorld;
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

/// Render-world copy of an active [`TerminalCamera`].
#[derive(Component, Debug, Clone, Copy)]
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

/// The main-world camera entity an extracted camera mirrors.
#[derive(Component, Debug, Clone, Copy)]
pub struct SourceCamera(pub Entity);

/// Finds the render-world buffer of the camera mirroring `target`.
pub fn camera_buffer_mut<'a>(
    cameras: &'a mut Query<(&SourceCamera, &mut CameraBuffer)>,
    target: Entity,
) -> Option<bevy_ecs::prelude::Mut<'a, CameraBuffer>> {
    cameras
        .iter_mut()
        .find(|(source, _)| source.0 == target)
        .map(|(_, buffer)| buffer)
}

/// Main-world entity of the default camera: the active camera with the
/// lowest `(order, entity)`. Pipelines target it when no explicit camera
/// is given. Kept current in the main world and extracted per frame.
#[derive(Resource, Debug, Clone, Copy, Default)]
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
    mut main_world: ResMut<MainWorld>,
    existing: Query<Entity, With<ExtractedCamera>>,
    mut commands: Commands,
) {
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    commands.insert_resource(*main_world.resource::<DefaultCamera>());
    let mut cameras =
        main_world.query_filtered::<(Entity, &Camera, &ResolvedViewport), With<TerminalCamera>>();
    for (source, camera, resolved) in cameras.iter(&main_world) {
        if !camera.is_active {
            continue;
        }
        let viewport = resolved.0;
        commands.spawn((
            ExtractedCamera::new(camera.order, viewport, camera.clear_color),
            SourceCamera(source),
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
    use ratatui_core::layout::Rect;

    use super::{CameraBuffer, DefaultCamera, ExtractedCamera, SourceCamera, TerminalCamera};
    use crate::{CorePlugin, TerminalRenderApp, TerminalSize};

    #[test]
    fn extracts_active_cameras_with_clamped_viewports() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
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

        app.update();

        let render_world = app.sub_app_mut(TerminalRenderApp).world_mut();
        let mut query = render_world.query::<(&ExtractedCamera, &CameraBuffer)>();
        let mut extracted: Vec<(Rect, Rect)> = query
            .iter(render_world)
            .map(|(camera, buffer)| (camera.viewport, buffer.0.area))
            .collect();
        extracted.sort_by_key(|(viewport, _)| (viewport.x, viewport.y));

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
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        let spawned = app.world_mut().spawn(TerminalCamera::default()).id();

        app.update();

        let render_world = app.sub_app_mut(TerminalRenderApp).world_mut();
        let mut query = render_world.query::<&SourceCamera>();
        let sources: Vec<_> = query.iter(render_world).map(|source| source.0).collect();
        assert_eq!(sources, vec![spawned]);
    }
}
