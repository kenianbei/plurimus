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

/// The node's laid-out rects in terminal cells, camera-space. Zero until
/// the first layout pass, or when no target camera resolves. Unlike
/// [`ComputedWidgetArea`](plurimus_ui::ComputedWidgetArea), which the bridge
/// maintains only for interactive nodes, every laid-out node carries one.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComputedNodeRect {
    /// The outer rect, border and padding included.
    pub rect: Rect,
    /// The content box: `rect` minus border and padding.
    pub content: Rect,
    /// `rect` intersected with the node's inherited clip; equals `rect`
    /// when nothing clips the node. What input arbitration hit-tests.
    pub visible: Rect,
}

// Edge rounding keeps adjacent nodes gapless: each edge rounds
// independently, width is the rounded-edge difference.
pub(crate) fn cell_rect(center: Vec2, size: Vec2, viewport: Rect) -> Option<Rect> {
    let left = (center.x - size.x / 2.0).round().max(0.0) as u16;
    let top = (center.y - size.y / 2.0).round().max(0.0) as u16;
    let right = (center.x + size.x / 2.0).round().max(0.0) as u16;
    let bottom = (center.y + size.y / 2.0).round().max(0.0) as u16;
    if right <= left || bottom <= top {
        return None;
    }
    let rect = Rect::new(
        viewport.x.saturating_add(left),
        viewport.y.saturating_add(top),
        right - left,
        bottom - top,
    );
    Some(rect)
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
    let rect = cell_rect(transform.translation, computed.size, viewport)?;
    let content_box = computed.content_box();
    let content = cell_rect(
        transform.translation + content_box.center(),
        content_box.size(),
        viewport,
    )
    .unwrap_or_default();
    let visible = clip.map_or(rect, |clip| {
        rect.intersection(clip_cells(clip.clip, viewport))
    });
    Some(ComputedNodeRect {
        rect,
        content,
        visible,
    })
}

// Edge-wise, not center/size: a scroll clip is infinite on its free
// axis, and infinity minus infinity is NaN.
pub(crate) fn clip_cells(clip: bevy_math::Rect, viewport: Rect) -> Rect {
    let width = f32::from(viewport.width);
    let height = f32::from(viewport.height);
    let left = clip.min.x.clamp(0.0, width).round() as u16;
    let top = clip.min.y.clamp(0.0, height).round() as u16;
    let right = clip.max.x.clamp(0.0, width).round() as u16;
    let bottom = clip.max.y.clamp(0.0, height).round() as u16;
    Rect::new(
        viewport.x.saturating_add(left),
        viewport.y.saturating_add(top),
        right.saturating_sub(left),
        bottom.saturating_sub(top),
    )
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
