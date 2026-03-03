use crate::draw::draw;
use bevy::prelude::{
    App, Fixed, FixedUpdate, IntoScheduleConfigs, Plugin, SystemSet, Time, resource_exists,
};
use bevy_ratatui::RatatuiContext;

/// System sets for fixed update stages. These are used to control the order of systems in the fixed update stage.
/// The `Layout` set is intended for systems that calculate layout, while the `Draw` set is for systems that perform drawing operations.
///
/// Example:
/// ```rust
/// use bevy::prelude::*;
/// use plurimus::PlurimusFixedSet;
///
/// fn my_layout_system() {
///     // Layout logic here
/// }
///
/// fn main() {
///     App::new()
///         .add_systems(
///             FixedUpdate,
///             my_layout_system.in_set(PlurimusFixedSet::Layout),
///         );
/// }
/// ```
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlurimusFixedSet {
    Layout,
    Draw,
}

/// The main plugin for Plurimus. This plugin sets up the necessary systems and resources for the terminal UI framework.
///
/// It configures the fixed update stages, inserts a fixed time resource for controlling the update rate, and adds the drawing system.
/// Additionally, it conditionally adds plugins for widgets and UI if the corresponding features are enabled, and sets up the TachyonFx registry if the `tachyonfx` feature is enabled.
///
/// Example:
/// ```rust
/// use bevy::prelude::*;
/// use plurimus::PlurimusPlugin;
/// use bevy_ratatui::RatatuiPlugins;
///
/// fn main() {
///     App::new()
///         .add_plugins(MinimalPlugins)
///         .add_plugins(RatatuiPlugins::default())
///         .add_plugins(PlurimusPlugin);
/// }
/// ```
#[derive(Default)]
pub struct PlurimusPlugin;

impl Plugin for PlurimusPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedUpdate,
            (PlurimusFixedSet::Layout, PlurimusFixedSet::Draw).chain(),
        );

        app.insert_resource(Time::<Fixed>::from_seconds(1.0 / 16.0));

        app.add_systems(
            FixedUpdate,
            draw.in_set(PlurimusFixedSet::Draw)
                .run_if(resource_exists::<RatatuiContext>),
        );

        #[cfg(feature = "widget")]
        app.add_plugins(crate::widget::PlurimusWidgetPlugin);

        #[cfg(feature = "ui")]
        app.add_plugins(crate::ui::PlurimusUiPlugin);

        #[cfg(feature = "tachyonfx")]
        {
            use crate::effects::*;
            app.insert_non_send_resource(TachyonRegistry::default());
            app.add_observer(on_remove_tachyon_fx_target);
        }
    }
}
