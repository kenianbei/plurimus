//! Bridges terminal cameras into bevy_camera so bevy_ui lays out against
//! terminal viewports.
//!
//! One physical pixel is one terminal cell: each camera reports its viewport
//! and target size in cells with a scale factor of 1, so taffy computes
//! directly in cells and nothing downstream converts units. The default
//! terminal camera is mirrored as bevy_ui's `IsDefaultUiCamera`, so a node
//! that names no camera still lands somewhere.

use bevy_camera::{Camera, RenderTargetInfo, Viewport};
use bevy_ecs::prelude::{Commands, Entity, Has, Query, Res};
use bevy_math::UVec2;
use bevy_ui::IsDefaultUiCamera;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{DefaultCamera, ResolvedViewport, TerminalCamera, TerminalSize};

pub(crate) fn sync_bui_cameras(
    size: Res<TerminalSize>,
    default_camera: Res<DefaultCamera>,
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
        if let Some(mut camera) = camera {
            apply(&mut camera, terminal_camera, viewport, *size);
        } else {
            let mut camera = Camera::default();
            apply(&mut camera, terminal_camera, viewport, *size);
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

fn apply(camera: &mut Camera, terminal: &TerminalCamera, viewport: Rect, size: TerminalSize) {
    camera.order = terminal.order;
    camera.is_active = terminal.active;
    camera.viewport = Some(Viewport {
        physical_position: UVec2::new(u32::from(viewport.x), u32::from(viewport.y)),
        physical_size: UVec2::new(u32::from(viewport.width), u32::from(viewport.height)),
        depth: 0.0..1.0,
    });
    camera.computed.target_info = Some(RenderTargetInfo {
        physical_size: UVec2::new(u32::from(size.cols), u32::from(size.rows)),
        scale_factor: 1.0,
    });
}
