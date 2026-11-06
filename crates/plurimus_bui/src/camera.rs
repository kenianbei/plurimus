//! Marks the default terminal camera as `bevy_ui`'s default, so a node that
//! names no camera still lands somewhere. Everything else `bevy_ui` reads
//! from a camera - viewport, target size - core writes onto the bevy `Camera`
//! itself.

use bevy_ecs::prelude::{Commands, Entity, Has, Query, Res, With};
use bevy_ui::IsDefaultUiCamera;
use plurimus_core::{DefaultCamera, TerminalCamera};

pub(crate) fn sync_default_ui_camera(
    default_camera: Res<DefaultCamera>,
    cameras: Query<(Entity, Has<IsDefaultUiCamera>), With<TerminalCamera>>,
    mut commands: Commands,
) {
    for (entity, is_default) in &cameras {
        let should_be_default = default_camera.0 == Some(entity);
        if should_be_default && !is_default {
            commands.entity(entity).insert(IsDefaultUiCamera);
        } else if !should_be_default && is_default {
            commands.entity(entity).remove::<IsDefaultUiCamera>();
        }
    }
}
