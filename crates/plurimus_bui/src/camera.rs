//! Bridges terminal cameras into `bevy_camera` so `bevy_ui` lays out against
//! terminal viewports.
//!
//! A cell is [`CellSize`] pixels: each camera reports its viewport and target
//! size in those pixels with a scale factor of 1, so taffy lays out in the
//! pixels a window would and the cell conversion happens once, on the way
//! out. The default
//! terminal camera is mirrored as `bevy_ui`'s `IsDefaultUiCamera`, so a node
//! that names no camera still lands somewhere.

use bevy_camera::{Camera, RenderTargetInfo, Viewport};
use bevy_ecs::prelude::{Commands, Entity, Has, Query, Res};
use bevy_math::Vec2;
use bevy_ui::IsDefaultUiCamera;
use plurimus_core::ratatui_core::layout::Rect;

use super::rect::CellSize;
use plurimus_core::{DefaultCamera, ResolvedViewport, TerminalCamera, TerminalSize};

pub(crate) fn sync_bui_cameras(
    size: Res<TerminalSize>,
    default_camera: Res<DefaultCamera>,
    cell: Res<CellSize>,
    mut cameras: Query<(
        Entity,
        &TerminalCamera,
        &ResolvedViewport,
        Option<&mut Camera>,
        Has<IsDefaultUiCamera>,
    )>,
    mut commands: Commands,
) {
    for (entity, terminal_camera, resolved, camera, is_default) in &mut cameras {
        let viewport = resolved.0;
        let pixels = CameraPixels {
            viewport,
            size: *size,
            cell: cell.vec(),
        };
        if let Some(mut camera) = camera {
            apply(&mut camera, terminal_camera, pixels);
        } else {
            let mut camera = Camera::default();
            apply(&mut camera, terminal_camera, pixels);
            commands.entity(entity).insert(camera);
        }
        let should_be_default = default_camera.0 == Some(entity);
        if should_be_default && !is_default {
            commands.entity(entity).insert(IsDefaultUiCamera);
        } else if !should_be_default && is_default {
            commands.entity(entity).remove::<IsDefaultUiCamera>();
        }
    }
}

struct CameraPixels {
    viewport: Rect,
    size: TerminalSize,
    cell: Vec2,
}

fn apply(camera: &mut Camera, terminal: &TerminalCamera, pixels: CameraPixels) {
    let CameraPixels {
        viewport,
        size,
        cell,
    } = pixels;
    let scaled = |x: u16, y: u16| (Vec2::new(f32::from(x), f32::from(y)) * cell).as_uvec2();
    camera.order = terminal.order;
    camera.is_active = terminal.active;
    camera.viewport = Some(Viewport {
        physical_position: scaled(viewport.x, viewport.y),
        physical_size: scaled(viewport.width, viewport.height),
        depth: 0.0..1.0,
    });
    camera.computed.target_info = Some(RenderTargetInfo {
        physical_size: scaled(size.cols, size.rows),
        scale_factor: 1.0,
    });
}
