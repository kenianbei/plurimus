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
//! `bevy_picking` through `bevy_ui`'s own backend, fed from the terminal's
//! mouse, so the upstream `bevy_ui_widgets` run on them unchanged - all but
//! `TextInputPlugin`, which edits through `bevy_text` and so has nothing to
//! draw with here.

mod camera;
mod decorate;
mod extract;
mod picking;
mod rasterize;
mod rect;
mod stack;
mod text;
mod visibility;

pub use bevy_picking;
pub use bevy_text;
pub use bevy_ui_widgets;
pub use rect::{CellSize, ComputedNodeRect};

use bevy_app::{
    App, HierarchyPropagatePlugin, Plugin, PostUpdate, PreUpdate, PropagateSet, Startup,
};
use bevy_ecs::prelude::{
    DetectChanges, DetectChangesMut, IntoScheduleConfigs, Query, Ref, Res, ResMut,
};
use bevy_input::mouse::MouseScrollPixelsPerLine;
use bevy_input_focus::pointer_focus::PointerFocusPlugin;
use bevy_picking::{InteractionPlugin, PickingPlugin, PickingSettings, PickingSystems};
use bevy_text::{EmSize, RemSize};
use bevy_ui::picking_backend::{UiPickingPlugin, ui_picking};
use bevy_ui::ui_surface::UiSurface;
use bevy_ui::update::{propagate_ui_target_cameras, update_clipping_system};
use bevy_ui::{
    ComputedUiRenderTargetInfo, ComputedUiTargetCamera, Node, UiScale, UiStack, UiSystems,
    ui_layout_system,
};
use bevy_ui_widgets::popover::PopoverPlugin;
use bevy_ui_widgets::{
    ButtonPlugin, CheckboxPlugin, DialogPlugin, ListBoxPlugin, MenuPlugin, ModalDialogPlugin,
    RadioGroupPlugin, ScrollAreaPlugin, ScrollbarPlugin, SliderPlugin, TabPlugin,
};
use plurimus_core::{
    RasterizeSystems, TerminalRenderApp, TerminalRenderAppExt, TerminalRenderSystems,
};
use plurimus_term::InputSystems;
use plurimus_ui::{UiPlugin, UiSystems as PuiSystems};

/// Runs `bevy_ui`'s layout and picking against terminal cameras, at
/// [`CellSize`] pixels per cell, with the fontless `bevy_ui_widgets` plugins
/// installed.
pub struct BuiPlugin;

impl Plugin for BuiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<UiPlugin>() {
            app.add_plugins(UiPlugin);
        }
        stand_in_for_omitted_plugins(app);
        add_picking(app);
        add_upstream_widgets(app);
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
                (camera::sync_bui_cameras, propagate_ui_target_cameras)
                    .chain()
                    .in_set(UiSystems::Prepare),
                visibility::propagate_visibility.in_set(UiSystems::Propagate),
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

// Window picking is off: a press on no node is `plurimus_ui`'s to route,
// and a window hit would clear the focus it left alone.
fn add_picking(app: &mut App) {
    if !app.is_plugin_added::<PickingPlugin>() {
        app.add_plugins((PickingPlugin, InteractionPlugin));
    }
    if !app.is_plugin_added::<PointerFocusPlugin>() {
        app.add_plugins(PointerFocusPlugin);
    }
    if !app.is_plugin_added::<UiPickingPlugin>() {
        app.add_plugins(UiPickingPlugin);
    }
    app.world_mut()
        .resource_mut::<PickingSettings>()
        .is_window_picking_enabled = false;
    app.add_systems(Startup, picking::spawn_pointer);
    app.add_systems(
        PreUpdate,
        (
            picking::feed_pointer
                .after(InputSystems::Update)
                .before(PickingSystems::ProcessInput),
            picking::gate_hits
                .in_set(PickingSystems::Backend)
                .after(ui_picking)
                .after(PuiSystems::Areas),
        ),
    );
}

fn add_upstream_widgets(app: &mut App) {
    app.init_resource::<MouseScrollPixelsPerLine>();
    macro_rules! add {
        ($($plugin:ident),* $(,)?) => {
            $(if !app.is_plugin_added::<$plugin>() {
                app.add_plugins($plugin);
            })*
        };
    }
    add!(
        ButtonPlugin,
        CheckboxPlugin,
        RadioGroupPlugin,
        SliderPlugin,
        TabPlugin,
        ListBoxPlugin,
        MenuPlugin,
        PopoverPlugin,
        DialogPlugin,
        ModalDialogPlugin,
        ScrollAreaPlugin,
        ScrollbarPlugin,
    );
}

fn stand_in_for_omitted_plugins(app: &mut App) {
    app.init_resource::<UiSurface>();
    app.init_resource::<UiScale>();
    app.init_resource::<UiStack>();
    app.init_resource::<CellSize>();
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
