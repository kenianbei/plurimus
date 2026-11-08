//! The terminal render sub-app: the schedules and system sets every pipeline
//! registers into.
//!
//! `bevy_extract`'s [`ExtractPlugin`] builds it: bevy's [`ExtractSchedule`],
//! the only place [`MainWorld`](bevy_extract::MainWorld) is reachable, then
//! [`TerminalRender`], whose [`TerminalRenderSystems`] phases apply what
//! extraction spawned, chain rasterize into composite into present, and
//! despawn the frame's temporaries. Pipeline crates never reach into the
//! sub-app themselves - they register through [`TerminalRenderAppExt`], which
//! is what keeps the sub-app's internals private to this crate.

use bevy_app::{App, AppLabel};
use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule, ScheduleLabel, SystemSet};
use bevy_ecs::system::ScheduleSystem;
use bevy_extract::extract_resource::ExtractResourcePlugin;
use bevy_extract::{ExtractPlugin, ExtractSchedule};
use ratatui_core::backend::Backend;

use crate::camera::{self, DefaultCamera};
use crate::compositor::{self, FrameBuffer};
use crate::present::TerminalContext;
use crate::{ColorDepth, TerminalCursor, TerminalSize, raster};

/// Label of the terminal render sub-app.
#[derive(AppLabel, Debug, Clone, Copy, Default, Hash, PartialEq, Eq)]
pub struct TerminalRenderApp;

/// Update schedule of the terminal render sub-app.
#[derive(ScheduleLabel, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct TerminalRender;

/// Ordered phases of the [`TerminalRender`] schedule.
#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[non_exhaustive]
pub enum TerminalRenderSystems {
    /// What the [`ExtractSchedule`] spawned and inserted is applied.
    ExtractCommands,
    /// Pipelines write cells into camera buffers.
    Rasterize,
    /// The composed frame is built; see [`CompositeSystems`] for its passes.
    Composite,
    /// The composed frame goes to the terminal.
    Present,
    /// The frame's extracted entities are despawned.
    Cleanup,
}

/// Ordered pipeline passes within [`TerminalRenderSystems::Rasterize`]:
/// world-space pipelines draw beneath the ui pipeline.
#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[non_exhaustive]
pub enum RasterizeSystems {
    /// World-space pipelines (2d, 3d).
    World,
    /// The ui pipeline, drawn on top.
    Ui,
}

/// Ordered passes within [`TerminalRenderSystems::Composite`]: apps may
/// post-process the composed frame after cameras merge and before color
/// depth is reduced, while every color is still what the widgets chose.
#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompositeSystems {
    /// Camera buffers merge into the frame buffer.
    Merge,
    /// Apps mutate or read the composed frame here.
    PostProcess,
    /// Color depth reduction, after every color has been decided.
    Downsample,
}

mod sealed {
    pub trait Sealed {}

    impl Sealed for bevy_app::App {}
}

/// Registers systems in the terminal render sub-app, and reads its backend
/// back, without exposing its internals.
///
/// Sealed: bevy's `App` is the only sensible implementor, so a downstream
/// one would be a mistake, and sealing is what lets a method appear as the
/// sub-app grows one.
pub trait TerminalRenderAppExt: sealed::Sealed {
    /// Adds systems to `set` within the [`TerminalRender`] schedule.
    fn add_terminal_systems<M>(
        &mut self,
        set: TerminalRenderSystems,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self;

    /// Adds systems to the [`ExtractSchedule`].
    fn add_extract_systems<M>(
        &mut self,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self;

    /// The backend a [`PresenterPlugin<B>`](crate::PresenterPlugin) draws
    /// through, which is how a headless app reads back what it presented.
    ///
    /// `None` when no presenter for `B` is installed: none was added, it
    /// was given a different backend type, or
    /// [`CorePlugin`](crate::CorePlugin) is missing.
    #[must_use]
    fn terminal_backend<B: Backend + Send + Sync + 'static>(&self) -> Option<&B>;
}

impl TerminalRenderAppExt for App {
    fn add_terminal_systems<M>(
        &mut self,
        set: TerminalRenderSystems,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.sub_app_mut(TerminalRenderApp)
            .add_systems(TerminalRender, systems.in_set(set));
        self
    }

    fn add_extract_systems<M>(
        &mut self,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.sub_app_mut(TerminalRenderApp)
            .add_systems(ExtractSchedule, systems);
        self
    }

    fn terminal_backend<B: Backend + Send + Sync + 'static>(&self) -> Option<&B> {
        let world = self.get_sub_app(TerminalRenderApp)?.world();
        Some(&world.get_resource::<TerminalContext<B>>()?.backend)
    }
}

pub(crate) fn install(app: &mut App) {
    app.add_plugins(ExtractPlugin::<TerminalRenderApp>::new(
        |_, _| {},
        terminal_render_schedule,
        TerminalRender.intern(),
        TerminalRenderSystems::ExtractCommands.intern(),
        TerminalRenderSystems::Cleanup.intern(),
    ));
    let sub_app = app.sub_app_mut(TerminalRenderApp);
    sub_app.update_schedule = Some(TerminalRender.intern());
    sub_app.init_resource::<FrameBuffer>();
    sub_app.init_resource::<TerminalSize>();
    sub_app.init_resource::<ColorDepth>();
    sub_app.init_resource::<DefaultCamera>();
    sub_app.init_resource::<TerminalCursor>();
    sub_app.add_systems(ExtractSchedule, camera::extract_cameras);
    sub_app.add_systems(
        TerminalRender,
        (
            compositor::composite.in_set(CompositeSystems::Merge),
            raster::downsample_frame.in_set(CompositeSystems::Downsample),
        ),
    );
    app.add_plugins((
        ExtractResourcePlugin::<TerminalSize, TerminalRenderApp>::default(),
        ExtractResourcePlugin::<ColorDepth, TerminalRenderApp>::default(),
        ExtractResourcePlugin::<DefaultCamera, TerminalRenderApp>::default(),
        ExtractResourcePlugin::<TerminalCursor, TerminalRenderApp>::default(),
    ));
}

fn terminal_render_schedule() -> Schedule {
    let mut schedule = Schedule::new(TerminalRender);
    schedule.configure_sets(
        (
            TerminalRenderSystems::ExtractCommands,
            TerminalRenderSystems::Rasterize,
            TerminalRenderSystems::Composite,
            TerminalRenderSystems::Present,
            TerminalRenderSystems::Cleanup,
        )
            .chain(),
    );
    schedule.configure_sets(
        (RasterizeSystems::World, RasterizeSystems::Ui)
            .chain()
            .in_set(TerminalRenderSystems::Rasterize),
    );
    schedule.configure_sets(
        (
            CompositeSystems::Merge,
            CompositeSystems::PostProcess,
            CompositeSystems::Downsample,
        )
            .chain()
            .in_set(TerminalRenderSystems::Composite),
    );
    schedule
}

#[cfg(test)]
mod tests {
    use bevy_app::App;
    use ratatui_core::backend::TestBackend;

    use super::TerminalRenderAppExt;
    use crate::CorePlugin;

    #[test]
    fn no_backend_is_read_back_before_a_presenter_owns_one() {
        let mut app = App::new();
        assert!(app.terminal_backend::<TestBackend>().is_none());

        app.add_plugins(CorePlugin);

        assert!(app.terminal_backend::<TestBackend>().is_none());
    }
}
