//! Turning laid-out bevy_ui nodes into paint-ready cell commands.
//!
//! Each camera's node tree is walked once and every [`ComputedNode`] becomes
//! a cell rect with its colors, gradients, shadows, corner glyphs and clip
//! already resolved, so the rasterizer only has to paint what it is handed.
//! Paint order is settled here as well - siblings by [`ZIndex`], then the
//! whole list by [`GlobalZIndex`] - which is why nothing downstream has to
//! reason about the tree.

use std::collections::HashMap;

use bevy_color::ColorToPacked;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::{ChildOf, Children};
use bevy_ecs::prelude::{ResMut, Resource, With, Without};
use bevy_ecs::world::World;
use bevy_math::Vec2;
use bevy_ui::{
    BackgroundColor, BackgroundGradient, BorderColor, BorderGradient, BoxShadow, CalculatedClip,
    ComputedNode, ComputedUiTargetCamera, GlobalZIndex, Gradient, ZIndex,
};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::Color as CellColor;
use plurimus_core::{MainWorld, ResolvedViewport, TerminalCamera};

use super::decorate::{Decoration, resolve_gradients, resolve_shadows, rounded_corners};
use super::rect::{ComputedNodeRect, clip_cells};
use super::text::{Seg, Text, TextStyle, resolved_spans};

#[derive(Resource, Default)]
pub(crate) struct ExtractedBuiNodes(pub(crate) Vec<ExtractedBuiNode>);

pub(crate) struct ExtractedBuiNode {
    pub(crate) camera: Entity,
    pub(crate) rect: Rect,
    pub(crate) clip: Option<Rect>,
    pub(crate) background: Option<CellColor>,
    pub(crate) border: Option<BorderSides>,
    pub(crate) text: Option<TextRun>,
    pub(crate) decoration: Decoration,
    global_z: i32,
}

pub(crate) struct BorderSides {
    pub(crate) top: Option<CellColor>,
    pub(crate) right: Option<CellColor>,
    pub(crate) bottom: Option<CellColor>,
    pub(crate) left: Option<CellColor>,
}

pub(crate) struct TextRun {
    pub(crate) spans: Vec<Seg>,
    pub(crate) content: Rect,
}

pub(crate) fn extract_bui_nodes(
    mut main_world: ResMut<MainWorld>,
    mut out: ResMut<ExtractedBuiNodes>,
) {
    out.0.clear();
    let viewports: HashMap<Entity, Rect> = {
        let mut cameras = main_world.query::<(Entity, &TerminalCamera, &ResolvedViewport)>();
        cameras
            .iter(&main_world)
            .filter(|(_, camera, _)| camera.active)
            .map(|(entity, _, resolved)| (entity, resolved.0))
            .collect()
    };
    let mut roots = main_world.query_filtered::<Entity, (With<ComputedNode>, Without<ChildOf>)>();
    let mut root_list: Vec<Entity> = roots.iter(&main_world).collect();
    root_list.sort();
    let mut queue = PaintQueue {
        viewports: &viewports,
        nodes: &mut out.0,
    };
    for root in root_list {
        visit(&main_world, root, 0, &mut queue);
    }
    out.0.sort_by_key(|node| node.global_z);
}

struct PaintQueue<'a> {
    viewports: &'a HashMap<Entity, Rect>,
    nodes: &'a mut Vec<ExtractedBuiNode>,
}

// A node's GlobalZIndex re-keys its whole subtree; the final sort must
// stay stable so traversal order survives within equal z.
fn visit(world: &World, entity: Entity, inherited_z: i32, queue: &mut PaintQueue) {
    let global_z = world
        .get::<GlobalZIndex>(entity)
        .map_or(inherited_z, |z| z.0);
    if let Some(mut node) = convert(world, entity, queue.viewports) {
        node.global_z = global_z;
        queue.nodes.push(node);
    }
    let Some(children) = world.get::<Children>(entity) else {
        return;
    };
    let mut ordered: Vec<Entity> = children.iter().copied().collect();
    ordered.sort_by_key(|&child| (world.get::<ZIndex>(child).map_or(0, |z| z.0), child));
    for child in ordered {
        visit(world, child, global_z, queue);
    }
}

