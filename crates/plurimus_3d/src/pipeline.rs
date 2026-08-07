//! Extraction and rasterization of readback frames into camera buffers.
//!
//! The GPU work happens on `bevy_render`'s own schedule, so what arrives here
//! is a frame of pixels that is already at least one frame old. Extraction
//! copies whatever readback has completed into the terminal sub-app, and
//! rasterization converts it with the camera's strategy; a camera whose
//! readback has not landed yet simply draws nothing that frame.

use bevy_ecs::prelude::{Entity, Local, Query, Res, ResMut, Resource};
use bevy_ecs::system::SystemParam;
use bevy_math::UVec2;
use plurimus_core::raster::DepthGrid;
use plurimus_core::{CameraBuffer, MainWorld, SourceCamera, TerminalSize, camera_buffer_mut};
use ratatui_core::layout::Rect;

use crate::convert::{Canvas, FrameSources, Scratch3d};
use crate::depth::{DepthFrame, DepthOcclusion};
use crate::occlude::{mask_losing_cells, record_losses};
use crate::overlay::{EdgeOverlay, apply_edge_overlay};
use crate::sample::{DepthSource, box_reduce};
use crate::strategy::Strategy3d;
use crate::target::{Aspect3d, ReadbackFrame, Supersample};

/// Per-camera readback frames, refilled every frame into reused buffers.
#[derive(Resource, Default)]
pub(crate) struct ExtractedFrames3d(pub(crate) Vec<ExtractedFrame3d>);

pub(crate) struct ExtractedFrame3d {
    pub(crate) camera: Entity,
    pub(crate) strategy: Strategy3d,
    pub(crate) pixels: Vec<u8>,
    pub(crate) size: UVec2,
    pub(crate) depths: Vec<f32>,
    pub(crate) depth_size: UVec2,
    pub(crate) letterboxed: bool,
    pub(crate) supersample: u32,
    pub(crate) overlay: Option<EdgeOverlay>,
    pub(crate) occlusion: bool,
}

impl Default for ExtractedFrame3d {
    fn default() -> Self {
        Self {
            camera: Entity::PLACEHOLDER,
            strategy: Strategy3d::default(),
            pixels: Vec::new(),
            size: UVec2::ZERO,
            depths: Vec::new(),
            depth_size: UVec2::ZERO,
            letterboxed: false,
            supersample: 1,
            overlay: None,
            occlusion: false,
        }
    }
}

pub(crate) fn extract_3d(mut main_world: ResMut<MainWorld>, mut frames: ResMut<ExtractedFrames3d>) {
    let mut count = 0;
    let mut cameras = main_world.query::<(
        Entity,
        &Strategy3d,
        &ReadbackFrame,
        Option<&DepthFrame>,
        Option<&Aspect3d>,
        Option<&Supersample>,
        Option<&EdgeOverlay>,
        Option<&DepthOcclusion>,
    )>();
    for (camera, strategy, frame, depth, aspect, supersample, overlay, occlusion) in
        cameras.iter(&main_world)
    {
        if frame.pixels.is_empty() {
            continue;
        }
        if frames.0.len() == count {
            frames.0.push(ExtractedFrame3d::default());
        }
        let slot = &mut frames.0[count];
        slot.camera = camera;
        slot.strategy = *strategy;
        slot.size = frame.size;
        slot.letterboxed = aspect.is_some();
        slot.supersample = supersample.map_or(1, |factor| factor.0.get());
        slot.overlay = overlay.copied();
        slot.occlusion = occlusion.is_some();
        slot.pixels.clear();
        slot.pixels.extend_from_slice(&frame.pixels);
        slot.depths.clear();
        slot.depth_size = UVec2::ZERO;
        let wants_depth = slot.occlusion || matches!(strategy, Strategy3d::Depth(_));
        if let Some(depth) = depth.filter(|_| wants_depth) {
            slot.depths.extend_from_slice(&depth.depths);
            slot.depth_size = depth.size;
        }
        count += 1;
    }
    frames.0.truncate(count);
}

/// Resets the shared depth grid to the frame area before rasterization.
/// Frame-wide shared depth at halfblock subcell resolution, reset to the
/// frame area before rasterization; cameras opting into
/// [`DepthOcclusion`](crate::DepthOcclusion) depth-test against it and
/// mask cells that lose.
#[derive(Resource, Debug, Default)]
pub struct FrameDepth(pub DepthGrid);

pub(crate) fn reset_frame_depth(size: Res<TerminalSize>, mut depth: ResMut<FrameDepth>) {
    depth.0.reset(size.rect());
}

#[derive(SystemParam)]
pub(crate) struct Rasterize3d<'w, 's> {
    frames: ResMut<'w, ExtractedFrames3d>,
    cameras: Query<'w, 's, (&'static SourceCamera, &'static mut CameraBuffer)>,
    shared_depth: ResMut<'w, FrameDepth>,
    scratch: Local<'s, Scratch3d>,
    reduced: Local<'s, Vec<u8>>,
    losses: Local<'s, Vec<bool>>,
}

pub(crate) fn rasterize_3d(params: Rasterize3d) {
    let Rasterize3d {
        mut frames,
        mut cameras,
        mut shared_depth,
        mut scratch,
        mut reduced,
        mut losses,
    } = params;
    for frame in &mut frames.0 {
        let Some(mut buffer) = camera_buffer_mut(&mut cameras, frame.camera) else {
            continue;
        };
        let (pixels, size) = if frame.supersample > 1 {
            let size = box_reduce(&frame.pixels, frame.size, frame.supersample, &mut reduced);
            (reduced.as_mut_slice(), size)
        } else {
            (frame.pixels.as_mut_slice(), frame.size)
        };
        let dest = frame_dest(frame.strategy, frame.letterboxed, size, buffer.0.area);
        let depth = (frame.depth_size != UVec2::ZERO).then(|| DepthSource {
            depths: &frame.depths,
            size: frame.depth_size,
        });
        if frame.occlusion
            && let Some(depth) = depth
            && record_losses(depth, dest, &mut shared_depth.0, &mut losses)
        {
            mask_losing_cells(&losses, dest, pixels, size);
        }
        let sources = FrameSources {
            pixels,
            size,
            depth,
        };
        let mut canvas = Canvas::new(&mut buffer.0, dest);
        frame.strategy.convert(&sources, &mut scratch, &mut canvas);
        if let Some(overlay) = &frame.overlay {
            apply_edge_overlay(overlay, &sources, &mut canvas);
        }
    }
}

/// A letterboxed frame draws into the centered rect matching its pixel
/// size; otherwise it stretch-fills the whole viewport.
fn frame_dest(strategy: Strategy3d, letterboxed: bool, size: UVec2, area: Rect) -> Rect {
    if !letterboxed {
        return area;
    }
    let scale = strategy.pixels_per_cell();
    let columns = (size.x / scale.x).min(u32::from(area.width)) as u16;
    let rows = (size.y / scale.y).min(u32::from(area.height)) as u16;
    Rect::new(
        area.x + (area.width - columns) / 2,
        area.y + (area.height - rows) / 2,
        columns,
        rows,
    )
}
