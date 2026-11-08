//! A 3d camera's projection, fitted to the terminal cell's shape.
//!
//! `bevy_render` fits a `Camera3d`'s projection to its target image, whose
//! pixels are square only when [`CellSize`] matches the strategy's pixel
//! density. This refits it after bevy does, to the pixels' physical shape, so a
//! scene is undistorted whatever cell the app names - and writes nothing at
//! the 1:2 cell every shipped density assumes.

use bevy_camera::primitives::Frustum;
use bevy_camera::{Camera, Projection};
use bevy_ecs::prelude::{DetectChangesMut, Query, Res};
use bevy_transform::components::GlobalTransform;
use plurimus_core::CellSize;

use crate::strategy::Strategy3d;

type Fitted = (
    &'static mut Camera,
    &'static mut Projection,
    &'static mut Frustum,
    &'static GlobalTransform,
    &'static Strategy3d,
);

pub(crate) fn fit_cell_projection(cell: Res<CellSize>, mut cameras: Query<Fitted>) {
    for (mut camera, mut projection, mut frustum, transform, strategy) in &mut cameras {
        let Some(size) = camera
            .logical_viewport_size()
            .filter(|size| size.x != 0.0 && size.y != 0.0)
        else {
            continue;
        };
        let density = strategy.pixels_per_cell().as_vec2();
        let pixel_aspect = (cell.x / density.x) / (cell.y / density.y);
        let mut fitted = projection.clone();
        fitted.update(size.x * pixel_aspect, size.y);
        let clip = match &camera.sub_camera_view {
            Some(sub_view) => fitted.get_clip_from_view_for_sub(sub_view),
            None => fitted.get_clip_from_view(),
        };
        if clip == camera.computed.clip_from_view {
            continue;
        }
        // `update_frusta` recomputes only for a changed projection, which the
        // bypass below hides from it.
        *frustum = fitted.compute_frustum(transform);
        // Past change detection, or `camera_system` would see a changed
        // projection and refit it to the image next frame.
        *projection.bypass_change_detection() = fitted;
        camera.bypass_change_detection().computed.clip_from_view = clip;
    }
}

#[cfg(test)]
mod tests {
    use bevy_camera::primitives::Frustum;
    use bevy_camera::{Camera, PerspectiveProjection, Projection, RenderTargetInfo};
    use bevy_ecs::prelude::{DetectChanges, Entity, World};
    use bevy_math::UVec2;
    use bevy_transform::components::GlobalTransform;
    use plurimus_core::CellSize;

    use super::fit_cell_projection;
    use crate::strategy::Strategy3d;

    const TARGET: UVec2 = UVec2::new(80, 40);

    /// A halfblock camera as `bevy_render` leaves it: sized to its target image,
    /// its projection fitted to the image's aspect.
    fn fitted_by_bevy(world: &mut World) -> Entity {
        let mut camera = Camera::default();
        camera.computed.target_info = Some(RenderTargetInfo {
            physical_size: TARGET,
            scale_factor: 1.0,
        });
        let mut projection = Projection::Perspective(PerspectiveProjection::default());
        projection.update(TARGET.x as f32, TARGET.y as f32);
        camera.computed.clip_from_view = projection.get_clip_from_view();
        world
            .spawn((
                camera,
                projection,
                Frustum::default(),
                GlobalTransform::default(),
                Strategy3d::Halfblocks,
            ))
            .id()
    }

    fn aspect_ratio(world: &World, camera: Entity) -> f32 {
        match world.get::<Projection>(camera).unwrap() {
            Projection::Perspective(perspective) => perspective.aspect_ratio,
            _ => unreachable!("the test camera is perspective"),
        }
    }

    #[test]
    fn a_square_cell_widens_the_halfblock_pixel() {
        let mut world = World::new();
        world.insert_resource(CellSize { x: 1.0, y: 1.0 });
        let camera = fitted_by_bevy(&mut world);
        world.clear_trackers();

        world.run_system_cached(fit_cell_projection).unwrap();

        // A halfblock pixel is half a cell tall, so under a square cell it is
        // twice as wide as it is tall, and the 2:1 image shows a 4:1 scene.
        assert!((aspect_ratio(&world, camera) - 4.0).abs() < 1e-6);
        let clip = world.get::<Camera>(camera).unwrap().clip_from_view();
        assert!((clip.x_axis.x * 4.0 - clip.y_axis.y).abs() < 1e-5);
        assert!(
            !world
                .entity(camera)
                .get_ref::<Projection>()
                .unwrap()
                .is_changed(),
            "a changed projection would have bevy refit it to the image"
        );
    }

    #[test]
    fn the_cell_every_density_assumes_writes_nothing() {
        let mut world = World::new();
        world.insert_resource(CellSize { x: 1.0, y: 2.0 });
        let camera = fitted_by_bevy(&mut world);
        world.clear_trackers();

        world.run_system_cached(fit_cell_projection).unwrap();

        assert!((aspect_ratio(&world, camera) - 2.0).abs() < 1e-6);
        let entity = world.entity(camera);
        assert!(!entity.get_ref::<Projection>().unwrap().is_changed());
        assert!(!entity.get_ref::<Camera>().unwrap().is_changed());
    }
}
