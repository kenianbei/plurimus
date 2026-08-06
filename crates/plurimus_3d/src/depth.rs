//! GPU depth readback: per-camera depth images filled by the render-world
//! copy pass and read back to the CPU.

use bevy_asset::{Assets, RenderAssetUsages};
use bevy_camera::{Camera3d, Camera3dDepthTextureUsage};
use bevy_ecs::prelude::{Commands, Component, Entity, On, Query, ResMut, With};
use bevy_math::UVec2;
use bevy_render::gpu_readback::{Readback, ReadbackComplete};
use bevy_render::render_resource::{BufferUsages, TextureUsages};
use bevy_render::renderer::RenderDevice;
use bevy_render::storage::ShaderBuffer;
use bevy_render::sync_world::SyncToRenderWorld;

use crate::depth_copy::DepthCopyTarget;
use crate::target::{TargetSize, unpadded_rows};

pub(crate) const DEPTH_BYTES_PER_PIXEL: usize = 4;

/// Row stride of the depth transfer buffer: the copy pass, buffer
/// sizing, and CPU unpack must all agree on it.
pub(crate) fn depth_row_stride(width: u32) -> usize {
    RenderDevice::align_copy_bytes_per_row(width as usize * DEPTH_BYTES_PER_PIXEL)
}

/// Opts a [`Strategy3d`](crate::Strategy3d) camera into GPU depth
/// readback, filling [`DepthFrame`] each frame.
///
/// The camera must render with `Msaa::Off` - multisampled depth cannot
/// be copied, and the copy pass skips it with a warning. Costs one
/// extra GPU texture, copy, and transfer per frame.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct DepthReadback;

/// Depth-tests the camera's cells against the frame-wide shared depth
/// and skips cells a nearer camera already claimed, so overlapping 3d
/// cameras compose by distance instead of order. Requires
/// [`DepthReadback`]; until depth data arrives the camera draws
/// unoccluded. Cameras without this component ignore the shared buffer
/// entirely. Occlusion reads as expected when the overlapping cameras
/// use transparent backgrounds - an opaque background repaints skipped
/// cells at composite time regardless.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct DepthOcclusion;

/// Latest CPU copy of the camera's depth texture.
///
/// Depths are bevy's reverse-Z: 1.0 at the near plane, 0.0 at the far
/// plane, non-linear in between - higher is closer.
#[derive(Component, Debug, Default)]
pub struct DepthFrame {
    /// Row-major reverse-Z depths.
    pub depths: Vec<f32>,
    /// Pixel dimensions, matching the color render target.
    pub size: UVec2,
}

/// Lifecycle state: current depth-image size and the side entity
/// carrying the depth `Readback` (a camera entity can hold only one
/// `Readback`; the color path owns it).
#[derive(Component)]
pub(crate) struct DepthTarget {
    size: UVec2,
    reader: Entity,
}

/// Points a reader side-entity back at its camera.
#[derive(Component)]
pub(crate) struct DepthReaderOf(Entity);

pub(crate) fn sync_depth_targets(
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut cameras: Query<
        (Entity, &TargetSize, &mut Camera3d, Option<&DepthTarget>),
        With<DepthReadback>,
    >,
    mut commands: Commands,
) {
    for (camera, target, mut camera3d, current) in &mut cameras {
        if let Some(usages) = missing_copy_usages(&camera3d) {
            camera3d.depth_texture_usages = usages;
        }
        if current.is_some_and(|depth| depth.size == target.0) {
            continue;
        }
        let handle = buffers.add(create_depth_buffer(target.0));
        let reader = match current {
            Some(depth) => depth.reader,
            None => commands
                .spawn((DepthReaderOf(camera), SyncToRenderWorld))
                .id(),
        };
        commands
            .entity(reader)
            .insert(Readback::buffer(handle.clone()));
        commands
            .entity(camera)
            .insert((
                DepthTarget {
                    size: target.0,
                    reader,
                },
                DepthCopyTarget(handle),
            ))
            .insert_if_new(DepthFrame::default());
    }
}

