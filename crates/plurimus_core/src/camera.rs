//! Terminal cameras: what each view covers, and the buffer it rasterizes
//! into.
//!
//! [`TerminalCamera`] is the main-world component - its [`Viewport`],
//! [`Background`] and order decide where a view lands and how it composites.
//! Each frame the render world holds one [`ExtractedCamera`] per active
//! camera, carrying a [`CameraBuffer`] sized to its resolved viewport and a
//! [`SourceCamera`] pointing back at the entity it mirrors. Pipelines reach
//! the buffer they should draw into with [`camera_buffer_mut`], and fall back
//! to [`DefaultCamera`] when an entity names no camera of its own.

use bevy_ecs::prelude::{Commands, Component, Entity, Query, ResMut, Resource, With};
use ratatui_core::buffer::{Buffer, Cell};
use ratatui_core::layout::Rect;
use ratatui_core::style::Color;

use crate::extract::MainWorld;
use crate::viewport::{ResolvedViewport, Viewport};

/// What a camera's untouched cells become when composited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Background {
    /// Untouched cells keep the terminal default.
    #[default]
    TerminalDefault,
    /// Untouched cells show whatever lower cameras drew; drawing a
    /// default space cell is then indistinguishable from not drawing.
    Transparent,
    /// The camera buffer is pre-filled with this color before
    /// rasterization, so untouched cells composite as solid background.
    Clear(Color),
}

/// A view composited into the terminal frame.
#[derive(Component, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct TerminalCamera {
    /// Cameras with a higher order composite later, on top.
    pub order: isize,
    /// Cell-space region of the terminal this camera occupies.
    pub viewport: Viewport,
    /// Inactive cameras are skipped entirely.
    pub active: bool,
    /// What untouched cells become when composited.
    pub background: Background,
}

impl TerminalCamera {
    /// Sets the composite order; higher composites later, on top.
    #[must_use]
    pub const fn with_order(mut self, order: isize) -> Self {
        self.order = order;
        self
    }

    /// Sets the cell-space region of the terminal the camera occupies.
    #[must_use]
    pub const fn with_viewport(mut self, viewport: Viewport) -> Self {
        self.viewport = viewport;
        self
    }

    /// Sets whether the camera renders at all.
    #[must_use]
    pub const fn with_active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Sets what untouched cells become when composited.
    #[must_use]
    pub const fn with_background(mut self, background: Background) -> Self {
        self.background = background;
        self
    }
}

impl Default for TerminalCamera {
    fn default() -> Self {
        Self {
            order: 0,
            viewport: Viewport::Full,
            active: true,
            background: Background::TerminalDefault,
        }
    }
}

/// Render-world copy of an active [`TerminalCamera`].
#[derive(Component, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct ExtractedCamera {
    /// Compositing order, from [`TerminalCamera::order`].
    pub order: isize,
    /// Resolved terminal region this camera occupies.
    pub viewport: Rect,
    /// Compositing mode, from [`TerminalCamera::background`].
    pub background: Background,
}

impl ExtractedCamera {
    /// A render-world camera over `viewport`.
    #[must_use]
    pub const fn new(order: isize, viewport: Rect, background: Background) -> Self {
        Self {
            order,
            viewport,
            background,
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
    cameras: Query<(Entity, &TerminalCamera)>,
    mut default: ResMut<DefaultCamera>,
) {
    let lowest = cameras
        .iter()
        .filter(|(_, camera)| camera.active)
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
    let mut cameras = main_world.query::<(Entity, &TerminalCamera, &ResolvedViewport)>();
    for (source, camera, resolved) in cameras.iter(&main_world) {
        if !camera.active {
            continue;
        }
        let viewport = resolved.0;
        commands.spawn((
            ExtractedCamera {
                order: camera.order,
                viewport,
                background: camera.background,
            },
            SourceCamera(source),
            CameraBuffer(camera_buffer(viewport, camera.background)),
        ));
    }
}

fn camera_buffer(viewport: Rect, background: Background) -> Buffer {
    match background {
        Background::Clear(color) => {
            let mut cell = Cell::EMPTY;
            cell.set_bg(color);
            Buffer::filled(viewport, cell)
        }
        Background::TerminalDefault | Background::Transparent => Buffer::empty(viewport),
    }
}

#[cfg(test)]
mod tests {
    use bevy_app::App;
    use ratatui_core::layout::Rect;

    use super::{CameraBuffer, DefaultCamera, ExtractedCamera, SourceCamera, TerminalCamera};
    use crate::{CorePlugin, TerminalRenderApp, TerminalSize, Viewport};

    #[test]
    fn extracts_active_cameras_with_clamped_viewports() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(TerminalSize::new(10, 5));
        app.world_mut().spawn(TerminalCamera::default());
        app.world_mut().spawn(
            TerminalCamera::default().with_viewport(Viewport::Fixed(Rect::new(8, 3, 10, 10))),
        );
        app.world_mut()
            .spawn(TerminalCamera::default().with_active(false));

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
        app.world_mut()
            .spawn(TerminalCamera::default().with_order(1));
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
