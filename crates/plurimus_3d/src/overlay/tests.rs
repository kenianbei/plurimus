//! Coverage for the sobel overlay: field extraction, edge detection, and
//! the cell characters chosen for each edge direction.

use bevy_math::UVec2;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::Color;

use super::{EdgeOverlay, EdgeSource, apply_edge_overlay};
use crate::convert::{Canvas, FrameSources};
use crate::sample::DepthSource;

const GRID: u16 = 5;

fn shaded_pixels(lit: impl Fn(usize, usize) -> bool) -> Vec<u8> {
    let side = usize::from(GRID);
    let mut pixels = Vec::with_capacity(side * side * 4);
    for y in 0..side {
        for x in 0..side {
            let value = if lit(x, y) { 255 } else { 0 };
            pixels.extend_from_slice(&[value, value, value, 255]);
        }
    }
    pixels
}

/// Reversed-z: `near` cells sit close to the camera.
fn stepped_depths(near: impl Fn(usize, usize) -> bool) -> Vec<f32> {
    let side = usize::from(GRID);
    let mut depths = Vec::with_capacity(side * side);
    for y in 0..side {
        for x in 0..side {
            depths.push(if near(x, y) { 0.9 } else { 0.1 });
        }
    }
    depths
}

fn frame<'a>(pixels: &'a [u8], depths: Option<&'a [f32]>) -> FrameSources<'a> {
    FrameSources {
        pixels,
        size: UVec2::new(u32::from(GRID), u32::from(GRID)),
        depth: depths.map(|depths| DepthSource {
            depths,
            size: UVec2::new(u32::from(GRID), u32::from(GRID)),
        }),
    }
}

fn overlaid(sources: &FrameSources<'_>, overlay: &EdgeOverlay) -> Buffer {
    let mut buffer = Buffer::empty(Rect::new(0, 0, GRID, GRID));
    let mut canvas = Canvas::full(&mut buffer);
    apply_edge_overlay(overlay, sources, &mut canvas);
    buffer
}

fn overlaid_from(sources: &FrameSources<'_>, source: EdgeSource) -> Buffer {
    overlaid(
        sources,
        &EdgeOverlay {
            source,
            ..EdgeOverlay::default()
        },
    )
}

fn blank() -> Buffer {
    Buffer::empty(Rect::new(0, 0, GRID, GRID))
}

#[test]
fn vertical_boundaries_draw_vertical_edges_with_the_overlay_color() {
    let pixels = shaded_pixels(|x, _| x >= 3);
    let overlay = EdgeOverlay {
        color: Some(Color::Yellow),
        ..EdgeOverlay::default()
    };

    let buffer = overlaid(&frame(&pixels, None), &overlay);

    let edge = buffer.cell((2, 2)).unwrap();
    assert_eq!(edge.symbol(), "|");
    assert_eq!(edge.fg, Color::Yellow);
    assert_eq!(buffer.cell((0, 2)).unwrap().symbol(), " ");
}

#[test]
fn horizontal_boundaries_draw_horizontal_edges() {
    let pixels = shaded_pixels(|_, y| y >= 3);

    let buffer = overlaid(&frame(&pixels, None), &EdgeOverlay::default());

    assert_eq!(buffer.cell((2, 2)).unwrap().symbol(), "―");
}

#[test]
fn diagonal_boundaries_draw_diagonal_edges() {
    let pixels = shaded_pixels(|x, y| x + y >= 5);

    let buffer = overlaid(&frame(&pixels, None), &EdgeOverlay::default());

    assert_eq!(buffer.cell((2, 2)).unwrap().symbol(), "/");
}

#[test]
fn gradients_below_threshold_leave_the_frame_untouched() {
    let pixels = shaded_pixels(|_, _| true);

    let buffer = overlaid(&frame(&pixels, None), &EdgeOverlay::default());

    assert_eq!(buffer, blank());
}

#[test]
fn depth_steps_draw_edges_that_luminance_cannot_see() {
    let flat = shaded_pixels(|_, _| true);
    let depths = stepped_depths(|x, _| x >= 3);
    let sources = frame(&flat, Some(&depths));

    let depth_edges = overlaid_from(&sources, EdgeSource::Depth);
    let luminance_edges = overlaid(&sources, &EdgeOverlay::default());

    assert_eq!(depth_edges.cell((2, 2)).unwrap().symbol(), "|");
    assert_eq!(luminance_edges, blank(), "uniform shading has no gradient");
}

/// Occlusion masking blackens lost pixels but leaves their depths, so
/// the luminance sobel invents a boundary the depth sobel does not.
#[test]
fn masked_pixels_over_continuous_depth_fool_only_luminance() {
    let masked = shaded_pixels(|x, _| x >= 3);
    let continuous = stepped_depths(|_, _| true);
    let sources = frame(&masked, Some(&continuous));

    let luminance_edges = overlaid(&sources, &EdgeOverlay::default());
    let depth_edges = overlaid_from(&sources, EdgeSource::Depth);

    assert_eq!(luminance_edges.cell((2, 2)).unwrap().symbol(), "|");
    assert_eq!(depth_edges, blank(), "depth stayed continuous");
}

#[test]
fn both_takes_its_direction_from_the_stronger_gradient() {
    let pixels = shaded_pixels(|_, y| y >= 3);
    let depths = stepped_depths(|x, _| x >= 3);
    let sources = frame(&pixels, Some(&depths));

    let depth_wins = overlaid_from(&sources, EdgeSource::Both);
    let luminance_wins = overlaid(
        &sources,
        &EdgeOverlay {
            source: EdgeSource::Both,
            depth_threshold: 1.0,
            ..EdgeOverlay::default()
        },
    );

    assert_eq!(depth_wins.cell((2, 2)).unwrap().symbol(), "|");
    assert_eq!(luminance_wins.cell((2, 2)).unwrap().symbol(), "―");
}

#[test]
fn depth_sources_draw_nothing_without_a_depth_frame() {
    let pixels = shaded_pixels(|x, _| x >= 3);
    let sources = frame(&pixels, None);

    let buffer = overlaid_from(&sources, EdgeSource::Depth);

    assert_eq!(buffer, blank());
}
