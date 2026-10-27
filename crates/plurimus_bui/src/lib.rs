//! `bevy_ui` layout translated to terminal cells: real `Node` trees, taffy
//! layout at one pixel per cell, rasterized by the terminal pipeline.
//!
//! Only `bevy_ui`'s layout stack runs; its text, focus, picking, and asset
//! systems stay out. Text uses grapheme-width measurement instead of fonts.
//!
//! Computed nodes rasterize beneath every widget, and their areas and wheel
//! targets bridge into `plurimus_ui`'s routers, so a `bevy_ui` tree hovers,
//! clicks and scrolls like any other widget without `bevy_ui`'s own picking.

mod camera;
mod decorate;
mod extract;
mod interact;
mod rasterize;
mod rect;
mod scroll;
mod text;

pub use rect::ComputedNodeRect;
pub use text::{Text, TextSpan, TextStyle};

use bevy_app::{App, HierarchyPropagatePlugin, Plugin, PostUpdate, PreUpdate, PropagateSet};
use bevy_camera::visibility::InheritedVisibility;
use bevy_ecs::prelude::{DetectChanges, DetectChangesMut, IntoScheduleConfigs, Query, Ref, Res};
use bevy_text::{EmSize, RemSize};
use bevy_ui::ui_surface::UiSurface;
use bevy_ui::update::{propagate_ui_target_cameras, update_clipping_system};
use bevy_ui::{
    ComputedUiRenderTargetInfo, ComputedUiTargetCamera, Node, UiScale, UiSystems, ui_layout_system,
};
use plurimus_core::{
    CameraSystems, RasterizeSystems, TerminalRenderApp, TerminalRenderAppExt, TerminalRenderSystems,
};
use plurimus_term::InputSystems;
use plurimus_ui::{UiPlugin, UiSystems as PuiSystems};

// Below all unordered widgets: the rasterizer draws every node under every
// widget, so input arbitration has to agree with what the user sees.
const BUI_ORDER: plurimus_core::UiOrder = plurimus_core::UiOrder(i32::MIN / 2);
// One pixel per cell, and text one row tall: a rem is a cell.
const CELL_REM: RemSize = RemSize(1.0);

/// Runs `bevy_ui`'s layout systems against terminal cameras, one pixel per
/// cell.
pub struct BuiPlugin;

impl Plugin for BuiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<UiPlugin>() {
            app.add_plugins(UiPlugin);
        }
        stand_in_for_omitted_plugins(app);
        app.configure_sets(
            PostUpdate,
            (
                UiSystems::Prepare,
                UiSystems::Propagate,
                UiSystems::Content,
                UiSystems::Layout,
                UiSystems::PostLayout,
            )
                .chain(),
        );
        register_propagation(app);
        app.add_systems(
            PostUpdate,
            (
                (camera::sync_bui_cameras, propagate_ui_target_cameras)
                    .chain()
                    .in_set(UiSystems::Prepare),
                (text::measure_bui_text, sync_em_sizes).in_set(UiSystems::Content),
                ui_layout_system.in_set(UiSystems::Layout),
                (update_clipping_system, rect::compute_node_rects).in_set(UiSystems::PostLayout),
            ),
        );
        app.add_systems(
            PreUpdate,
            (
                interact::sync_node_widget_areas,
                scroll::sync_bui_wheel_targets,
            )
                .after(InputSystems::Update)
                .after(CameraSystems::ResolveViewports)
                .before(PuiSystems::Hover),
        );
        app.add_observer(scroll::bui_node_scrolled);
        app.sub_app_mut(TerminalRenderApp)
            .init_resource::<extract::ExtractedBuiNodes>();
        app.add_extract_systems(extract::extract_bui_nodes);
        app.add_terminal_systems(
            TerminalRenderSystems::Rasterize,
            rasterize::rasterize_bui
                .in_set(RasterizeSystems::Ui)
                .before(plurimus_core::WidgetRasterize),
        );
    }
}

fn stand_in_for_omitted_plugins(app: &mut App) {
    app.init_resource::<UiSurface>();
    app.init_resource::<UiScale>();
    app.init_resource::<bevy_text::FontCx>();
    app.world_mut().get_resource_or_insert_with(|| CELL_REM);
    // Tab navigation skips what is not inherited-visible, which bui never
    // computes; the rasterizer ignores `Visibility` too.
    app.register_required_components_with::<Node, InheritedVisibility>(|| {
        InheritedVisibility::VISIBLE
    });
}

// bevy_ui derives an em only from a `TextFont`, which bridged text never has,
// so every node's em is the rem.
fn sync_em_sizes(rem: Res<RemSize>, mut nodes: Query<(&mut EmSize, Ref<Node>)>) {
    let (em_size, is_rem_changed) = (EmSize(rem.0), rem.is_changed());
    for (mut em, node) in &mut nodes {
        if is_rem_changed || node.is_added() {
            em.set_if_neq(em_size);
        }
    }
}

// An explicit UiOrder on a node overrides the banding.
fn publish_area(
    commands: &mut bevy_ecs::prelude::Commands,
    entity: bevy_ecs::entity::Entity,
    existing: Option<bevy_ecs::change_detection::Mut<plurimus_ui::ComputedWidgetArea>>,
    rect: plurimus_core::ratatui_core::layout::Rect,
) {
    upsert(
        commands,
        entity,
        existing,
        plurimus_ui::ComputedWidgetArea(rect),
    );
    commands.entity(entity).insert_if_new(BUI_ORDER);
}

fn upsert<
    C: bevy_ecs::component::Component<Mutability = bevy_ecs::component::Mutable> + PartialEq,
>(
    commands: &mut bevy_ecs::prelude::Commands,
    entity: bevy_ecs::entity::Entity,
    existing: Option<bevy_ecs::change_detection::Mut<C>>,
    value: C,
) {
    use bevy_ecs::change_detection::DetectChangesMut;
    match existing {
        Some(mut existing) => {
            existing.set_if_neq(value);
        }
        None => {
            commands.entity(entity).insert(value);
        }
    }
}

fn register_propagation(app: &mut App) {
    propagate::<ComputedUiTargetCamera>(app);
    propagate::<ComputedUiRenderTargetInfo>(app);
}

fn propagate<T: bevy_ecs::prelude::Component + Clone + PartialEq>(app: &mut App) {
    app.configure_sets(
        PostUpdate,
        PropagateSet::<T>::default().in_set(UiSystems::Propagate),
    );
    app.add_plugins(HierarchyPropagatePlugin::<T>::new(PostUpdate));
}
