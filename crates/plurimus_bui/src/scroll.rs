//! Wheel scrolling for bui nodes with scroll overflow.
//!
//! A node whose [`Overflow`] is set to scroll registers as a wheel target, so
//! `plurimus_ui`'s wheel router treats it exactly like a scrollable widget.
//! Ticks are applied to `bevy_ui`'s own [`ScrollPosition`] rather than to any
//! state of ours, which is what keeps scrolled layout the business of the
//! next taffy pass instead of something this crate has to recompute.

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Has, On, Query};
use bevy_math::Vec2;
use bevy_ui::{ComputedNode, Node, Overflow, OverflowAxis, ScrollPosition};

use super::rect::ComputedNodeRect;
use super::{publish_area, upsert};
use plurimus_ui::ComputedWidgetArea;
use plurimus_ui::{ScrollBy, WheelAxes, WheelReceptive};

type BridgeQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Node,
        &'static ComputedNodeRect,
        Option<&'static mut ComputedWidgetArea>,
        Option<&'static mut WheelAxes>,
        Has<WheelReceptive>,
    ),
>;

// Registration tracks `overflow`, so a node that stops scrolling stops
// winning ticks.
pub(crate) fn sync_bui_wheel_targets(mut nodes: BridgeQuery, mut commands: Commands) {
    for (entity, node, rect, area, axes, registered) in &mut nodes {
        let overflow = overflow_axes(node.overflow);
        if !(overflow.horizontal || overflow.vertical) {
            if registered {
                commands.entity(entity).remove::<WheelReceptive>();
            }
            continue;
        }
        publish_area(&mut commands, entity, area, rect.visible);
        upsert(&mut commands, entity, axes, overflow);
        if !registered {
            commands.entity(entity).insert(WheelReceptive);
        }
    }
}

// Uses the previous frame's layout (same one-frame lag as bevy_picking);
// bevy_ui clamps and applies the position during the next layout pass.
pub(crate) fn bui_node_scrolled(
    event: On<ScrollBy>,
    mut nodes: Query<(&ComputedNode, &mut ScrollPosition)>,
) {
    let Ok((computed, mut scroll)) = nodes.get_mut(event.entity) else {
        return;
    };
    let max = (computed.content_size - computed.size).max(Vec2::ZERO);
    let stepped = scroll.0 + Vec2::new(event.step.0 as f32, event.step.1 as f32);
    let clamped = stepped.clamp(Vec2::ZERO, max);
    if scroll.0 != clamped {
        scroll.0 = clamped;
    }
}

fn overflow_axes(overflow: Overflow) -> WheelAxes {
    WheelAxes {
        horizontal: overflow.x == OverflowAxis::Scroll,
        vertical: overflow.y == OverflowAxis::Scroll,
    }
}
