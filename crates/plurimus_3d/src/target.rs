//! Render-target lifecycle and CPU readback storage.
//!
//! A 3d camera renders into an image sized to its terminal viewport, so a
//! resize means allocating a new target and discarding the old one. The
//! readback path is asynchronous: the GPU copy is requested during the render
//! schedule and the pixels arrive later, which is why the CPU side is storage
//! that may or may not hold a finished frame at any moment.

use core::num::NonZeroU32;

use bevy_asset::Assets;
use bevy_camera::{ImageRenderTarget, RenderTarget};
use bevy_ecs::prelude::{Commands, Component, Entity, On, Query, ResMut};
use bevy_image::Image;
use bevy_math::UVec2;
use bevy_render::gpu_readback::{Readback, ReadbackComplete};
use bevy_render::render_resource::{TextureFormat, TextureUsages};
use bevy_render::renderer::RenderDevice;
use plurimus_core::ResolvedViewport;

use crate::strategy::Strategy3d;

const BYTES_PER_PIXEL: usize = 4;

/// Latest CPU copy of a camera's rendered pixels, tightly packed RGBA8.
#[derive(Component, Debug, Default)]
#[non_exhaustive]
pub struct ReadbackFrame {
    /// Row-major RGBA8 bytes with padding stripped.
    pub pixels: Vec<u8>,
    /// Pixel dimensions: viewport cells times the strategy's per-cell
    /// pixel density.
    pub size: UVec2,
}

/// Height of a terminal cell in cell widths; render-target pixels are
/// physically square at the supported per-cell densities.
const CELL_ASPECT: f32 = 2.0;

/// Letterboxes the camera to a fixed scene aspect ratio.
///
/// The ratio is width over height in square pixels - the scene aspect
/// as a 3d author thinks of it (e.g. `4.0 / 3.0`); the 1:2 terminal
/// cell shape is handled internally. Absent, the render stretch-fills
/// the viewport. The letterbox bars are cells the conversion never
/// writes, so they show the camera background.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Aspect3d {
    /// Width over height of the rendered scene.
    pub ratio: f32,
}

/// Renders at `factor`× the cell pixel density and box-filters back
/// down in linear space - opt-in antialiasing. Absent means factor 1
/// and the reduce step is skipped. Cost grows with the square of the
/// factor.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Supersample(pub NonZeroU32);

/// Current pixel size of the camera's render-target image.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct TargetSize(pub(crate) UVec2);

pub(crate) fn sync_render_targets(
    mut images: ResMut<Assets<Image>>,
    cameras: Query<(
        Entity,
        &ResolvedViewport,
        &Strategy3d,
        Option<&Aspect3d>,
        Option<&Supersample>,
        Option<&TargetSize>,
    )>,
    mut commands: Commands,
) {
    for (entity, resolved, strategy, aspect, supersample, current) in &cameras {
        let viewport = resolved.0;
        let (columns, rows) = match aspect {
            Some(aspect) => aspect_fit_cells((viewport.width, viewport.height), aspect.ratio),
            None => (viewport.width, viewport.height),
        };
        let scale = strategy.pixels_per_cell() * supersample.map_or(1, |factor| factor.0.get());
        let desired = UVec2::new(u32::from(columns) * scale.x, u32::from(rows) * scale.y);
        if desired.x == 0 || desired.y == 0 || current.is_some_and(|current| current.0 == desired) {
            continue;
        }
        let handle = images.add(create_target_image(desired));
        commands
            .entity(entity)
            .insert((
                RenderTarget::Image(ImageRenderTarget {
                    handle: handle.clone(),
                    scale_factor: 1.0,
                }),
                Readback::texture(handle),
                TargetSize(desired),
            ))
            .insert_if_new(ReadbackFrame::default());
    }
}

/// Largest cell rect of physical aspect `ratio` inside `viewport`,
/// with the 1:2 cell shape factored in.
fn aspect_fit_cells(viewport: (u16, u16), ratio: f32) -> (u16, u16) {
    let (columns, rows) = viewport;
    if ratio <= 0.0 || columns == 0 || rows == 0 {
        return (columns, rows);
    }
    let full_ratio = f32::from(columns) / (f32::from(rows) * CELL_ASPECT);
    if full_ratio > ratio {
        let fit = (f32::from(rows) * CELL_ASPECT * ratio).round().max(1.0) as u16;
        (fit.min(columns), rows)
    } else {
        let fit = (f32::from(columns) / (ratio * CELL_ASPECT))
            .round()
            .max(1.0) as u16;
        (columns, fit.min(rows))
    }
}

