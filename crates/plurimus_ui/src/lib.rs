//! UI pipeline for plurimus: interaction, focus, navigation, scrolling,
//! and theming over widget entities.
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
//!
//! [`UiTheme`] is the other half of that contract: a widget library resolves
//! its look from the app's theme rather than owning one, so widgets from
//! different crates restyle together.

mod cursor;
mod focus;
mod interaction;
mod keys;
mod modal;
mod nav;
mod scroll;
mod scroll_keys;
mod scrolled;
mod stylist;
mod theme;

pub use tui_scrollview;

pub use bevy_input::keyboard::Key;
pub use cursor::WidgetCursor;
pub use focus::FocusWithin;
pub use interaction::ValueChange;
pub use interaction::{
    Checked, Click, ComputedWidgetArea, Hovered, InteractionDisabled, PointerDrag, PointerPress,
    PointerRelease, PressPassThrough, Pressed,
};
pub use keys::first_bound;
pub use modal::{ModalDismiss, ModalOpen, ModalityToggle};
pub use nav::NavigationConfig;
pub use plurimus_core::{TerminalWidget, UiArea, UiCamera, UiHidden, UiOrder, UiWidget};
pub use scroll::{
    ScrollArea, ScrollBy, ScrollIntoView, ScrollOffset, WheelAxes, WheelReceptive, apply_offset,
    content_cell, max_offset, screen_cell,
};
pub use scroll_keys::{ScrollAction, ScrollKeys};
pub use scrolled::LiveWidget;
pub use stylist::{
    LabeledQuery, StateQuery, Stylable, StylistCache, UiLabel, decorate, hashed_bits, observed,
    restyle,
};
pub use theme::{InteractionState, StylistDisabled, UiStyle, UiTheme};

use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::prelude::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemSet;
use plurimus_core::{CameraSystems, TerminalRenderApp, TerminalRenderAppExt};
use plurimus_term::InputSystems;

/// Ordered phases of ui maintenance in `PreUpdate`, after
/// [`InputSystems::Update`] and [`CameraSystems::ResolveViewports`].
/// Widget libraries interleave their systems against these.
///
/// Focus dispatch sits between [`Areas`](Self::Areas) and
/// [`Hover`](Self::Hover), and after `bevy_input`'s own update, so a
/// `FocusedInput` observer reads this frame's [`ComputedWidgetArea`] and
/// a settled `ButtonInput`. Anything that has to see what such an
/// observer did - forwarding a keyed edit, restyling from it - belongs
/// after `bevy_input_focus::InputFocusSystems::Dispatch` rather than
/// after [`Areas`](Self::Areas) alone.
#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[non_exhaustive]
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
        if !app.is_plugin_added::<plurimus_term::TermPlugin>() {
            app.add_plugins(plurimus_term::TermPlugin);
        }
        focus::install(app);
        app.init_resource::<UiTheme>();
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
        // In PostUpdate so it sees a caret moved anywhere this frame -
        // a key handler in focus dispatch, a stylist in Update - and still
        // before the sub-app extracts it.
        app.add_systems(bevy_app::PostUpdate, cursor::sync_terminal_cursor);
        app.add_observer(scroll::scroll_into_view);
        app.add_observer(scroll::scroll_area_scrolled);
        app.add_observer(scroll_keys::scroll_key);
        app.add_extract_systems(scrolled::extract_scrolled_widgets);
        app.sub_app_mut(TerminalRenderApp)
            .init_resource::<scrolled::ScrolledContentCache>();
    }
}
