//! UI pipeline for plurimus: interaction, focus, navigation, scrolling,
//! and theming over widget entities.
//!
//! Widgets render with ratatui semantics into their target camera's buffer;
//! see [`UiWidget`] for the overlay contract. Widget libraries
//! ([`plurimus_widgets`](https://docs.rs/plurimus_widgets)) build on the
//! primitives here.
//!
//! Everything works from one idea: an entity with a screen area is
//! interactive. [`ComputedWidgetArea`] is what makes an entity pickable, so
//! anything that can produce a rect joins in - a ratatui widget, a
//! `bevy_ui` node, or an app's own component - and this crate is the
//! `bevy_picking` backend that reports those rects, so bevy's hover map,
//! pointer events, capture and click counts are the pointer vocabulary a
//! widget observes. A press captures the pointer to the widget it landed on,
//! so its drag and release reach it wherever the pointer goes, and
//! [`click_cell`] is how a widget tells a click that ended on it from one
//! that did not. The [`UiSystems`] phases run in a fixed order each frame so
//! areas exist before the backend hit-tests them and hit-testing is done
//! before the wheel routes.
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
mod pointer;
mod scroll;
mod scrolled;
mod stylist;
mod text;
mod theme;

pub use bevy_input_focus;
pub use bevy_picking;
pub use bevy_ui_widgets;
pub use tui_scrollview;

pub use bevy_input::keyboard::Key;
pub use cursor::WidgetCursor;
pub use interaction::{Activate, ValueChange};
pub use interaction::{
    Checked, ComputedDisabled, ComputedWidgetArea, Hovered, InteractionDisabled,
    PressFocusDisabled, Pressed, Selectable, Selected,
};
pub use keys::{KeyBinding, first_bound};
pub use modal::{ModalDismiss, ModalOpen, ModalityToggle};
pub use plurimus_core::{Hidden, Propagate, TerminalWidget, UiArea, UiCamera, UiOrder, UiWidget};
pub use pointer::{click_cell, pointer_cell};
pub use scroll::{
    ScrollArea, ScrollBy, ScrollIntoView, ScrollOffset, WheelReceptive, apply_offset, content_cell,
    max_offset, screen_cell,
};
pub use scrolled::LiveWidget;
pub use stylist::{
    LabeledQuery, StateQuery, Stylable, StylistCache, UiLabel, decorate, hashed_bits, observed,
    restyle,
};
pub use text::{
    DrawnRow, TextInput, TextInputAction, TextInputKeys, TextRow, cluster_len_after,
    cluster_len_before, unbound_text,
};
pub use theme::{InteractionState, StylistDisabled, UiStyle, UiTheme};

use bevy_app::{App, First, Last, Plugin, PluginGroup, PreUpdate, Startup};
use bevy_ecs::prelude::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemSet;
use bevy_input_focus::pointer_focus::PointerFocusPlugin;
use bevy_picking::input::{PointerInputPlugin, PointerInputSettings};
use bevy_picking::{InteractionPlugin, PickingPlugin, PickingSystems};
use bevy_ui::UiScale;
use bevy_ui_widgets::{ButtonPlugin, TextInputPlugin, UiWidgetsPlugins};
use plurimus_core::{CameraSystems, TerminalRenderApp, TerminalRenderAppExt};
use plurimus_term::InputSystems;

/// Ordered phases of ui maintenance in `PreUpdate`, after
/// [`CameraSystems::ResolveViewports`], around `bevy_picking`'s own. Widget
/// libraries interleave their systems against these.
///
/// Focus dispatch sits between [`Areas`](Self::Areas) and
/// [`PickingSystems::Hover`], and after `bevy_input`'s own update, so a
/// `FocusedInput` observer reads this frame's [`ComputedWidgetArea`] and a
/// settled `ButtonInput`, and runs before the frame's pointer events.
/// Anything that has to see what such an observer did - forwarding a keyed
/// edit, restyling from it - belongs after [`InputFocusSystems::Dispatch`]
/// rather than after [`Areas`](Self::Areas) alone.
///
/// [`InputFocusSystems::Dispatch`]: bevy_input_focus::InputFocusSystems::Dispatch
#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[non_exhaustive]
pub enum UiSystems {
    /// Widget screen areas are attached and computed, and
    /// [`ComputedDisabled`] is resolved; before [`PickingSystems::Backend`].
    Areas,
    /// Navigation maps rebuild and wheel input routes; after
    /// [`PickingSystems::Hover`], so the frame's pointer events have run.
    Route,
}

