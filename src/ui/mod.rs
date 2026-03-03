use crate::ui::actions::{
    UiActionState, UiActionsAppExt, collect_key_actions, collect_mouse_actions, run_world_intents,
};
use crate::ui::focus::{UiFocusMessage, UiFocusState, apply_focus_intents, focus_on_pointer};
use crate::ui::pointer::{UiPointerState, update_pointer_state};
use crate::ui::state::sanitize_disabled_state;
use bevy::prelude::{App, IntoScheduleConfigs, Plugin, PreUpdate};

pub mod actions;
pub mod builder;
pub mod focus;
pub mod pointer;
pub mod state;
pub mod util;

#[derive(Default)]
pub struct PlurimusUiPlugin;

impl Plugin for PlurimusUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiActionState>();
        app.init_resource::<UiFocusState>();
        app.init_resource::<UiPointerState>();

        app.add_message::<UiFocusMessage>();
        app.ui_actions_message::<UiFocusMessage>();

        app.add_systems(
            PreUpdate,
            (
                sanitize_disabled_state,
                collect_key_actions,
                collect_mouse_actions,
                focus_on_pointer,
                run_world_intents,
                apply_focus_intents,
                update_pointer_state,
            )
                .chain(),
        );
    }
}
