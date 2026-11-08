//! A real terminal changing size.
//!
//! `plurimus_core` owns [`TerminalSize`] because a render target has one
//! whether or not a terminal is involved; the message reporting that a
//! terminal's size *changed* is terminal contract, so it lives here and
//! writes core's resource.

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use bevy_ecs::prelude::{DetectChangesMut, MessageReader, MessageWriter, Res, ResMut, With};
use bevy_ecs::system::Single;
use bevy_window::{Window, WindowEvent, WindowResized};
use plurimus_core::{CellSize, TerminalSize};

use crate::VirtualWindow;

/// A terminal size change reported by a backend.
///
/// Backends write this from their event pump, ordered before
/// [`CameraSystems::SyncSize`](plurimus_core::CameraSystems::SyncSize) so the
/// size applies in the same frame.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct TerminalResized {
    /// Number of columns.
    pub cols: u16,
    /// Number of rows.
    pub rows: u16,
}

impl TerminalResized {
    /// A resize to `cols` by `rows` cells.
    #[must_use]
    pub const fn new(cols: u16, rows: u16) -> Self {
        Self { cols, rows }
    }
}

pub(crate) fn apply_terminal_resize(
    mut resizes: MessageReader<TerminalResized>,
    mut size: ResMut<TerminalSize>,
) {
    if let Some(resized) = resizes.read().last() {
        size.set_if_neq(TerminalSize::new(resized.cols, resized.rows));
    }
}

// Reported as `bevy_winit` would: bevy_render re-reads a window target only
// on it, and a docked camera's viewport never changes with the terminal.
pub(crate) fn size_virtual_window(
    size: Res<TerminalSize>,
    cell: Res<CellSize>,
    window: Single<(Entity, &mut Window), With<VirtualWindow>>,
    mut resized: MessageWriter<WindowResized>,
    mut events: MessageWriter<WindowEvent>,
) {
    let physical = cell.pixels(size.cols, size.rows);
    let (entity, mut window) = window.into_inner();
    if window.physical_size() == physical {
        return;
    }
    window
        .resolution
        .set_physical_resolution(physical.x, physical.y);
    let logical = window.size();
    let message = WindowResized {
        window: entity,
        width: logical.x,
        height: logical.y,
    };
    resized.write(message.clone());
    events.write(message.into());
}

#[cfg(test)]
mod tests {
    use bevy_app::App;
    use bevy_ecs::message::Messages;
    use bevy_ecs::prelude::With;
    use bevy_math::UVec2;
    use bevy_window::{PrimaryWindow, Window, WindowEvent, WindowResized};
    use plurimus_core::{CellSize, CorePlugin, TerminalSize};

    use crate::{TermPlugin, VirtualWindow};

    fn window_after_frame(app: &mut App) -> (UVec2, usize) {
        app.update();
        let world = app.world_mut();
        let size = world
            .query_filtered::<&Window, With<VirtualWindow>>()
            .single(world)
            .unwrap()
            .physical_size();
        let resized = world
            .resource_mut::<Messages<WindowResized>>()
            .drain()
            .count();
        let in_stream = world
            .resource_mut::<Messages<WindowEvent>>()
            .drain()
            .filter(|event| matches!(event, WindowEvent::WindowResized(_)))
            .count();
        assert_eq!(
            in_stream, resized,
            "each resize is in the ordered stream too"
        );
        (size, resized)
    }

    #[test]
    fn the_virtual_window_follows_the_terminal_in_cell_pixels() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, TermPlugin));
        assert_eq!(window_after_frame(&mut app), (UVec2::new(80, 24), 1));
        assert_eq!(window_after_frame(&mut app), (UVec2::new(80, 24), 0));
        app.insert_resource(TerminalSize::new(40, 12));
        assert_eq!(window_after_frame(&mut app), (UVec2::new(40, 12), 1));
        app.insert_resource(CellSize { x: 8.0, y: 16.0 });
        assert_eq!(window_after_frame(&mut app), (UVec2::new(320, 192), 1));
    }

    #[test]
    fn a_window_the_app_opened_is_never_resized() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, TermPlugin));
        let own = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        app.update();
        app.update();
        let world = app.world_mut();
        assert_eq!(world.query::<&VirtualWindow>().iter(world).count(), 0);
        let window = world.get::<Window>(own).unwrap();
        assert_eq!(window.physical_size(), Window::default().physical_size());
        assert_eq!(world.resource::<Messages<WindowResized>>().len(), 0);
    }
}
