//! The composite phase: every camera buffer merged into the one frame the
//! presenter sends to the terminal.
//!
//! The [`FrameBuffer`] is resized to the terminal and cleared each frame, then
//! camera buffers merge into it in camera order, so a higher-order camera
//! lands on top. How a camera merges depends on its [`Background`]: a
//! [`Background::Transparent`] camera copies only the cells it actually drew
//! and leaves lower cameras showing through, while the others merge wholesale
//! and their untouched cells overwrite whatever was beneath.

use bevy_ecs::prelude::{Query, Res, ResMut, Resource};
use ratatui_core::buffer::{Buffer, Cell};
use ratatui_core::layout::Rect;

use crate::camera::{Background, CameraBuffer, ExtractedCamera};
use crate::size::TerminalSize;

/// The composed full-terminal frame, consumed by the presenter.
#[derive(Resource, Debug)]
pub struct FrameBuffer(pub Buffer);

impl Default for FrameBuffer {
    fn default() -> Self {
        Self(Buffer::empty(Rect::ZERO))
    }
}

pub(crate) fn composite(
    mut frame: ResMut<FrameBuffer>,
    size: Res<TerminalSize>,
    cameras: Query<(&ExtractedCamera, &CameraBuffer)>,
) {
    frame.0.resize(size.rect());
    frame.0.reset();
    let mut ordered: Vec<_> = cameras.iter().collect();
    ordered.sort_by_key(|(camera, _)| camera.order);
    for (camera, buffer) in ordered {
        if buffer.0.area.is_empty() {
            continue;
        }
        match camera.background {
            Background::Transparent => merge_transparent(&mut frame.0, &buffer.0),
            Background::TerminalDefault | Background::Clear(_) => frame.0.merge(&buffer.0),
        }
    }
}

fn merge_transparent(frame: &mut Buffer, layer: &Buffer) {
    for position in layer.area.positions() {
        let Some(cell) = layer.cell(position) else {
            continue;
        };
        if *cell != Cell::EMPTY
            && let Some(target) = frame.cell_mut(position)
        {
            *target = cell.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy_app::App;
    use bevy_ecs::prelude::Query;
    use ratatui_core::layout::Rect;
    use ratatui_core::style::{Color, Style};

    use crate::{
        Background, CameraBuffer, CorePlugin, ExtractedCamera, FrameBuffer, TerminalCamera,
        TerminalRenderApp, TerminalRenderAppExt, TerminalRenderSystems, TerminalSize, Viewport,
    };

    fn paint(mut cameras: Query<(&ExtractedCamera, &mut CameraBuffer)>) {
        for (camera, mut buffer) in &mut cameras {
            let symbol = if camera.order > 0 { "B" } else { "A" };
            let area = buffer.0.area;
            let row = symbol.repeat(area.width as usize);
            buffer
                .0
                .set_string(area.left(), area.top(), row, Style::new());
        }
    }

    #[test]
    fn transparent_cameras_let_untouched_cells_through() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(TerminalSize { cols: 4, rows: 1 });
        app.world_mut().spawn(TerminalCamera::default());
        app.world_mut().spawn(TerminalCamera {
            order: 1,
            background: Background::Transparent,
            ..TerminalCamera::default()
        });
        app.add_terminal_systems(TerminalRenderSystems::Rasterize, paint_partial);

        app.update();

        let frame = app
            .sub_app(TerminalRenderApp)
            .world()
            .resource::<FrameBuffer>();
        let row: String = (0..4)
            .filter_map(|x| frame.0.cell((x, 0)).map(|cell| cell.symbol().to_owned()))
            .collect();
        assert_eq!(row, "AABA");
    }

    fn paint_partial(mut cameras: Query<(&ExtractedCamera, &mut CameraBuffer)>) {
        for (camera, mut buffer) in &mut cameras {
            if camera.background == Background::Transparent {
                buffer.0.set_string(2, 0, "B", Style::new());
            } else {
                let area = buffer.0.area;
                let row = "A".repeat(area.width as usize);
                buffer
                    .0
                    .set_string(area.left(), area.top(), row, Style::new());
            }
        }
    }

    #[test]
    fn clear_cameras_fill_untouched_cells_over_lower_layers() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(TerminalSize { cols: 4, rows: 1 });
        app.world_mut().spawn(TerminalCamera::default());
        app.world_mut().spawn(TerminalCamera {
            order: 1,
            viewport: Viewport::Fixed(Rect::new(2, 0, 2, 1)),
            background: Background::Clear(Color::Blue),
            ..TerminalCamera::default()
        });
        app.add_terminal_systems(TerminalRenderSystems::Rasterize, paint_base_only);

        app.update();

        let frame = app
            .sub_app(TerminalRenderApp)
            .world()
            .resource::<FrameBuffer>();
        assert_eq!(frame.0.cell((0, 0)).unwrap().symbol(), "A");
        let cleared = frame.0.cell((2, 0)).unwrap();
        assert_eq!(cleared.symbol(), " ");
        assert_eq!(cleared.bg, Color::Blue);
    }

    fn paint_base_only(mut cameras: Query<(&ExtractedCamera, &mut CameraBuffer)>) {
        for (camera, mut buffer) in &mut cameras {
            if camera.order == 0 {
                let area = buffer.0.area;
                let row = "A".repeat(area.width as usize);
                buffer
                    .0
                    .set_string(area.left(), area.top(), row, Style::new());
            }
        }
    }

    #[test]
    fn composite_layers_cameras_by_order() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(TerminalSize { cols: 4, rows: 1 });
        app.world_mut().spawn(TerminalCamera::default());
        app.world_mut().spawn(TerminalCamera {
            order: 1,
            viewport: Viewport::Fixed(Rect::new(2, 0, 2, 1)),
            ..TerminalCamera::default()
        });
        app.add_terminal_systems(TerminalRenderSystems::Rasterize, paint);

        app.update();

        let frame = app
            .sub_app(TerminalRenderApp)
            .world()
            .resource::<FrameBuffer>();
        let row: String = (0..4)
            .filter_map(|x| frame.0.cell((x, 0)).map(|cell| cell.symbol().to_owned()))
            .collect();
        assert_eq!(row, "AABB");
    }
}