/// The main pass's depth texture is `RENDER_ATTACHMENT`-only by
/// default; the copy pass needs it readable. `None` when already set,
/// so the caller can leave `Camera3d` change-detection untouched.
fn missing_copy_usages(camera3d: &Camera3d) -> Option<Camera3dDepthTextureUsage> {
    let usages = TextureUsages::from_bits_truncate(camera3d.depth_texture_usages.0);
    (!usages.contains(TextureUsages::COPY_SRC)).then(|| (usages | TextureUsages::COPY_SRC).into())
}

/// WebGPU forbids `Depth32Float` textures as copy *destinations*, so
/// the transfer target is a buffer sized for the aligned row stride the
/// texture-to-buffer copy writes.
fn create_depth_buffer(size: UVec2) -> ShaderBuffer {
    let stride = depth_row_stride(size.x);
    let mut buffer =
        ShaderBuffer::with_size(stride * size.y as usize, RenderAssetUsages::default());
    buffer.buffer_description.usage = BufferUsages::COPY_DST | BufferUsages::COPY_SRC;
    buffer
}

pub(crate) fn store_depth_readback(
    complete: On<ReadbackComplete>,
    readers: Query<&DepthReaderOf>,
    mut cameras: Query<(&DepthTarget, &mut DepthFrame)>,
) {
    let Ok(reader) = readers.get(complete.entity) else {
        return;
    };
    if let Ok((target, mut frame)) = cameras.get_mut(reader.0) {
        copy_unpadded_f32(&complete.data, target.size, &mut frame);
    }
}

/// Strips wgpu's row-alignment padding and reinterprets the bytes as
/// `f32` depths. Wrong-length data (a stale readback from a just-resized
/// target) is ignored, keeping the previous frame.
fn copy_unpadded_f32(data: &[u8], size: UVec2, frame: &mut DepthFrame) {
    let row_bytes = size.x as usize * DEPTH_BYTES_PER_PIXEL;
    let Some(rows) = unpadded_rows(data, row_bytes, size.y as usize) else {
        return;
    };
    frame.size = size;
    frame.depths.clear();
    frame.depths.reserve(size.x as usize * size.y as usize);
    for row in rows {
        frame
            .depths
            .extend(row.chunks_exact(DEPTH_BYTES_PER_PIXEL).map(|bytes| {
                f32::from_ne_bytes(bytes.try_into().expect("chunks are exactly one pixel"))
            }));
    }
}

#[cfg(test)]
mod tests {
    use bevy_math::UVec2;

    use super::{DEPTH_BYTES_PER_PIXEL, DepthFrame, copy_unpadded_f32};

    fn padded_depth_rows(depths: &[&[f32]], stride: usize) -> Vec<u8> {
        let mut data = Vec::new();
        for row in depths {
            let mut bytes: Vec<u8> = row.iter().flat_map(|d| d.to_ne_bytes()).collect();
            bytes.resize(stride, 0xEE);
            data.extend_from_slice(&bytes);
        }
        data
    }

    #[test]
    fn strips_padding_and_reinterprets_depths() {
        let data = padded_depth_rows(&[&[1.0, 0.5], &[0.25, 0.0]], 256);
        let mut frame = DepthFrame::default();

        copy_unpadded_f32(
            &data[..256 + 2 * DEPTH_BYTES_PER_PIXEL],
            UVec2::new(2, 2),
            &mut frame,
        );

        assert_eq!(frame.depths, [1.0, 0.5, 0.25, 0.0]);
        assert_eq!(frame.size, UVec2::new(2, 2));
    }

    #[test]
    fn stale_short_data_keeps_the_previous_frame() {
        let mut frame = DepthFrame {
            depths: vec![0.5; 4],
            size: UVec2::new(2, 2),
        };

        copy_unpadded_f32(&[0; 16], UVec2::new(80, 2), &mut frame);

        assert_eq!(frame.depths, vec![0.5; 4]);
    }
}
