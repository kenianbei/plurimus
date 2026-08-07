//! Cell-space approximations of bevy_ui decoration: rounded corner glyphs,
//! gradient sampling, and blur-free box shadows.
//!
//! bevy_ui describes decoration in continuous pixels, but a cell is the
//! smallest thing that can be drawn, so each feature gets the closest honest
//! equivalent rather than a scaled-down copy. A corner radius becomes at most
//! one rounded glyph, a gradient is sampled once per cell along its line, and
//! a shadow dims toward its color with no blur at all - blur survives only as
//! extra rect for the shadow to occupy.

use std::f32::consts::TAU;

use bevy_color::{Alpha, LinearRgba, Mix, Srgba};
use bevy_math::Vec2;
use bevy_ui::{BoxShadow, ColorStop, Gradient, ResolvedBorderRadius, ShadowStyle};
use plurimus_core::raster::linear_cell_color;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::Color as CellColor;

// One glyph per corner is the honest cell resolution; half a cell of
// radius is where a rounded glyph reads better than a square one.
const ROUND_RADIUS_MIN: f32 = 0.5;
// Shadows have no blur in cells: a fixed dim toward the shadow color.
const SHADOW_BLEND: f32 = 0.5;
const CELL_SCALE: f32 = 1.0;
// Hints at exactly 0 or 1 would degenerate the midpoint exponent.
const HINT_MIN: f32 = 1e-3;

#[derive(Debug, Clone, Copy, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "a rectangle has four corners and each rounds independently"
)]
pub(crate) struct RoundedCorners {
    pub(crate) top_left: bool,
    pub(crate) top_right: bool,
    pub(crate) bottom_right: bool,
    pub(crate) bottom_left: bool,
}

#[derive(Default)]
pub(crate) struct Decoration {
    pub(crate) corners: RoundedCorners,
    pub(crate) background_gradients: Vec<ResolvedGradient>,
    pub(crate) border_gradients: Vec<ResolvedGradient>,
    pub(crate) shadows: Vec<ResolvedShadow>,
}

pub(crate) fn rounded_corners(radius: ResolvedBorderRadius) -> RoundedCorners {
    RoundedCorners {
        top_left: radius.top_left >= ROUND_RADIUS_MIN,
        top_right: radius.top_right >= ROUND_RADIUS_MIN,
        bottom_right: radius.bottom_right >= ROUND_RADIUS_MIN,
        bottom_left: radius.bottom_left >= ROUND_RADIUS_MIN,
    }
}

pub(crate) struct ResolvedGradient {
    kind: GradientKind,
    stops: Vec<GradientStop>,
    length: f32,
}

struct GradientStop {
    position: f32,
    color: LinearRgba,
    exponent: f32,
}

enum GradientKind {
    Linear { start: Vec2, direction: Vec2 },
    Radial { center: Vec2, extents: Vec2 },
    Conic { center: Vec2, start: f32 },
}

pub(crate) fn resolve_gradients(
    gradients: &[Gradient],
    size: Vec2,
    target: Vec2,
) -> Vec<ResolvedGradient> {
    gradients
        .iter()
        .map(|gradient| match gradient {
            Gradient::Linear(linear) => resolve_linear(linear, size, target),
            Gradient::Radial(radial) => resolve_radial(radial, size, target),
            Gradient::Conic(conic) => resolve_conic(conic, size, target),
        })
        .collect()
}

fn resolve_linear(linear: &bevy_ui::LinearGradient, size: Vec2, target: Vec2) -> ResolvedGradient {
    let direction = Vec2::new(linear.angle.sin(), -linear.angle.cos());
    let length = (size.x * direction.x).abs() + (size.y * direction.y).abs();
    let start = size / 2.0 - direction * (length / 2.0);
    ResolvedGradient {
        kind: GradientKind::Linear { start, direction },
        stops: resolve_stops(&linear.stops, length, target),
        length,
    }
}

