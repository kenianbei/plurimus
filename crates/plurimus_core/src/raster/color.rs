//! Matching the frame's colors to what the terminal can actually show.
//!
//! Pipelines always work in true color; the frame is quantized once, after
//! compositing and before present, so nothing upstream has to know what the
//! terminal supports. [`ColorDepth`] picks the target - the xterm 256 cube
//! plus its grayscale ramp, or the 16 named ANSI colors - and only
//! `Color::Rgb` values are touched, so a widget that asked for
//! `Color::Green` still gets the terminal's green at every depth.

use bevy_ecs::prelude::{DetectChangesMut, Res, ResMut, Resource};
use ratatui_core::style::Color;

use crate::compositor::FrameBuffer;
use crate::extract::MainWorld;

const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
const CUBE_BASE: u8 = 16;
const CUBE_SIDE: u8 = 6;
const GRAY_BASE: u8 = 232;
const GRAY_STEPS: u16 = 24;
const GRAY_FIRST: u16 = 8;
const GRAY_STEP: u16 = 10;

const ANSI16_PALETTE: [(Color, [u8; 3]); 16] = [
    (Color::Black, [0, 0, 0]),
    (Color::Red, [128, 0, 0]),
    (Color::Green, [0, 128, 0]),
    (Color::Yellow, [128, 128, 0]),
    (Color::Blue, [0, 0, 128]),
    (Color::Magenta, [128, 0, 128]),
    (Color::Cyan, [0, 128, 128]),
    (Color::Gray, [192, 192, 192]),
    (Color::DarkGray, [128, 128, 128]),
    (Color::LightRed, [255, 0, 0]),
    (Color::LightGreen, [0, 255, 0]),
    (Color::LightYellow, [255, 255, 0]),
    (Color::LightBlue, [0, 0, 255]),
    (Color::LightMagenta, [255, 0, 255]),
    (Color::LightCyan, [0, 255, 255]),
    (Color::White, [255, 255, 255]),
];

/// Output color depth of the composed frame. `TrueColor` passes RGB
/// through untouched; the other depths quantize every `Color::Rgb` in
/// the frame after compositing, before present. A main-world resource,
/// defaulting to `TrueColor`.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ColorDepth {
    /// 24-bit RGB pass-through.
    #[default]
    TrueColor,
    /// The xterm 256-color palette: 6×6×6 cube plus grayscale ramp.
    Ansi256,
    /// The 16 standard ANSI colors.
    Ansi16,
}

/// Encodes a linear-space color into a terminal RGB cell color; alpha is
/// dropped.
#[must_use]
pub fn linear_cell_color(color: bevy_color::LinearRgba) -> Color {
    use bevy_color::ColorToPacked;
    let [red, green, blue] = bevy_color::Srgba::from(color).to_u8_array_no_alpha();
    Color::Rgb(red, green, blue)
}

/// Quantizes `color` to `depth`. Only `Color::Rgb` values change; named,
/// indexed, and reset colors pass through at every depth.
#[must_use]
pub fn downsample(color: Color, depth: ColorDepth) -> Color {
    let Color::Rgb(red, green, blue) = color else {
        return color;
    };
    match depth {
        ColorDepth::TrueColor => color,
        ColorDepth::Ansi256 => nearest_ansi256([red, green, blue]),
        ColorDepth::Ansi16 => nearest_ansi16([red, green, blue]),
    }
}

pub(crate) fn extract_color_depth(main_world: Res<MainWorld>, mut depth: ResMut<ColorDepth>) {
    depth.set_if_neq(*main_world.resource::<ColorDepth>());
}

pub(crate) fn downsample_frame(mut frame: ResMut<FrameBuffer>, depth: Res<ColorDepth>) {
    if *depth == ColorDepth::TrueColor {
        return;
    }
    for cell in &mut frame.0.content {
        cell.fg = downsample(cell.fg, *depth);
        cell.bg = downsample(cell.bg, *depth);
    }
}

fn nearest_ansi256(rgb: [u8; 3]) -> Color {
    let (cube_index, cube_rgb) = nearest_cube(rgb);
    let (gray_index, gray_rgb) = nearest_gray(rgb);
    if distance(rgb, gray_rgb) < distance(rgb, cube_rgb) {
        Color::Indexed(gray_index)
    } else {
        Color::Indexed(cube_index)
    }
}

fn nearest_cube(rgb: [u8; 3]) -> (u8, [u8; 3]) {
    let level_of = |channel: u8| {
        (0..CUBE_LEVELS.len())
            .min_by_key(|&level| channel.abs_diff(CUBE_LEVELS[level]))
            .expect("cube levels are non-empty") as u8
    };
    let [red, green, blue] = rgb.map(level_of);
    let index = CUBE_BASE + (red * CUBE_SIDE + green) * CUBE_SIDE + blue;
    let value = [red, green, blue].map(|level| CUBE_LEVELS[usize::from(level)]);
    (index, value)
}