fn convert(
    world: &World,
    entity: Entity,
    viewports: &HashMap<Entity, Rect>,
) -> Option<ExtractedBuiNode> {
    let computed = world.get::<ComputedNode>(entity)?;
    let camera = world.get::<ComputedUiTargetCamera>(entity)?.get()?;
    let viewport = *viewports.get(&camera)?;
    let rects = *world.get::<ComputedNodeRect>(entity)?;
    if rects.rect.is_empty() {
        return None;
    }
    let clip = world
        .get::<CalculatedClip>(entity)
        .map(|clip| clip_cells(clip.clip, viewport));
    let frame = NodeFrame {
        rect: rects.rect,
        viewport,
    };
    let decoration = node_decoration(world, entity, computed, frame);
    let (background, border) = solid_colors(
        world,
        entity,
        computed,
        !decoration.border_gradients.is_empty(),
    );
    Some(ExtractedBuiNode {
        camera,
        rect: rects.rect,
        clip,
        background,
        border,
        text: text_run(world, entity, rects.content),
        decoration,
        global_z: 0,
    })
}

fn solid_colors(
    world: &World,
    entity: Entity,
    computed: &ComputedNode,
    has_border_gradient: bool,
) -> (Option<CellColor>, Option<BorderSides>) {
    let background = world
        .get::<BackgroundColor>(entity)
        .and_then(|background| cell_color(background.0));
    let border = border_sides(
        computed,
        world.get::<BorderColor>(entity),
        has_border_gradient,
    );
    (background, border)
}

struct NodeFrame {
    rect: Rect,
    viewport: Rect,
}

fn node_decoration(
    world: &World,
    entity: Entity,
    computed: &ComputedNode,
    frame: NodeFrame,
) -> Decoration {
    let size = computed.size;
    let target = Vec2::new(
        f32::from(frame.viewport.width),
        f32::from(frame.viewport.height),
    );
    let gradients = |list: Option<&Vec<Gradient>>| {
        list.map(|list| resolve_gradients(list, size, target))
            .unwrap_or_default()
    };
    Decoration {
        corners: rounded_corners(computed.border_radius),
        background_gradients: gradients(world.get::<BackgroundGradient>(entity).map(|g| &g.0)),
        border_gradients: gradients(world.get::<BorderGradient>(entity).map(|g| &g.0)),
        shadows: world
            .get::<BoxShadow>(entity)
            .map(|shadow| resolve_shadows(shadow, frame.rect, size, target))
            .unwrap_or_default(),
    }
}

fn cell_color(color: bevy_color::Color) -> Option<CellColor> {
    let srgba = color.to_srgba();
    if srgba.alpha <= f32::EPSILON {
        return None;
    }
    let [red, green, blue] = srgba.to_u8_array_no_alpha();
    Some(CellColor::Rgb(red, green, blue))
}

// A border gradient colors any side with border width, even without a
// solid BorderColor; the Reset placeholder is overpainted per cell.
fn border_sides(
    computed: &ComputedNode,
    colors: Option<&BorderColor>,
    has_gradient: bool,
) -> Option<BorderSides> {
    let border = computed.border();
    let side = |width: f32, color: Option<bevy_color::Color>| {
        if width < 0.5 {
            return None;
        }
        match color.and_then(cell_color) {
            Some(color) => Some(color),
            None if has_gradient => Some(CellColor::Reset),
            None => None,
        }
    };
    let sides = BorderSides {
        top: side(border.min_inset.y, colors.map(|colors| colors.top)),
        right: side(border.max_inset.x, colors.map(|colors| colors.right)),
        bottom: side(border.max_inset.y, colors.map(|colors| colors.bottom)),
        left: side(border.min_inset.x, colors.map(|colors| colors.left)),
    };
    [sides.top, sides.right, sides.bottom, sides.left]
        .iter()
        .any(Option::is_some)
        .then_some(sides)
}

fn text_run(world: &World, entity: Entity, content: Rect) -> Option<TextRun> {
    let text = world.get::<Text>(entity)?;
    if content.is_empty() {
        return None;
    }
    let style = world.get::<TextStyle>(entity).copied().unwrap_or_default();
    Some(TextRun {
        spans: resolved_spans(text, style.0),
        content,
    })
}
