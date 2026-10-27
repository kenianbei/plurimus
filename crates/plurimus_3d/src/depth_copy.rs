//! Render-world copy of the view's depth texture into the readback
//! buffer, scheduled in `Core3d` after the main pass.

use bevy_asset::Handle;
use bevy_ecs::prelude::{Component, Res};
use bevy_log::warn_once;
use bevy_render::RenderApp;
use bevy_render::extract_component::ExtractComponent;
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_resource::{TexelCopyBufferInfo, TexelCopyBufferLayout};
use bevy_render::renderer::{RenderContext, ViewQuery};
use bevy_render::storage::{GpuShaderBuffer, ShaderBuffer};
use bevy_render::view::ViewDepthStencilTexture;

use crate::depth::depth_row_stride;

/// The camera's depth-readback buffer, extracted for the copy pass.
#[derive(Component, Clone, ExtractComponent)]
#[extract_app(RenderApp)]
pub(crate) struct DepthCopyTarget(pub(crate) Handle<ShaderBuffer>);

pub(crate) fn copy_depth_to_readback(
    view: ViewQuery<(&ViewDepthStencilTexture, &DepthCopyTarget)>,
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
    mut ctx: RenderContext,
) {
    let (depth, target) = view.into_inner();
    let Some(buffer) = buffers.get(&target.0) else {
        return;
    };
    let texture = depth.texture();
    if texture.sample_count() > 1 {
        warn_once!("depth readback needs Msaa::Off; multisampled depth cannot be copied");
        return;
    }
    let size = texture.size();
    let stride = depth_row_stride(size.width);
    if buffer.buffer.size() < (stride * size.height as usize) as u64 {
        return;
    }
    ctx.command_encoder().copy_texture_to_buffer(
        texture.as_image_copy(),
        TexelCopyBufferInfo {
            buffer: &buffer.buffer,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride as u32),
                rows_per_image: None,
            },
        },
        size,
    );
}
