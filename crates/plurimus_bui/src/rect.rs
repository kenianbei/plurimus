//! The laid-out node rect in terminal cells: computed once per frame after
//! layout, read by interaction, scrolling, extraction, and apps.
//!
//! One node yields three rects because three different questions get asked
//! of it. `rect` is the outer box that borders and backgrounds paint into,
//! `content` is what text and children lay out inside, and `visible` is
//! `rect` narrowed by the inherited clip - the only one input arbitration
//! hit-tests, so a node scrolled out of its container stops being clickable
//! even though it still has a rect.

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Component, Query};
use bevy_math::Vec2;
use bevy_ui::{
    CalculatedClip, CalculatedClipRect, ComputedNode, ComputedUiTargetCamera, UiGlobalTransform,
};
use plurimus_core::ResolvedViewport;
use plurimus_core::ratatui_core::layout::Rect;

use super::upsert;

/// The node's laid-out rects in terminal cells, cut to its camera's viewport.
/// Zero until the first layout pass, when no target camera resolves, or when
/// the node lies wholly outside the viewport. Unlike
/// [`ComputedWidgetArea`](plurimus_ui::ComputedWidgetArea), which the bridge
/// maintains only for interactive nodes, every laid-out node carries one.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ComputedNodeRect {
    /// The outer rect, border and padding included.
    pub rect: Rect,
    /// The content box: `rect` minus border and padding.
    pub content: Rect,
    /// `rect` intersected with the node's inherited clip; equals `rect`
    /// when nothing clips the node. What input arbitration hit-tests.
    pub visible: Rect,
}

/// A node's cell edges in screen space. Signed, unlike a [`Rect`], so a node
/// hanging off its viewport keeps its true edges for painting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CellBox {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
}

impl CellBox {
    pub(crate) const fn is_empty(self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }

    /// The part of the box inside `area`, or [`Rect::ZERO`] when none is.
    pub(crate) fn clamped(self, area: Rect) -> Rect {
        let horizontal = |edge: i32| edge.clamp(area.left().into(), area.right().into()) as u16;
        let vertical = |edge: i32| edge.clamp(area.top().into(), area.bottom().into()) as u16;
        let (left, right) = (horizontal(self.left), horizontal(self.right));
        let (top, bottom) = (vertical(self.top), vertical(self.bottom));
        if right <= left || bottom <= top {
            return Rect::ZERO;
        }
        Rect::new(left, top, right - left, bottom - top)
    }
}

// Each edge rounds on its own, which keeps adjacent nodes gapless.
fn cell_box(min: Vec2, max: Vec2, viewport: Rect) -> CellBox {
    let edge = |position: f32, origin: u16| (position + f32::from(origin)).round() as i32;
    CellBox {
        left: edge(min.x, viewport.x),
        top: edge(min.y, viewport.y),
        right: edge(max.x, viewport.x),
        bottom: edge(max.y, viewport.y),
    }
}

/// The node's outer box and its content box. Painting and hit-testing both
/// derive from these, so the two agree on where every edge falls.
pub(crate) fn node_boxes(
    computed: &ComputedNode,
    transform: &UiGlobalTransform,
    viewport: Rect,
) -> (CellBox, CellBox) {
    let half = computed.size / 2.0;
    let content_box = computed.content_box();
    (
        cell_box(
            transform.translation - half,
            transform.translation + half,
            viewport,
        ),
        cell_box(
            transform.translation + content_box.min,
            transform.translation + content_box.max,
            viewport,
        ),
    )
}

type NodeGeometry<'a> = (
    &'a ComputedNode,
    &'a UiGlobalTransform,
    &'a ComputedUiTargetCamera,
    Option<&'a CalculatedClip>,
);

fn node_rects(
    geometry: NodeGeometry<'_>,
    cameras: &Query<&ResolvedViewport>,
) -> Option<ComputedNodeRect> {
    let (computed, transform, camera, clip) = geometry;
    let viewport = cameras.get(camera.get()?).ok()?.0;
    let (node, content) = node_boxes(computed, transform, viewport);
    let rect = node.clamped(viewport);
    if rect.is_empty() {
        return None;
    }
    let content = content.clamped(viewport);
    let visible = clip
        .and_then(|clip| clip_cells(clip, viewport))
        .map_or(rect, |clip| rect.intersection(clip));
    Some(ComputedNodeRect {
        rect,
        content,
        visible,
    })
}

