//! The bevy [`Camera`] mirror of a resolved terminal viewport.
//!
//! Every camera but a [`Camera3d`](bevy_camera::Camera3d) is mirrored in
//! [`CellSize`] pixels - the terminal is its whole target - and one carrying a
//! [`Projection`] has it fitted to the view as bevy's render stack would. A
//! `Camera3d` renders to an image and is left to that stack, whose
//! `camera_system` sizes its viewport from the image.

use bevy_camera::{Camera, Projection, RenderTargetInfo, Viewport};
use bevy_ecs::prelude::{DetectChanges, DetectChangesMut, Mut};
use bevy_math::UVec2;
use ratatui_core::layout::Rect;

use crate::size::CellSize;

// Bypassed so an idle frame leaves both change ticks alone; `Mut`'s own
// `DerefMut` would mark them before the comparison ran.
pub(crate) fn sync_camera(
    mut camera: Mut<Camera>,
    projection: Option<Mut<Projection>>,
    rect: Rect,
    target: UVec2,
    cell: CellSize,
) {
    let mut changed = mirror_into_camera(camera.bypass_change_detection(), rect, target, cell);
    if let Some(mut projection) = projection
        && (changed || projection.is_changed())
    {
        changed |= fit_projection(
            camera.bypass_change_detection(),
            projection.bypass_change_detection(),
        );
    }
    if changed {
        camera.set_changed();
    }
}

/// What `bevy_render`'s `camera_system` does once a projected camera is sized,
/// for the cameras it does not own. Returns whether anything was written; a
/// zero-sized view writes nothing, as a perspective projection panics on one.
fn fit_projection(camera: &mut Camera, projection: &mut Projection) -> bool {
    let Some(size) = camera
        .logical_viewport_size()
        .filter(|size| size.x != 0.0 && size.y != 0.0)
    else {
        return false;
    };
    projection.update(size.x, size.y);
    camera.computed.clip_from_view = projection.get_clip_from_view();
    true
}

/// Returns whether anything was written. Neither bevy type is `PartialEq`,
/// so the comparison is by field.
fn mirror_into_camera(camera: &mut Camera, rect: Rect, target: UVec2, cell: CellSize) -> bool {
    let position = cell.pixels(rect.x, rect.y);
    let extent = cell.pixels(rect.width, rect.height);
    let viewport_differs = camera.viewport.as_ref().is_none_or(|viewport| {
        viewport.physical_position != position || viewport.physical_size != extent
    });
    if viewport_differs {
        camera.viewport = Some(Viewport {
            physical_position: position,
            physical_size: extent,
            depth: 0.0..1.0,
        });
    }
    let target_differs = camera
        .computed
        .target_info
        .as_ref()
        .is_none_or(|current| current.physical_size != target);
    if target_differs {
        camera.computed.target_info = Some(RenderTargetInfo {
            physical_size: target,
            scale_factor: 1.0,
        });
    }
    viewport_differs || target_differs
}

#[cfg(test)]
mod tests {
    use bevy_app::{App, Last};
    use bevy_camera::{Camera, Camera2d, Camera3d, OrthographicProjection, Projection};
    use bevy_ecs::prelude::{DetectChanges, Query, Ref, ResMut, Resource};
    use bevy_math::UVec2;
    use ratatui_core::layout::Rect;

    use crate::{CellSize, CorePlugin, ResolvedViewport, TerminalCamera, TerminalSize};

    fn resolved(app: &App, entity: bevy_ecs::entity::Entity) -> Rect {
        app.world().get::<ResolvedViewport>(entity).unwrap().0
    }

    #[test]
    fn a_terminal_camera_is_mirrored_in_cell_pixels_and_a_3d_one_is_left_alone() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(TerminalSize::new(10, 4));
        app.insert_resource(CellSize { x: 8.0, y: 16.0 });
        let windowed = app
            .world_mut()
            .spawn(TerminalCamera::Fixed(Rect::new(2, 1, 4, 2)))
            .id();
        let projected = app
            .world_mut()
            .spawn((
                TerminalCamera::Fixed(Rect::new(2, 1, 4, 2)),
                Camera3d::default(),
            ))
            .id();

        app.update();

        let camera = app.world().get::<Camera>(windowed).unwrap();
        let viewport = camera.viewport.as_ref().unwrap();
        assert_eq!(viewport.physical_position, UVec2::new(16, 16));
        assert_eq!(viewport.physical_size, UVec2::new(32, 32));
        assert_eq!(
            camera.computed.target_info.as_ref().unwrap().physical_size,
            UVec2::new(80, 64)
        );
        let camera = app.world().get::<Camera>(projected).unwrap();
        assert!(camera.viewport.is_none());
        assert!(camera.computed.target_info.is_none());
        assert_eq!(resolved(&app, projected), Rect::new(2, 1, 4, 2));
    }

    fn clip_from_view(app: &App, entity: bevy_ecs::entity::Entity) -> bevy_math::Mat4 {
        app.world().get::<Camera>(entity).unwrap().clip_from_view()
    }

    #[test]
    fn a_2d_camera_is_mirrored_and_its_projection_fitted_to_the_view() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(TerminalSize::new(10, 4));
        app.insert_resource(CellSize { x: 1.0, y: 2.0 });
        let camera = app
            .world_mut()
            .spawn((TerminalCamera::Fixed(Rect::new(2, 1, 4, 2)), Camera2d))
            .id();

        app.update();

        let viewport = app.world().get::<Camera>(camera).unwrap().viewport.clone();
        assert_eq!(viewport.unwrap().physical_size, UVec2::new(4, 4));
        let fitted = clip_from_view(&app, camera);
        assert_ne!(fitted, bevy_math::Mat4::IDENTITY);
        // Orthographic clip x is 2 over the view's width in world units.
        assert!((fitted.x_axis.x - 0.5).abs() < 1e-6);

        let mut projection = app.world_mut().get_mut::<Projection>(camera).unwrap();
        *projection = Projection::Orthographic(OrthographicProjection {
            scale: 2.0,
            ..OrthographicProjection::default_2d()
        });
        app.update();

        assert!((clip_from_view(&app, camera).x_axis.x - 0.25).abs() < 1e-6);
    }

    #[derive(Resource, Default)]
    struct CameraChanged(bool);

    fn record_camera_change(cameras: Query<Ref<Camera>>, mut changed: ResMut<CameraChanged>) {
        changed.0 = cameras.iter().any(|camera| camera.is_changed());
    }

    #[test]
    fn an_idle_frame_leaves_the_mirrored_camera_unchanged() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.init_resource::<CameraChanged>();
        app.add_systems(Last, record_camera_change);
        app.insert_resource(TerminalSize::new(10, 4));
        app.world_mut().spawn(TerminalCamera::Full);
        app.world_mut().spawn((TerminalCamera::Full, Camera2d));

        app.update();
        assert!(app.world().resource::<CameraChanged>().0);
        app.update();
        assert!(!app.world().resource::<CameraChanged>().0);

        app.insert_resource(TerminalSize::new(12, 4));
        app.update();
        assert!(app.world().resource::<CameraChanged>().0);
    }
}
