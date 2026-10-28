//! `bevy_ui`'s paint and pick order, computed here because the system that
//! fills [`UiStack`] upstream is private to `bevy_ui`'s own plugin.
//!
//! The order is upstream's: every root node, and every node carrying a
//! `GlobalZIndex`, is a stack root; roots sort by global z, then local z;
//! each root's subtree follows it depth first with siblings by `ZIndex`,
//! stopping at a nested stack root. Equal keys fall back to entity order,
//! where upstream keeps the previous frame's.

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{DetectChangesMut, Query, ResMut, With, Without};
use bevy_ecs::system::SystemParam;
use bevy_ui::experimental::{UiChildren, UiRootNodes};
use bevy_ui::{ComputedStackIndex, GlobalZIndex, UiStack, ZIndex};

type RootQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static GlobalZIndex>,
        Option<&'static ZIndex>,
    ),
    With<ComputedStackIndex>,
>;

type LocalQuery<'w, 's> =
    Query<'w, 's, Option<&'static ZIndex>, (With<ComputedStackIndex>, Without<GlobalZIndex>)>;

#[derive(SystemParam)]
pub(crate) struct StackTree<'w, 's> {
    roots: UiRootNodes<'w, 's>,
    children: UiChildren<'w, 's>,
    nodes: RootQuery<'w, 's>,
    global: Query<
        'w,
        's,
        (Entity, &'static GlobalZIndex, Option<&'static ZIndex>),
        With<ComputedStackIndex>,
    >,
    local: LocalQuery<'w, 's>,
}

pub(crate) fn compute_ui_stack(
    mut stack: ResMut<UiStack>,
    tree: StackTree,
    mut indices: Query<&mut ComputedStackIndex>,
) {
    let StackTree {
        roots,
        children,
        nodes,
        global,
        local,
    } = &tree;
    let mut stack_roots: Vec<(i32, i32, Entity)> = nodes
        .iter_many(roots.iter())
        .matched()
        .map(|(entity, global_z, local_z)| {
            (
                global_z.map_or(0, |z| z.0),
                local_z.map_or(0, |z| z.0),
                entity,
            )
        })
        .collect();
    stack_roots.extend(
        global
            .iter()
            .filter(|(entity, ..)| !roots.iter().any(|root| root == *entity))
            .map(|(entity, global_z, local_z)| (global_z.0, local_z.map_or(0, |z| z.0), entity)),
    );
    stack_roots.sort_unstable();

    let UiStack { partition, uinodes } = &mut *stack;
    partition.clear();
    uinodes.clear();
    for (_, _, root) in stack_roots {
        let start = uinodes.len();
        push_subtree(root, children, local, uinodes);
        partition.push(start..uinodes.len());
    }
    for (index, &entity) in stack.uinodes.iter().enumerate() {
        if let Ok(mut stack_index) = indices.get_mut(entity) {
            stack_index.set_if_neq(ComputedStackIndex(index as u32));
        }
    }
}

fn push_subtree(entity: Entity, children: &UiChildren, local: &LocalQuery, out: &mut Vec<Entity>) {
    out.push(entity);
    let mut ordered: Vec<(i32, Entity)> = children
        .iter_ui_children(entity)
        .filter_map(|child| local.get(child).ok().map(|z| (z.map_or(0, |z| z.0), child)))
        .collect();
    ordered.sort_by_key(|&(z, _)| z);
    for (_, child) in ordered {
        push_subtree(child, children, local, out);
    }
}