/// The inherited clip in cells, every clipping ancestor intersected; `None`
/// when nothing clips the node.
pub(crate) fn clip_cells(clip: &CalculatedClip, viewport: Rect) -> Option<Rect> {
    let Some(rects) = clip.rects() else {
        return Some(Rect::ZERO);
    };
    rects
        .iter()
        .map(|clip| clip_rect_cells(clip, viewport))
        .reduce(Rect::intersection)
}

// An axis that does not clip is infinite, and 0 * inf in the transform would
// be NaN; past this bound an edge is off every screen anyway.
const CLIP_EDGE_BOUND: f32 = 1e9;

// Bounds the clip rect's corners in world space, so a rotated clip widens
// to its bounding box rather than vanishing.
fn clip_rect_cells(clip: &CalculatedClipRect, viewport: Rect) -> Rect {
    let bound = Vec2::splat(CLIP_EDGE_BOUND);
    let (min, max) = (clip.rect.min.max(-bound), clip.rect.max.min(bound));
    let to_world = clip.world_to_clip_local.inverse();
    let corners = [min, max, Vec2::new(min.x, max.y), Vec2::new(max.x, min.y)]
        .map(|corner| to_world.transform_point2(corner));
    let (low, high) = corners.iter().fold(
        (Vec2::INFINITY, Vec2::NEG_INFINITY),
        |(low, high), &corner| (low.min(corner), high.max(corner)),
    );
    cell_box(low, high, viewport).clamped(viewport)
}

pub(crate) fn compute_node_rects(
    cameras: Query<&ResolvedViewport>,
    mut nodes: Query<(
        Entity,
        &ComputedNode,
        &UiGlobalTransform,
        &ComputedUiTargetCamera,
        Option<&CalculatedClip>,
        Option<&mut ComputedNodeRect>,
    )>,
    mut commands: Commands,
) {
    for (entity, computed, transform, camera, clip, rect) in &mut nodes {
        let resolved =
            node_rects((computed, transform, camera, clip), &cameras).unwrap_or_default();
        upsert(&mut commands, entity, rect, resolved);
    }
}

#[cfg(test)]
mod tests {
    use super::{CellBox, clip_cells};
    use bevy_math::{Affine2, Vec2};
    use bevy_ui::CalculatedClip;
    use plurimus_core::ratatui_core::layout::Rect;

    const VIEWPORT: Rect = Rect::new(2, 1, 6, 4);

    #[test]
    fn a_box_is_cut_to_the_viewport_on_every_side() {
        let overflowing = CellBox {
            left: -3,
            top: -1,
            right: 20,
            bottom: 9,
        };

        assert_eq!(overflowing.clamped(VIEWPORT), VIEWPORT);
    }

    #[test]
    fn a_box_wholly_outside_the_viewport_is_nothing() {
        let beside = CellBox {
            left: 8,
            top: 1,
            right: 12,
            bottom: 3,
        };

        assert_eq!(beside.clamped(VIEWPORT), Rect::ZERO);
    }

    fn clip_around(center: Vec2, size: Vec2) -> CalculatedClip {
        let mut clip = CalculatedClip::default();
        clip.push_rect(
            bevy_math::Rect::from_center_size(Vec2::ZERO, size),
            Affine2::from_translation(-center),
        );
        clip
    }

    #[test]
    fn no_clipping_ancestor_is_no_clip() {
        assert_eq!(clip_cells(&CalculatedClip::default(), VIEWPORT), None);
    }

    #[test]
    fn a_fully_clipped_node_has_no_visible_cells() {
        assert_eq!(
            clip_cells(&CalculatedClip::FullyClipped, VIEWPORT),
            Some(Rect::ZERO)
        );
    }

    #[test]
    fn nested_clips_intersect_and_an_unclipped_axis_spans_the_viewport() {
        let mut clip = clip_around(Vec2::new(3.0, 2.0), Vec2::new(4.0, 2.0));
        let scroll_row = clip_around(Vec2::new(0.0, 2.5), Vec2::new(f32::INFINITY, 1.0));
        let row = scroll_row.rects().unwrap()[0];
        clip.push_rect(row.rect, row.world_to_clip_local);

        assert_eq!(
            clip_cells(&scroll_row, VIEWPORT),
            Some(Rect::new(2, 3, 6, 1))
        );
        assert_eq!(clip_cells(&clip, VIEWPORT), Some(Rect::new(3, 3, 4, 1)));
    }
}
