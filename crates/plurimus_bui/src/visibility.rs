//! `Visibility` resolved down the node tree into `InheritedVisibility`.
//!
//! `bevy_camera` does this for anything carrying `Visibility`, but its system
//! is private to a plugin that brings frustum culling with it, so the walk is
//! repeated here over `bevy_ui`'s own hierarchy. What it writes is what
//! extraction, `ui_picking` and Tab navigation all read, so hiding a node
//! hides its subtree from the screen, the pointer and the keyboard at once.

use bevy_camera::visibility::{InheritedVisibility, Visibility};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Changed, ChildOf, Children, DetectChangesMut, Or, Query};
use bevy_ui::experimental::{UiChildren, UiRootNodes};

type NodeQuery<'w, 's> = Query<'w, 's, (&'static Visibility, &'static mut InheritedVisibility)>;

pub(crate) fn propagate_visibility(
    roots: UiRootNodes,
    children: UiChildren,
    mut nodes: NodeQuery,
    changed: Query<(), Or<(Changed<Visibility>, Changed<ChildOf>, Changed<Children>)>>,
) {
    if changed.is_empty() {
        return;
    }
    for root in roots.iter() {
        propagate(root, true, &children, &mut nodes);
    }
}

fn propagate(entity: Entity, parent_visible: bool, children: &UiChildren, nodes: &mut NodeQuery) {
    let Ok((visibility, mut inherited)) = nodes.get_mut(entity) else {
        return;
    };
    let visible = match visibility {
        Visibility::Hidden => false,
        Visibility::Visible => true,
        Visibility::Inherited => parent_visible,
    };
    inherited.set_if_neq(if visible {
        InheritedVisibility::VISIBLE
    } else {
        InheritedVisibility::HIDDEN
    });
    for child in children.iter_ui_children(entity) {
        propagate(child, visible, children, nodes);
    }
}
