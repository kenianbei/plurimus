//! `bevy_ui` layout translated to terminal cells: real `Node` trees, taffy
//! layout at [`CellSize`] pixels per cell, rasterized by the terminal
//! pipeline.
//!
//! Only `bevy_ui`'s layout and picking stacks run; its text, focus and asset
//! systems stay out. Text is measured by grapheme width instead of fonts, and
//! `Visibility` is propagated here, since `bevy_camera`'s propagation is not
//! installed.
//!
//! Computed nodes rasterize beneath every widget and are picked by
//! `bevy_picking` through `bevy_ui`'s own backend, on the mouse pointer the
//! terminal backend reports to `bevy_picking`, so the upstream
//! `bevy_ui_widgets` run on them unchanged - all but `TextInputPlugin`,
//! which edits through `bevy_text` and so has nothing to draw with here.
//!
//! # What a cell cannot show
//!
//! Text is one row tall whatever its `font_size`, and `TextFont::font`,
//! `TextShadow`, `UnderlineColor` and `StrikethroughColor` are ignored; a
//! weight from semibold up is bold, any non-normal style italic. A border or
//! outline thinner than a cell paints in the node's outer cell, over
//! content. Colour reaches a cell as RGB and nothing else, so a skin names
//! the colours it wants rather than a terminal palette entry.
//!
//! # What stays per target
//!
//! A tree shared with a windowed build of the same app keeps these to each
//! target: a text field, since upstream's `TextInput` needs a font engine -
//! the terminal places a `plurimus_widgets::EditableText` at the field's
//! node through `UiArea`; arrow-key navigation, which `plurimus_ui` builds
//! from widget areas here and nothing builds for nodes on a window; focus
//! restored when an upstream dialog closes, which neither target does; and
//! hover on a node beneath a plurimus widget, which the widget blocks.

mod camera;
mod decorate;
mod extract;
mod rasterize;
mod rect;
mod stack;
mod text;
mod visibility;

pub use bevy_text;
pub use bevy_ui_widgets;
pub use rect::ComputedNodeRect;

use bevy_app::{App, HierarchyPropagatePlugin, Plugin, PostUpdate, PropagateSet};
use bevy_camera::visibility::VisibilitySystems;
use bevy_ecs::prelude::{
    DetectChanges, DetectChangesMut, IntoScheduleConfigs, Query, Ref, Res, ResMut,
};
use bevy_text::{EmSize, RemSize};
use bevy_ui::picking_backend::UiPickingPlugin;
use bevy_ui::ui_surface::UiSurface;
use bevy_ui::update::{propagate_ui_target_cameras, update_clipping_system};
use bevy_ui::{
    ComputedUiRenderTargetInfo, ComputedUiTargetCamera, Node, UiStack, UiSystems, ui_layout_system,
};
use plurimus_core::{
    CellSize, RasterizeSystems, TerminalRenderApp, TerminalRenderAppExt, TerminalRenderSystems,
};
use plurimus_ui::UiPlugin;

/// Runs `bevy_ui`'s layout and picking against terminal cameras, at
/// [`CellSize`] pixels per cell; the fontless `bevy_ui_widgets` plugins
/// that drive nodes are installed by [`UiPlugin`], which this adds when
/// absent.
///
/// Upstream's constants are window pixels, cells only through [`CellSize`]:
/// a dialog drag keeps the pointer 16 px inside the window, sixteen cells at
/// the one-pixel default, so a terminal under 32 cells in an axis cannot
/// drag a dialog along it. An app dragging dialogs names its cell.
pub struct BuiPlugin;

impl Plugin for BuiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<UiPlugin>() {
            app.add_plugins(UiPlugin);
        }
        stand_in_for_omitted_plugins(app);
        add_picking(app);
        app.configure_sets(
            PostUpdate,
            (
                UiSystems::Prepare,
                UiSystems::Propagate,
                UiSystems::Content,
                UiSystems::Layout,
                UiSystems::Stack,
                UiSystems::PostLayout,
            )
                .chain(),
        );
        register_propagation(app);
        app.add_systems(
            PostUpdate,
            (
                (camera::sync_default_ui_camera, propagate_ui_target_cameras)
                    .chain()
                    .in_set(UiSystems::Prepare),
                // Upstream readers of `InheritedVisibility`, such as a
                // menu focusing its first item, order after this set.
                visibility::propagate_visibility
                    .in_set(UiSystems::Propagate)
                    .in_set(VisibilitySystems::VisibilityPropagate),
                (
                    text::resolve_bui_text,
                    (sync_rem_size, sync_em_sizes).chain(),
                )
                    .in_set(UiSystems::Content),
                ui_layout_system.in_set(UiSystems::Layout),
                stack::compute_ui_stack.in_set(UiSystems::Stack),
                (update_clipping_system, rect::compute_node_rects).in_set(UiSystems::PostLayout),
            ),
        );
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

// `plurimus_ui` installs picking, its pointer and upstream's widget
// observers; bevy_ui's backend is the one piece nodes add.
fn add_picking(app: &mut App) {
    if !app.is_plugin_added::<UiPickingPlugin>() {
        app.add_plugins(UiPickingPlugin);
    }
}

fn stand_in_for_omitted_plugins(app: &mut App) {
    app.init_resource::<UiSurface>();
    app.init_resource::<UiStack>();
    app.init_resource::<RemSize>();
    app.init_resource::<bevy_text::FontCx>();
}

// Text is one row tall, so a rem is a row, as `Val::Rem(1)` is one line of
// text on a window.
fn sync_rem_size(cell: Res<CellSize>, mut rem: ResMut<RemSize>) {
    if cell.is_changed() {
        rem.set_if_neq(RemSize(cell.y));
    }
}

// bevy_ui derives an em from a `TextFont` in a system this crate does not
// run, so every node's em is the rem.
fn sync_em_sizes(rem: Res<RemSize>, mut nodes: Query<(&mut EmSize, Ref<Node>)>) {
    let (em_size, is_rem_changed) = (EmSize(rem.0), rem.is_changed());
    for (mut em, node) in &mut nodes {
        if is_rem_changed || node.is_added() {
            em.set_if_neq(em_size);
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
