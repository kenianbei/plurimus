//! Adoption of the bevy input-focus stack for terminal apps.
//!
//! `bevy_input_focus` dispatches a frame's `KeyboardInput` to the focused
//! entity; this file pins when, so an observer reads this frame's areas and
//! settled key state, and adds the terminal's paste to what is dispatched.

use bevy_app::{App, PreUpdate};
use bevy_ecs::prelude::IntoScheduleConfigs;
use bevy_input_focus::tab_navigation::TabNavigationPlugin;
use bevy_input_focus::{
    InputDispatchPlugin, InputFocusPlugin, InputFocusSystems, dispatch_focused_input,
};
use bevy_picking::PickingSystems;
use plurimus_term::PasteMessage;

use crate::UiSystems;

pub(crate) fn install(app: &mut App) {
    if !app.is_plugin_added::<InputFocusPlugin>() {
        app.add_plugins((InputFocusPlugin, InputDispatchPlugin, TabNavigationPlugin));
    }
    crate::nav::install(app);
    // On the set rather than the systems, so a dispatch an app adds
    // inherits the edges; upstream orders only the three it registers.
    app.configure_sets(
        PreUpdate,
        InputFocusSystems::Dispatch
            .after(bevy_input::InputSystems)
            .after(UiSystems::Areas)
            .before(PickingSystems::Hover),
    );
    app.add_systems(
        PreUpdate,
        dispatch_focused_input::<PasteMessage>.in_set(InputFocusSystems::Dispatch),
    );
}
