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
use bevy_ui::{CalculatedClip, ComputedNode, ComputedUiTargetCamera, UiGlobalTransform};
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
/// hanging off its viewport keeps its true edges for painting; only
/// [`CellBox::clamped`] cuts it down to what is on-screen.
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

// Edge rounding keeps adjacent nodes gapless: each edge rounds
// independently, width is the rounded-edge difference. Edge-wise, not
// center/size, because a scroll clip is infinite on its free axis and
// infinity minus infinity is NaN; the cast saturates an infinite edge.
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
    let visible = clip.map_or(rect, |clip| {
        rect.intersection(clip_cells(clip.clip, viewport))
    });
    Some(ComputedNodeRect {
        rect,
        content,
        visible,
    })
}

pub(crate) fn clip_cells(clip: bevy_math::Rect, viewport: Rect) -> Rect {
    cell_box(clip.min, clip.max, viewport).clamped(viewport)
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
    use super::CellBox;
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
}
