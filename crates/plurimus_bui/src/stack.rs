//! `bevy_ui`'s paint and pick order, computed here because the system that
//! fills [`UiStack`] upstream is private to `bevy_ui`'s own plugin.
//!
//! The order is upstream's: every root node, and every node carrying a
//! `GlobalZIndex`, is a stack root; roots sort by global z, then local z;
//! each root's subtree follows it depth first with siblings by `ZIndex`,
//! stopping at a nested stack root. Equal keys fall back to spawn order,
//! so a later root stands over an earlier one, as a root new to upstream
//! stands over those it already held.

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{DetectChangesMut, Local, Query, ResMut, With};
use bevy_ui::experimental::{UiChildren, UiRootNodes};
use bevy_ui::{ComputedStackIndex, GlobalZIndex, UiStack, ZIndex};

type NodeQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static GlobalZIndex>,
        Option<&'static ZIndex>,
    ),
    With<ComputedStackIndex>,
>;

// An `Entity` orders by its bits, with a later index first; the index
// itself orders roots the way they were spawned.
type StackRoot = (i32, i32, u32, Entity);

const fn stack_root(
    (entity, global_z, local_z): (Entity, Option<&GlobalZIndex>, Option<&ZIndex>),
) -> StackRoot {
    (
        match global_z {
            Some(z) => z.0,
            None => 0,
        },
        match local_z {
            Some(z) => z.0,
            None => 0,
        },
        entity.index_u32(),
        entity,
    )
}

pub(crate) fn compute_ui_stack(
    mut stack: ResMut<UiStack>,
    (roots, children, nodes): (UiRootNodes, UiChildren, NodeQuery),
    mut indices: Query<&mut ComputedStackIndex>,
    mut siblings: Local<Vec<(i32, Entity)>>,
) {
    let mut stack_roots: Vec<StackRoot> = nodes
        .iter_many(roots.iter())
        .matched()
        .chain(nodes.iter().filter(|(_, global_z, _)| global_z.is_some()))
        .map(stack_root)
        .collect();
    stack_roots.sort_unstable();
    stack_roots.dedup();

    let UiStack { partition, uinodes } = &mut *stack;
    partition.clear();
    uinodes.clear();
    for (_, _, _, root) in stack_roots {
        let start = uinodes.len();
        push_subtree(root, (&children, &nodes), uinodes, &mut siblings);
        partition.push(start..uinodes.len());
    }
    for (index, &entity) in stack.uinodes.iter().enumerate() {
        if let Ok(mut stack_index) = indices.get_mut(entity) {
            stack_index.set_if_neq(ComputedStackIndex(index as u32));
        }
    }
}

// One sibling buffer serves every level: each call sorts the slice it
// appended and truncates it away on the way out.
fn push_subtree(
    entity: Entity,
    (children, nodes): (&UiChildren, &NodeQuery),
    out: &mut Vec<Entity>,
    siblings: &mut Vec<(i32, Entity)>,
) {
    out.push(entity);
    let start = siblings.len();
    siblings.extend(children.iter_ui_children(entity).filter_map(|child| {
        let (_, global_z, local_z) = nodes.get(child).ok()?;
        global_z
            .is_none()
            .then(|| (local_z.map_or(0, |z| z.0), child))
    }));
    siblings[start..].sort_by_key(|&(z, _)| z);
    for index in start..siblings.len() {
        push_subtree(siblings[index].1, (children, nodes), out, siblings);
    }
    siblings.truncate(start);
}