/// Rasterizes widget entities into terminal camera buffers and drives
/// pointer/focus interaction.
///
/// Requires [`plurimus_core::CorePlugin`] to be added first. Installs
/// `bevy_picking` with window picking on, so a press on nothing clears
/// focus as it does on a window, and its mouse pointer, fed from the window
/// events the terminal backend writes; touch stays off, a terminal having
/// none to report.
///
/// Installs `bevy_ui_widgets`' observers too, all but `TextInputPlugin`,
/// whose field is a node that `plurimus_bui`'s `BuiPlugin` adds it for, and
/// `PointerFocusPlugin`, since a
/// press already moves focus here and a second writer would race it - so
/// upstream's `Button`, `Checkbox` and `RadioGroup` work on a cell widget
/// as on a node.
///
/// A focus plugin the app adds before this one is the app's to configure.
/// `InputFocusPlugin` first skips `InputDispatchPlugin` and
/// `TabNavigationPlugin` too; `DirectionalNavigationPlugin` first skips the
/// arrow keys and the grid tuning in `AutoNavigationConfig`, while the map is
/// still built from widget areas for the app's own keys to navigate.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<plurimus_term::TermPlugin>() {
            app.add_plugins(plurimus_term::TermPlugin);
        }
        if !app.is_plugin_added::<PickingPlugin>() {
            app.add_plugins((PickingPlugin, InteractionPlugin));
        }
        if !app.is_plugin_added::<PointerInputPlugin>() {
            app.add_plugins(PointerInputPlugin);
            app.insert_resource(PointerInputSettings {
                is_touch_enabled: false,
                is_mouse_enabled: true,
            });
        }
        focus::install(app);
        add_upstream_widgets(app);
        app.init_resource::<UiTheme>();
        app.init_resource::<pointer::DeferredCancel>();
        add_schedule(app);
        app.add_observer(pointer::capture_press);
        app.add_observer(pointer::release_press);
        app.add_observer(pointer::press_focus);
        app.add_observer(pointer::toggle_dismiss);
        app.add_observer(scroll::scroll_into_view);
        app.add_observer(scroll::scroll_area_scrolled);
        app.add_extract_systems(scrolled::extract_scrolled_widgets);
        app.sub_app_mut(TerminalRenderApp)
            .init_resource::<scrolled::ScrolledContentCache>();
    }
}

// Upstream's slider, scrollbar and dialog presses read `UiScale`, which no
// plugin installed here provides, and a missing one fails the observer.
fn add_upstream_widgets(app: &mut App) {
    if app.is_plugin_added::<ButtonPlugin>() {
        return;
    }
    app.add_plugins(
        UiWidgetsPlugins
            .build()
            .disable::<TextInputPlugin>()
            .disable::<PointerFocusPlugin>(),
    );
    app.init_resource::<UiScale>();
}

fn add_schedule(app: &mut App) {
    // The terminal's input lands before picking reads the frame's pointer
    // events, so a pump never lags them a frame.
    app.configure_sets(
        First,
        InputSystems::Synthesize.before(PickingSystems::Input),
    );
    app.configure_sets(
        PreUpdate,
        (
            UiSystems::Areas
                .after(CameraSystems::ResolveViewports)
                .before(PickingSystems::Backend),
            UiSystems::Route.after(PickingSystems::Hover),
        ),
    );
    app.add_systems(Startup, pointer::spawn_barrier);
    app.add_systems(
        First,
        pointer::cancel_on_focus_loss.after(PickingSystems::Input),
    );
    app.add_systems(Last, pointer::flush_deferred_cancel);
    app.add_systems(
        PreUpdate,
        (
            (
                interaction::attach_widget_areas,
                interaction::compute_widget_areas,
            )
                .chain()
                .in_set(UiSystems::Areas),
            interaction::propagate_disabled.in_set(UiSystems::Areas),
            pointer::pick_widgets.in_set(PickingSystems::Backend),
            pointer::confine_hits
                .after(PickingSystems::Backend)
                .before(PickingSystems::Hover),
            // After picking's input, before the hover pass releases the
            // capture it reads.
            pointer::cancel_gestures
                .after(PickingSystems::ProcessInput)
                .before(PickingSystems::Hover),
            (nav::build_navigation_map, scroll::route_wheel).in_set(UiSystems::Route),
        ),
    );
    // In Last so it sees a caret moved anywhere this frame - a key handler
    // in focus dispatch, a stylist in Update, a node field placed after
    // layout - and still before the sub-app extracts it.
    app.add_systems(Last, cursor::sync_terminal_cursor);
}
