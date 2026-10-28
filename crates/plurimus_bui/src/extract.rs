//! Turning laid-out `bevy_ui` nodes into paint-ready cell commands.
//!
//! Each camera's node tree is walked once and every [`ComputedNode`] becomes
//! a cell rect with its colors, gradients, shadows, corner glyphs and clip
//! already resolved, so the rasterizer only has to paint what it is handed.
//! Paint order is `bevy_ui`'s own [`UiStack`], back to front, so what is
//! drawn over what agrees with what is picked over what.

use std::collections::HashMap;

use bevy_camera::visibility::InheritedVisibility;
use bevy_color::ColorToPacked;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ResMut, Resource};
use bevy_ecs::world::World;
use bevy_math::Vec2;
use bevy_ui::{
    BackgroundColor, BackgroundGradient, BorderColor, BorderGradient, BoxShadow, CalculatedClip,
    ComputedNode, ComputedUiTargetCamera, Gradient, Outline, UiGlobalTransform, UiStack,
};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::Color as CellColor;
use plurimus_core::{MainWorld, ResolvedViewport, TerminalCamera};

use super::decorate::{Decoration, ValBasis, resolve_gradients, resolve_shadows, rounded_corners};
use super::rect::{CellBox, CellSize, clip_cells, node_boxes};
use super::text::ResolvedText;

#[derive(Resource)]
pub(crate) struct ExtractedBuiNodes {
    pub(crate) nodes: Vec<ExtractedBuiNode>,
    /// The frame's [`CellSize`], for sampling pixel geometry at cell centers.
    pub(crate) cell: Vec2,
}

impl Default for ExtractedBuiNodes {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            cell: CellSize::default().vec(),
        }
    }
}

pub(crate) struct ExtractedBuiNode {
    pub(crate) camera: Entity,
    pub(crate) rect: CellBox,
    pub(crate) clip: Option<Rect>,
    pub(crate) background: Option<CellColor>,
    pub(crate) border: Option<BorderSides>,
    pub(crate) outline: Option<OutlineRing>,
    pub(crate) text: Option<TextRun>,
    pub(crate) decoration: Decoration,
}

pub(crate) struct BorderSides {
    pub(crate) top: Option<CellColor>,
    pub(crate) right: Option<CellColor>,
    pub(crate) bottom: Option<CellColor>,
    pub(crate) left: Option<CellColor>,
}

/// One cell of border glyphs around the node, where `Outline` draws a ring
/// of any width outside the border box.
pub(crate) struct OutlineRing {
    pub(crate) rect: CellBox,
    pub(crate) color: CellColor,
}

pub(crate) struct TextRun {
    pub(crate) text: ResolvedText,
    pub(crate) content: CellBox,
}

pub(crate) fn extract_bui_nodes(
    mut main_world: ResMut<MainWorld>,
    mut out: ResMut<ExtractedBuiNodes>,
) {
    out.nodes.clear();
    out.cell = main_world.resource::<CellSize>().vec();
    let cell = out.cell;
    let viewports: HashMap<Entity, Rect> = {
        let mut cameras = main_world.query::<(Entity, &TerminalCamera, &ResolvedViewport)>();
        cameras
            .iter(&main_world)
            .filter(|(_, camera, _)| camera.active)
            .map(|(entity, _, resolved)| (entity, resolved.0))
            .collect()
    };
    let stack = main_world.resource::<UiStack>();
    out.nodes.extend(
        stack
            .uinodes
            .iter()
            .filter_map(|&entity| convert(&main_world, entity, &viewports, cell)),
    );
}

fn convert(
    world: &World,
    entity: Entity,
    viewports: &HashMap<Entity, Rect>,
    cell: Vec2,
) -> Option<ExtractedBuiNode> {
    if !world
        .get::<InheritedVisibility>(entity)
        .is_some_and(|visibility| visibility.get())
    {
        return None;
    }
    let computed = world.get::<ComputedNode>(entity)?;
    let camera = world.get::<ComputedUiTargetCamera>(entity)?.get()?;
    let viewport = *viewports.get(&camera)?;
    let transform = world.get::<UiGlobalTransform>(entity)?;
    let (rect, content) = node_boxes(computed, transform, viewport, cell);
    if rect.is_empty() {
        return None;
    }
    let clip = world
        .get::<CalculatedClip>(entity)
        .map(|clip| clip_cells(clip, viewport, cell));
    let on_screen = rect.clamped(viewport);
    let on_screen = clip.map_or(on_screen, |clip| on_screen.intersection(clip));
    // Only a shadow can reach the screen from a node that is not on it.
    if on_screen.is_empty() && !world.entity(entity).contains::<BoxShadow>() {
        return None;
    }
    let frame = NodeFrame {
        rect,
        viewport,
        cell,
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
        rect,
        clip,
        background,
        border,
        outline: outline_ring(world, entity, computed, rect, cell),
        text: text_run(world, entity, content),
        decoration,
    })
}

fn outline_ring(
    world: &World,
    entity: Entity,
    computed: &ComputedNode,
    rect: CellBox,
    cell: Vec2,
) -> Option<OutlineRing> {
    if computed.outline_width() <= 0.0 {
        return None;
    }
    let color = cell_color(world.get::<Outline>(entity)?.color)?;
    let gap = Vec2::splat(computed.outline_offset()) / cell;
    let (gap_x, gap_y) = (1 + gap.x.round() as i32, 1 + gap.y.round() as i32);
    Some(OutlineRing {
        rect: CellBox {
            left: rect.left - gap_x,
            top: rect.top - gap_y,
            right: rect.right + gap_x,
            bottom: rect.bottom + gap_y,
        },
        color,
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
    rect: CellBox,
    viewport: Rect,
    cell: Vec2,
}

fn node_decoration(
    world: &World,
    entity: Entity,
    computed: &ComputedNode,
    frame: NodeFrame,
) -> Decoration {
    let size = computed.size;
    let basis = ValBasis {
        target: Vec2::new(
            f32::from(frame.viewport.width),
            f32::from(frame.viewport.height),
        ) * frame.cell,
        em: computed.em_size,
        rem: computed.rem_size,
    };
    let gradients = |list: Option<&Vec<Gradient>>| {
        list.map(|list| resolve_gradients(list, size, basis))
            .unwrap_or_default()
    };
    Decoration {
        corners: rounded_corners(computed.border_radius, frame.cell),
        background_gradients: gradients(world.get::<BackgroundGradient>(entity).map(|g| &g.0)),
        border_gradients: gradients(world.get::<BorderGradient>(entity).map(|g| &g.0)),
        shadows: world
            .get::<BoxShadow>(entity)
            .map(|shadow| resolve_shadows(shadow, frame.rect, size, basis, frame.cell))
            .unwrap_or_default(),
    }
}

pub(crate) fn cell_color(color: bevy_color::Color) -> Option<CellColor> {
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

fn text_run(world: &World, entity: Entity, content: CellBox) -> Option<TextRun> {
    let text = world.get::<ResolvedText>(entity)?;
    if content.is_empty() {
        return None;
    }
    Some(TextRun {
        text: text.clone(),
        content,
    })
}