fn create_target_image(size: UVec2) -> Image {
    let mut image = Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    image
}

pub(crate) fn store_readback(
    complete: On<ReadbackComplete>,
    mut cameras: Query<(&TargetSize, &mut ReadbackFrame)>,
) {
    if let Ok((size, mut frame)) = cameras.get_mut(complete.entity) {
        copy_unpadded(&complete.data, size.0, &mut frame);
    }
}

/// Copies readback bytes into `frame`, stripping wgpu's 256-byte row
/// alignment padding. A wrong-length readback (from a just-resized
/// target) is ignored, keeping the previous frame.
fn copy_unpadded(bytes: &[u8], size: UVec2, frame: &mut ReadbackFrame) {
    let row_bytes = size.x as usize * BYTES_PER_PIXEL;
    let Some(rows) = unpadded_rows(bytes, row_bytes, size.y as usize) else {
        return;
    };
    frame.size = size;
    frame.pixels.clear();
    frame.pixels.reserve(row_bytes * size.y as usize);
    for row in rows {
        frame.pixels.extend_from_slice(row);
    }
}

/// Iterates the payload rows of a readback whose rows were written at
/// wgpu's 256-byte copy alignment; `None` for wrong-length (stale)
/// input, or zero rows.
pub(crate) fn unpadded_rows(
    bytes: &[u8],
    row_bytes: usize,
    rows: usize,
) -> Option<impl Iterator<Item = &[u8]>> {
    let stride = RenderDevice::align_copy_bytes_per_row(row_bytes);
    let expected = rows.checked_sub(1)? * stride + row_bytes;
    if bytes.len() < expected {
        return None;
    }
    Some((0..rows).map(move |row| &bytes[row * stride..row * stride + row_bytes]))
}

#[cfg(test)]
mod tests {
    use bevy_math::UVec2;

    use super::{ReadbackFrame, aspect_fit_cells, copy_unpadded};

    #[test]
    fn aspect_fit_pillarboxes_wide_viewports() {
        assert_eq!(aspect_fit_cells((80, 20), 1.0), (40, 20));
    }

    #[test]
    fn aspect_fit_letterboxes_tall_viewports() {
        assert_eq!(aspect_fit_cells((40, 40), 2.0), (40, 10));
    }

    #[test]
    fn aspect_fit_passes_matching_and_degenerate_ratios_through() {
        assert_eq!(aspect_fit_cells((40, 20), 1.0), (40, 20));
        assert_eq!(aspect_fit_cells((80, 20), 0.0), (80, 20));
    }

    const PADDING: u8 = 0xEE;

    fn padded_rows(width: usize, rows: usize, stride: usize) -> Vec<u8> {
        let mut padded = vec![PADDING; stride * rows];
        for row in 0..rows {
            let marker = row as u8 + 1;
            padded[row * stride..row * stride + width * 4].fill(marker);
        }
        padded
    }

    #[test]
    fn strips_row_padding_from_unaligned_widths() {
        let bytes = padded_rows(80, 2, 512);
        let mut frame = ReadbackFrame::default();

        copy_unpadded(&bytes, UVec2::new(80, 2), &mut frame);

        assert_eq!(frame.pixels.len(), 80 * 4 * 2);
        assert!(frame.pixels[..80 * 4].iter().all(|byte| *byte == 1));
        assert!(frame.pixels[80 * 4..].iter().all(|byte| *byte == 2));
    }

    #[test]
    fn aligned_widths_copy_without_padding() {
        let bytes = padded_rows(64, 3, 256);
        let mut frame = ReadbackFrame::default();

        copy_unpadded(&bytes, UVec2::new(64, 3), &mut frame);

        assert_eq!(frame.pixels.len(), 64 * 4 * 3);
        assert_eq!(frame.pixels[64 * 4 * 2], 3);
        assert!(!frame.pixels.contains(&PADDING));
    }

    #[test]
    fn stale_short_data_keeps_the_previous_frame() {
        let mut frame = ReadbackFrame {
            pixels: vec![7; 16],
            size: UVec2::new(2, 2),
        };

        copy_unpadded(&[0; 64], UVec2::new(80, 2), &mut frame);

        assert_eq!(frame.pixels, vec![7; 16]);
        assert_eq!(frame.size, UVec2::new(2, 2));
    }

    #[test]
    fn single_row_needs_no_stride_beyond_its_pixels() {
        let bytes = padded_rows(3, 1, 256);
        let mut frame = ReadbackFrame::default();

        copy_unpadded(&bytes[..12], UVec2::new(3, 1), &mut frame);

        assert_eq!(frame.pixels.len(), 12);
    }
}
