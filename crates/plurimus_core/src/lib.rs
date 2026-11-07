//! The render pipeline every plurimus tier builds on: it turns a Bevy world
//! into cells and hands them to a ratatui `Backend`.
//!
//! The pipeline and nothing else. Everything here means something against
//! any backend - a test harness, a GPU surface, a file - so nothing that
//! needs a real terminal belongs in this crate, and it depends on no
//! plurimus crate that does. The terminal contract itself, in both
//! directions, is `plurimus_term`.
//!
//! [`CorePlugin`] installs a dedicated render sub-app. Each frame the
//! [`ExtractSchedule`] copies terminal-relevant data out of the main world,
//! then the [`TerminalRender`] schedule runs three phases in order:
//! [`Rasterize`](TerminalRenderSystems::Rasterize) writes cells into per-camera
//! [`CameraBuffer`]s, [`Composite`](TerminalRenderSystems::Composite) merges
//! those into one [`FrameBuffer`] in camera order, and
//! [`Present`](TerminalRenderSystems::Present) diffs that frame against the
//! previous one so [`PresenterPlugin`] writes only the changed cells through
//! any ratatui [`Backend`](ratatui_core::backend::Backend).
//!
//! Alongside the pipeline this crate owns what the other crates extend:
//! [`TerminalCamera`], a placement resolved against [`TerminalSize`],
//! the subcell rasterization primitives in [`raster`], and the widget
//! primitive - a [`UiWidget`] placed by [`UiArea`] and drawn in the ui pass.

mod camera;
mod compositor;
mod cursor;
mod extract;
mod present;
pub mod raster;
mod size;
mod sub_app;
mod viewport;
mod widget;

pub use bevy_app::Propagate;
pub use bevy_camera;
pub use ratatui_core;

pub use camera::{
    CameraBuffer, DefaultCamera, ExtractedCamera, SourceCamera, TerminalCamera, camera_buffer_mut,
};
pub use compositor::FrameBuffer;
pub use cursor::TerminalCursor;
pub use extract::MainWorld;
pub use present::{PresenterPlugin, TerminalContext};
pub use raster::ColorDepth;
pub use size::{CellSize, TerminalSize};
pub use sub_app::{
    CompositeSystems, ExtractSchedule, RasterizeSystems, TerminalRender, TerminalRenderApp,
    TerminalRenderAppExt, TerminalRenderSystems,
};
pub use viewport::{CameraSystems, CameraViewports, Edge, ResolvedViewport};
pub use widget::placement::{
    ComputedUiCamera, Hidden, UiArea, UiCamera, UiOrder, local_area, resolve_area, resolve_camera,
};
pub use widget::raster::{ExtractedWidget, RasterDeferred, WidgetRasterize};
pub use widget::{TerminalWidget, UiWidget};

use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::prelude::IntoScheduleConfigs;

/// Registers the terminal render sub-app and core rendering resources.
pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerminalSize>();
        app.init_resource::<CellSize>();
        app.init_resource::<DefaultCamera>();
        app.init_resource::<raster::ColorDepth>();
        app.init_resource::<TerminalCursor>();
        app.configure_sets(
            PreUpdate,
            (
                CameraSystems::SyncSize,
                CameraSystems::PropagateCameras,
                CameraSystems::ResolveViewports,
            )
                .chain(),
        );
        app.add_systems(
            PreUpdate,
            camera::update_default_camera.in_set(CameraSystems::SyncSize),
        );
        widget::placement::install(app);
        app.add_systems(
            PreUpdate,
            viewport::resolve_camera_viewports.in_set(CameraSystems::ResolveViewports),
        );
        sub_app::install(app);
        // In the sub-app rather than with the presenter: the extract system
        // below writes it whether or not anything presents.
        app.sub_app_mut(TerminalRenderApp)
            .init_resource::<TerminalCursor>();
        app.add_extract_systems((
            widget::raster::extract_widgets,
            cursor::extract_terminal_cursor,
        ));
        app.add_terminal_systems(
            TerminalRenderSystems::Rasterize,
            widget::raster::rasterize_widgets
                .in_set(RasterizeSystems::Ui)
                .in_set(WidgetRasterize),
        );
    }
}
