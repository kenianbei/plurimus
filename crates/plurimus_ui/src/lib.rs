//! UI pipeline for plurimus: interaction, focus, navigation, and
//! scrolling over widget entities.
//!
//! Widgets render with ratatui semantics into their target camera's buffer;
//! see [`UiWidget`] for the overlay contract. Widget libraries
//! ([`plurimus_widgets`](https://docs.rs/plurimus_widgets)) build on the
//! primitives here.
//!
//! Everything works from one idea: an entity with a screen area is
//! interactive. [`ComputedWidgetArea`] is what makes an entity hoverable and
//! clickable, so anything that can produce a rect joins in - a ratatui widget,
//! a `bevy_ui` node, or an app's own component - and the [`UiSystems`] phases
//! run in a fixed order each frame so areas exist before hover resolves and
//! hover exists before input routes.

mod focus;
mod interaction;
mod modal;
mod nav;
mod scroll;
mod scrolled;

pub use tui_scrollview;

pub use focus::FocusWithin;
pub use interaction::ValueChange;
pub use interaction::{
    Checked, Click, ComputedWidgetArea, Hovered, InteractionDisabled, PointerDrag, PointerPress,
    PointerRelease, Pressed,
};
pub use modal::{ModalDismiss, ModalOpen, ModalityToggle};
pub use nav::NavigationConfig;
pub use plurimus_core::{TerminalWidget, UiArea, UiCamera, UiHidden, UiOrder, UiWidget};
pub use scroll::{
    ScrollArea, ScrollIntoView, ScrollOffset, WheelAxes, WheelReceptive, WheelScroll, apply_offset,
    max_offset,
};
pub use scrolled::LiveWidget;

use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::prelude::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemSet;
use plurimus_core::{CameraSystems, TerminalRenderApp, TerminalRenderAppExt};
use plurimus_input::InputSystems;

/// Ordered phases of ui maintenance in `PreUpdate`, after
/// [`InputSystems::Update`] and [`CameraSystems::ResolveViewports`].
/// Widget libraries interleave their systems against these.
#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum UiSystems {
    /// Widget screen areas are attached and computed.
    Areas,
    /// Hover state resolves from the cursor.
    Hover,
    /// Navigation maps rebuild and pointer/wheel input routes.
    Route,
}

/// Rasterizes widget entities into terminal camera buffers and drives
/// pointer/focus interaction.
///
/// Requires [`plurimus_core::CorePlugin`] to be added first.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<plurimus_input::InputPlugin>() {
            app.add_plugins(plurimus_input::InputPlugin);
        }
        focus::install(app);
        app.configure_sets(
            PreUpdate,
            (UiSystems::Areas, UiSystems::Hover, UiSystems::Route)
                .chain()
                .after(InputSystems::Update)
                .after(CameraSystems::ResolveViewports),
        );
        app.add_systems(
            PreUpdate,
            (
                (
                    interaction::attach_widget_areas,
                    interaction::compute_widget_areas,
                )
                    .chain()
                    .in_set(UiSystems::Areas),
                interaction::hover_widgets.in_set(UiSystems::Hover),
                (
                    nav::build_navigation_map,
                    scroll::sync_scroll_area_axes,
                    scroll::route_wheel,
                    interaction::pointer_interaction,
                )
                    .chain()
                    .in_set(UiSystems::Route),
            ),
        );
        app.add_observer(scroll::scroll_into_view);
        app.add_observer(scroll::scroll_area_wheel);
        app.add_extract_systems(scrolled::extract_scrolled_widgets);
        app.sub_app_mut(TerminalRenderApp)
            .init_resource::<scrolled::ScrolledContentCache>();
    }
}