fn nearest_gray(rgb: [u8; 3]) -> (u8, [u8; 3]) {
    let average = rgb.iter().map(|&channel| u16::from(channel)).sum::<u16>() / 3;
    let step =
        ((average.saturating_sub(GRAY_FIRST) + GRAY_STEP / 2) / GRAY_STEP).min(GRAY_STEPS - 1);
    let level = (GRAY_FIRST + GRAY_STEP * step) as u8;
    (GRAY_BASE + step as u8, [level; 3])
}

fn nearest_ansi16(rgb: [u8; 3]) -> Color {
    ANSI16_PALETTE
        .iter()
        .min_by_key(|(_, candidate)| distance(rgb, *candidate))
        .map(|(color, _)| *color)
        .expect("palette is non-empty")
}

fn distance(a: [u8; 3], b: [u8; 3]) -> u32 {
    a.iter()
        .zip(b)
        .map(|(&left, right)| u32::from(left.abs_diff(right)).pow(2))
        .sum()
}

#[cfg(test)]
mod tests {
    use bevy_app::App;
    use bevy_ecs::prelude::{IntoScheduleConfigs, Query, ResMut};
    use ratatui_core::style::{Color, Style};

    use super::{ColorDepth, downsample};
    use crate::{
        CameraBuffer, CompositeSystems, CorePlugin, FrameBuffer, TerminalCamera, TerminalRenderApp,
        TerminalRenderAppExt, TerminalRenderSystems, TerminalSize,
    };

    #[test]
    fn ansi16_depth_downsamples_composed_cell_colors() {
        let mut app = ansi16_app();

        app.update();

        assert_eq!(frame_fg(&app), Color::LightRed);
    }

    fn ansi16_app() -> App {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(TerminalSize::new(2, 1));
        app.insert_resource(ColorDepth::Ansi16);
        app.world_mut().spawn(TerminalCamera::default());
        app.add_terminal_systems(TerminalRenderSystems::Rasterize, paint_truecolor);
        app
    }

    fn frame_fg(app: &App) -> Color {
        app.sub_app(TerminalRenderApp)
            .world()
            .resource::<FrameBuffer>()
            .0
            .cell((0, 0))
            .unwrap()
            .fg
    }

    fn paint_truecolor(mut cameras: Query<&mut CameraBuffer>) {
        for mut buffer in &mut cameras {
            buffer
                .0
                .set_string(0, 0, "R", Style::new().fg(Color::Rgb(255, 0, 0)));
        }
    }

    #[test]
    fn post_process_runs_between_merge_and_downsample() {
        let mut app = ansi16_app();
        app.add_terminal_systems(
            TerminalRenderSystems::Composite,
            recolor_raw_red.in_set(CompositeSystems::PostProcess),
        );

        app.update();

        assert_eq!(frame_fg(&app), Color::LightGreen);
    }

    /// Recolors only if the composed cell still holds the raw rasterized RGB,
    /// so running before merge or after downsample leaves the frame red and
    /// fails the assertion.
    fn recolor_raw_red(mut frame: ResMut<FrameBuffer>) {
        let cell = frame.0.cell_mut((0, 0)).unwrap();
        if cell.fg == Color::Rgb(255, 0, 0) {
            cell.fg = Color::Rgb(0, 255, 0);
        }
    }

    #[test]
    fn truecolor_and_non_rgb_colors_pass_through() {
        let rgb = Color::Rgb(12, 34, 56);
        assert_eq!(downsample(rgb, ColorDepth::TrueColor), rgb);
        assert_eq!(
            downsample(Color::Magenta, ColorDepth::Ansi256),
            Color::Magenta
        );
        assert_eq!(
            downsample(Color::Indexed(42), ColorDepth::Ansi16),
            Color::Indexed(42)
        );
    }

    #[test]
    fn ansi256_maps_cube_corners_to_cube_indices() {
        assert_eq!(
            downsample(Color::Rgb(0, 0, 0), ColorDepth::Ansi256),
            Color::Indexed(16)
        );
        assert_eq!(
            downsample(Color::Rgb(255, 0, 0), ColorDepth::Ansi256),
            Color::Indexed(196)
        );
        assert_eq!(
            downsample(Color::Rgb(255, 255, 255), ColorDepth::Ansi256),
            Color::Indexed(231)
        );
    }

    #[test]
    fn ansi256_prefers_the_gray_ramp_for_grays() {
        assert_eq!(
            downsample(Color::Rgb(128, 128, 128), ColorDepth::Ansi256),
            Color::Indexed(244)
        );
        assert_eq!(
            downsample(Color::Rgb(8, 8, 8), ColorDepth::Ansi256),
            Color::Indexed(232)
        );
    }

    #[test]
    fn ansi16_picks_the_nearest_standard_color() {
        assert_eq!(
            downsample(Color::Rgb(200, 30, 30), ColorDepth::Ansi16),
            Color::LightRed
        );
        assert_eq!(
            downsample(Color::Rgb(0, 100, 0), ColorDepth::Ansi16),
            Color::Green
        );
        assert_eq!(
            downsample(Color::Rgb(250, 250, 250), ColorDepth::Ansi16),
            Color::White
        );
    }
}
