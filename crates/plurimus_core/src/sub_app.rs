//! The terminal render sub-app: the schedules and system sets every pipeline
//! registers into.
//!
//! The sub-app runs two schedules: [`ExtractSchedule`], the only place
//! [`MainWorld`](crate::MainWorld) is reachable, and [`TerminalRender`], whose
//! [`TerminalRenderSystems`] phases chain rasterize into composite into
//! present. Pipeline crates never reach into the sub-app themselves - they
//! register through [`TerminalRenderAppExt`], which is what keeps the
//! sub-app's internals private to this crate.

use bevy_app::{App, AppLabel, SubApp};
use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule, ScheduleLabel, SystemSet};
use bevy_ecs::system::ScheduleSystem;

use crate::compositor::{self, FrameBuffer};
use crate::{camera, extract, raster, size};

/// Label of the terminal render sub-app.
#[derive(AppLabel, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct TerminalRenderApp;

/// Schedule with access to [`crate::MainWorld`], run before
/// [`TerminalRender`] each frame.
#[derive(ScheduleLabel, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct ExtractSchedule;

/// Update schedule of the terminal render sub-app.
#[derive(ScheduleLabel, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct TerminalRender;

/// Ordered phases of the [`TerminalRender`] schedule.
#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum TerminalRenderSystems {
    /// Pipelines write cells into camera buffers.
    Rasterize,
    /// Camera buffers merge into the frame buffer, which is then
    /// post-processed and reduced to the terminal's color depth.
    Composite,
    /// The composed frame goes to the terminal.
    Present,
}

/// Ordered pipeline passes within [`TerminalRenderSystems::Rasterize`]:
/// world-space pipelines draw beneath the ui pipeline.
#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
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
pub enum CompositeSystems {
    /// Camera buffers merge into the frame buffer.
    Merge,
    /// Apps mutate or read the composed frame here.
    PostProcess,
    /// Color depth reduction, after every color has been decided.
    Downsample,
}

/// Registers systems in the terminal render sub-app without exposing its
/// internals.
pub trait TerminalRenderAppExt {
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
}

pub(crate) fn install(app: &mut App) {
    app.insert_resource(extract::ScratchMainWorld::default());
    let mut sub_app = SubApp::new();
    sub_app.update_schedule = Some(TerminalRender.intern());
    sub_app.add_schedule(Schedule::new(ExtractSchedule));
    sub_app.add_schedule(Schedule::new(TerminalRender));
    sub_app.configure_sets(
        TerminalRender,
        (
            TerminalRenderSystems::Rasterize,
            TerminalRenderSystems::Composite,
            TerminalRenderSystems::Present,
        )
            .chain(),
    );
    sub_app.configure_sets(
        TerminalRender,
        (RasterizeSystems::World, RasterizeSystems::Ui)
            .chain()
            .in_set(TerminalRenderSystems::Rasterize),
    );
    sub_app.configure_sets(
        TerminalRender,
        (
            CompositeSystems::Merge,
            CompositeSystems::PostProcess,
            CompositeSystems::Downsample,
        )
            .chain()
            .in_set(TerminalRenderSystems::Composite),
    );
    sub_app.init_resource::<FrameBuffer>();
    sub_app.init_resource::<size::TerminalSize>();
    sub_app.init_resource::<raster::ColorDepth>();
    sub_app.add_systems(
        ExtractSchedule,
        (
            size::extract_size,
            camera::extract_cameras,
            raster::extract_color_depth,
        ),
    );
    sub_app.add_systems(
        TerminalRender,
        (
            compositor::composite.in_set(CompositeSystems::Merge),
            raster::downsample_frame.in_set(CompositeSystems::Downsample),
        ),
    );
    sub_app.set_extract(extract::extract);
    app.insert_sub_app(TerminalRenderApp, sub_app);
}