fn resolve_radial(radial: &bevy_ui::RadialGradient, size: Vec2, target: Vec2) -> ResolvedGradient {
    let offset = radial.position.resolve(CELL_SCALE, size, target);
    let extents = radial
        .shape
        .resolve(offset, CELL_SCALE, size, target)
        .max(Vec2::splat(f32::EPSILON));
    let length = extents.x;
    ResolvedGradient {
        kind: GradientKind::Radial {
            center: size / 2.0 + offset,
            extents,
        },
        stops: resolve_stops(&radial.stops, length, target),
        length,
    }
}

fn resolve_conic(conic: &bevy_ui::ConicGradient, size: Vec2, target: Vec2) -> ResolvedGradient {
    let offset = conic.position.resolve(CELL_SCALE, size, target);
    let points = conic
        .stops
        .iter()
        .map(|stop| stop.angle.map(|angle| angle.clamp(0.0, TAU)))
        .collect();
    let styled = conic.stops.iter().map(|stop| (stop.color, stop.hint));
    ResolvedGradient {
        kind: GradientKind::Conic {
            center: size / 2.0 + offset,
            start: conic.start,
        },
        stops: finalize_stops(points, TAU, styled),
        length: TAU,
    }
}

fn resolve_stops(stops: &[ColorStop], length: f32, target: Vec2) -> Vec<GradientStop> {
    let points = stops
        .iter()
        .map(|stop| stop.point.resolve(CELL_SCALE, length, target).ok())
        .collect();
    let styled = stops.iter().map(|stop| (stop.color, stop.hint));
    finalize_stops(points, length, styled)
}

// First auto stop pins to 0, last to the line length, runs in between
// spread evenly; explicit positions never move backwards.
fn finalize_stops(
    mut points: Vec<Option<f32>>,
    length: f32,
    styled: impl Iterator<Item = (bevy_color::Color, f32)>,
) -> Vec<GradientStop> {
    if let Some(first) = points.first_mut() {
        first.get_or_insert(0.0);
    }
    if let Some(last) = points.last_mut() {
        last.get_or_insert(length);
    }
    fill_auto_points(&mut points);
    let mut previous = 0.0f32;
    points
        .iter()
        .zip(styled)
        .map(|(point, (color, hint))| {
            previous = point.unwrap_or(previous).max(previous);
            GradientStop {
                position: previous,
                color: color.to_linear(),
                exponent: midpoint_exponent(hint),
            }
        })
        .collect()
}

// CSS gradient midpoint: the hint moves where the blend reaches 50%.
// The default 0.5 hint resolves to exactly 1.0, the identity exponent.
fn midpoint_exponent(hint: f32) -> f32 {
    let hint = hint.clamp(HINT_MIN, 1.0 - HINT_MIN);
    0.5f32.ln() / hint.ln()
}

fn fill_auto_points(points: &mut [Option<f32>]) {
    let mut anchor = 0usize;
    for index in 1..points.len() {
        let Some(end) = points[index] else {
            continue;
        };
        let start = points[anchor].unwrap_or(0.0);
        let gap = index - anchor;
        for (offset, point) in points[anchor + 1..index].iter_mut().enumerate() {
            *point = Some(start + (end - start) * (offset + 1) as f32 / gap as f32);
        }
        anchor = index;
    }
}

impl ResolvedGradient {
    pub(crate) fn sample(&self, point: Vec2) -> LinearRgba {
        let along = match self.kind {
            GradientKind::Linear { start, direction } => (point - start).dot(direction),
            GradientKind::Radial { center, extents } => {
                ((point - center) / extents).length() * self.length
            }
            GradientKind::Conic { center, start } => {
                let delta = point - center;
                // Zero points up and the angle grows clockwise, matching
                // bevy_ui's convention in UI (y-down) coordinates.
                (delta.x.atan2(-delta.y) - start).rem_euclid(TAU)
            }
        };
        color_at(&self.stops, along.clamp(0.0, self.length))
    }
}

fn color_at(stops: &[GradientStop], along: f32) -> LinearRgba {
    let Some(first) = stops.first() else {
        return LinearRgba::NONE;
    };
    if along <= first.position {
        return first.color;
    }
    for pair in stops.windows(2) {
        let (from, to) = (&pair[0], &pair[1]);
        if along <= to.position {
            let span = (to.position - from.position).max(f32::EPSILON);
            let mut position = (along - from.position) / span;
            #[expect(
                clippy::float_cmp,
                reason = "1.0 is the sentinel for a linear stop; powf(1.0) is identity, so this only skips the call"
            )]
            if from.exponent != 1.0 {
                position = position.powf(from.exponent);
            }
            return from.color.mix(&to.color, position);
        }
    }
    stops.last().map_or(LinearRgba::NONE, |stop| stop.color)
}

/// Alpha-over composite of every gradient at `point`, in linear space.
pub(crate) fn sample_gradients(gradients: &[ResolvedGradient], point: Vec2) -> Option<LinearRgba> {
    let mut composite: Option<LinearRgba> = None;
    for gradient in gradients {
        let top = gradient.sample(point);
        composite = Some(match composite {
            Some(under) => under.mix(&top.with_alpha(1.0), top.alpha.clamp(0.0, 1.0)),
            None => top,
        });
    }
    composite
}

pub(crate) struct ResolvedShadow {
    pub(crate) rect: Rect,
    pub(crate) color: LinearRgba,
}

pub(crate) fn resolve_shadows(
    shadow: &BoxShadow,
    node_rect: Rect,
    size: Vec2,
    target: Vec2,
) -> Vec<ResolvedShadow> {
    shadow
        .0
        .iter()
        .filter_map(|style| resolve_shadow(style, node_rect, size, target))
        .collect()
}

// Blur has no cell equivalent: it only extends the rect by half itself.
fn resolve_shadow(
    style: &ShadowStyle,
    node_rect: Rect,
    size: Vec2,
    target: Vec2,
) -> Option<ResolvedShadow> {
    let resolve =
        |value: bevy_ui::Val, base: f32| value.resolve(CELL_SCALE, base, target).unwrap_or(0.0);
    let offset_x = resolve(style.x_offset, size.x);
    let offset_y = resolve(style.y_offset, size.y);
    let grow = resolve(style.spread_radius, size.x) + resolve(style.blur_radius, size.x) / 2.0;
    let rect = offset_and_inflate(node_rect, Vec2::new(offset_x, offset_y), grow)?;
    let color = style.color.to_linear();
    (color.alpha > f32::EPSILON).then_some(ResolvedShadow { rect, color })
}

fn offset_and_inflate(rect: Rect, offset: Vec2, grow: f32) -> Option<Rect> {
    let grow = grow.round() as i32;
    let left = i32::from(rect.x) + offset.x.round() as i32 - grow;
    let top = i32::from(rect.y) + offset.y.round() as i32 - grow;
    let width = i32::from(rect.width) + 2 * grow;
    let height = i32::from(rect.height) + 2 * grow;
    if width <= 0 || height <= 0 {
        return None;
    }
    let clip_x = left.max(0);
    let clip_y = top.max(0);
    let width = (width - (clip_x - left)).max(0) as u16;
    let height = (height - (clip_y - top)).max(0) as u16;
    (width > 0 && height > 0).then(|| Rect::new(clip_x as u16, clip_y as u16, width, height))
}

/// Darkens a cell background toward the shadow color; `None` reads as
/// terminal-default black.
pub(crate) fn dim_toward(existing: Option<[u8; 3]>, shadow: LinearRgba) -> CellColor {
    let base = existing.map_or(LinearRgba::BLACK, |[red, green, blue]| {
        Srgba::rgb_u8(red, green, blue).into()
    });
    let factor = SHADOW_BLEND * shadow.alpha.clamp(0.0, 1.0);
    linear_cell_color(base.mix(&shadow.with_alpha(1.0), factor))
}
