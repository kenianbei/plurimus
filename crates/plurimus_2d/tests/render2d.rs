//! Snapshot tests over composed 2d frames.

use bevy_app::App;
use bevy_transform::components::Transform;
use plurimus_2d::{
    Glyph, GlyphBlock, Pixel, PixelBlock, Plugin2d, Projection2d, RenderLayers, SubcellMode,
};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::{Color, Style};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize, Viewport};
use plurimus_test::{composed_frame, composed_styled_frame};

fn app(cols: u16, rows: u16) -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, Plugin2d));
    app.insert_resource(TerminalSize::new(cols, rows));
    app
}

fn glyph_at(symbol: &str, x: f32, y: f32, z: f32) -> (Glyph, Transform) {
    (Glyph::new(symbol), Transform::from_xyz(x, y, z))
}

#[test]
fn glyphs_project_around_an_offset_camera() {
    let mut app = app(9, 5);
    app.world_mut().spawn((
        TerminalCamera::default(),
        Projection2d::default(),
        Transform::from_xyz(10.0, 20.0, 0.0),
    ));
    app.world_mut().spawn(glyph_at("@", 10.0, 20.0, 0.0));
    app.world_mut().spawn(glyph_at("#", 12.0, 22.0, 0.0));
    app.world_mut().spawn(glyph_at("~", 7.0, 19.0, 0.0));

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn higher_z_glyph_wins_the_cell() {
    let mut app = app(5, 3);
    app.world_mut()
        .spawn((TerminalCamera::default(), Projection2d::default()));
    app.world_mut().spawn(glyph_at("b", 0.0, 0.0, 2.0));
    app.world_mut().spawn(glyph_at("a", 0.0, 0.0, 1.0));

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn pixels_land_in_upper_and_lower_halves() {
    let mut app = app(5, 2);
    app.world_mut()
        .spawn((TerminalCamera::default(), Projection2d::default()));
    app.world_mut()
        .spawn((Pixel::new(Color::Red), Transform::from_xyz(-2.0, 1.5, 0.0)));
    app.world_mut()
        .spawn((Pixel::new(Color::Blue), Transform::from_xyz(0.0, 0.5, 0.0)));
    app.world_mut()
        .spawn((Pixel::new(Color::Green), Transform::from_xyz(1.0, 0.0, 0.0)));

    app.update();

    insta::assert_snapshot!(composed_styled_frame(&app));
}

#[test]
fn two_cameras_view_the_world_at_different_scales() {
    let mut app = app(12, 4);
    app.world_mut().spawn((
        TerminalCamera::default().with_viewport(Viewport::Fixed(Rect::new(0, 0, 8, 4))),
        Projection2d::default(),
    ));
    app.world_mut().spawn((
        TerminalCamera::default()
            .with_order(1)
            .with_viewport(Viewport::Fixed(Rect::new(8, 0, 4, 4))),
        Projection2d::default().with_scale(2.0),
    ));
    app.world_mut().spawn(glyph_at("@", 2.0, 2.0, 0.0));
    app.world_mut().spawn(glyph_at("#", -2.0, -2.0, 0.0));

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn render_layers_mask_entities_per_camera() {
    let mut app = app(8, 3);
    app.world_mut().spawn((
        TerminalCamera::default().with_viewport(Viewport::Fixed(Rect::new(0, 0, 4, 3))),
        Projection2d::default(),
    ));
    app.world_mut().spawn((
        TerminalCamera::default()
            .with_order(1)
            .with_viewport(Viewport::Fixed(Rect::new(4, 0, 4, 3))),
        Projection2d::default(),
        RenderLayers::layer(1),
    ));
    app.world_mut()
        .spawn(glyph_at("@", 0.0, 0.0, 0.0))
        .insert(RenderLayers::layer(0).with(1));
    app.world_mut().spawn(glyph_at("#", -1.0, 0.0, 0.0));
    app.world_mut()
        .spawn((Pixel::new(Color::Red), Transform::from_xyz(1.0, 0.0, 0.0)));
    app.world_mut()
        .spawn(glyph_at("!", 1.0, 1.0, 0.0))
        .insert(RenderLayers::none());

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn glyphs_draw_over_pixels() {
    let mut app = app(3, 1);
    app.world_mut()
        .spawn((TerminalCamera::default(), Projection2d::default()));
    app.world_mut()
        .spawn((Pixel::new(Color::Red), Transform::from_xyz(0.0, 0.0, 5.0)));
    app.world_mut().spawn((
        Glyph::new("X").with_style(Style::new().fg(Color::White)),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    app.update();

    insta::assert_snapshot!(composed_styled_frame(&app));
}

#[test]
fn glyph_blocks_stamp_centered_with_transparent_holes() {
    let mut app = app(7, 5);
    app.world_mut()
        .spawn((TerminalCamera::default(), Projection2d::default()));
    app.world_mut().spawn(glyph_at(".", 0.0, 0.0, 0.0));
    app.world_mut().spawn((
        GlyphBlock::new("/-\\\n| |\n\\-/"),
        Transform::from_xyz(0.0, 0.0, 1.0),
    ));

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn glyph_blocks_clip_at_the_viewport_edge() {
    let mut app = app(5, 3);
    app.world_mut()
        .spawn((TerminalCamera::default(), Projection2d::default()));
    app.world_mut().spawn((
        GlyphBlock::new("abc\ndef\nghi"),
        Transform::from_xyz(-2.0, 2.0, 0.0),
    ));

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn wide_graphemes_occupy_their_display_width() {
    let mut app = app(7, 3);
    app.world_mut()
        .spawn((TerminalCamera::default(), Projection2d::default()));
    for x in 0..7 {
        app.world_mut()
            .spawn(glyph_at(".", x as f32 - 3.0, 0.0, 0.0));
    }
    app.world_mut()
        .spawn((GlyphBlock::new("苹果"), Transform::from_xyz(0.0, 0.0, 1.0)));

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn braille_cameras_render_pixels_at_dot_resolution() {
    let mut app = app(8, 2);
    app.world_mut().spawn((
        TerminalCamera::default().with_viewport(Viewport::Fixed(Rect::new(0, 0, 4, 2))),
        Projection2d::default(),
    ));
    app.world_mut().spawn((
        TerminalCamera::default()
            .with_order(1)
            .with_viewport(Viewport::Fixed(Rect::new(4, 0, 4, 2))),
        Projection2d::default(),
        SubcellMode::Braille,
    ));
    for step in 0..8 {
        let along = step as f32 / 2.0;
        app.world_mut().spawn((
            Pixel::new(Color::Red),
            Transform::from_xyz(along - 2.0, along - 2.0, 0.0),
        ));
    }

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn child_glyphs_follow_their_parent_transform() {
    let mut app = app(5, 3);
    app.world_mut()
        .spawn((TerminalCamera::default(), Projection2d::default()));
    let paddle = app
        .world_mut()
        .spawn(Transform::from_xyz(-1.0, 0.0, 0.0))
        .id();
    for offset in [-2.0, 0.0, 2.0] {
        let segment = app
            .world_mut()
            .spawn((Glyph::new("|"), Transform::from_xyz(0.0, offset, 0.0)))
            .id();
        app.world_mut()
            .entity_mut(segment)
            .insert(bevy_ecs::hierarchy::ChildOf(paddle));
    }

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

const BRICK: [(char, Color); 2] = [('r', Color::Red), ('b', Color::Blue)];

#[test]
fn pixel_blocks_stamp_centered_with_transparent_holes() {
    let mut app = app(5, 2);
    app.world_mut()
        .spawn((TerminalCamera::default(), Projection2d::default()));
    app.world_mut().spawn((
        PixelBlock::new("rbr\nb b\nrbr", BRICK),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    app.update();

    insta::assert_snapshot!(composed_styled_frame(&app));
}

#[test]
fn pixel_blocks_mirror_horizontally() {
    let mut app = app(5, 2);
    app.world_mut()
        .spawn((TerminalCamera::default(), Projection2d::default()));
    app.world_mut().spawn((
        PixelBlock::new("rb\nrb", BRICK).with_mirrored(true),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    app.update();

    insta::assert_snapshot!(composed_styled_frame(&app));
}

#[test]
fn ragged_pixel_block_rows_pad_to_the_widest() {
    let mut app = app(5, 2);
    app.world_mut()
        .spawn((TerminalCamera::default(), Projection2d::default()));
    app.world_mut().spawn((
        PixelBlock::new("rrrr\nb", BRICK),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    app.update();

    insta::assert_snapshot!(composed_styled_frame(&app));
}

#[test]
fn pixel_blocks_clip_at_the_viewport_edge() {
    let mut app = app(4, 2);
    app.world_mut()
        .spawn((TerminalCamera::default(), Projection2d::default()));
    app.world_mut().spawn((
        PixelBlock::new("rrrr\nrrrr\nrrrr\nrrrr", BRICK),
        Transform::from_xyz(-3.0, 2.0, 0.0),
    ));

    app.update();

    insta::assert_snapshot!(composed_styled_frame(&app));
}

#[test]
fn higher_z_pixel_wins_the_subcell() {
    let mut app = app(3, 1);
    app.world_mut()
        .spawn((TerminalCamera::default(), Projection2d::default()));
    app.world_mut()
        .spawn((Pixel::new(Color::Blue), Transform::from_xyz(0.0, 0.0, 2.0)));
    app.world_mut()
        .spawn((Pixel::new(Color::Red), Transform::from_xyz(0.0, 0.0, 1.0)));

    app.update();

    insta::assert_snapshot!(composed_styled_frame(&app));
}

#[test]
fn pixels_and_pixel_blocks_share_one_z_order() {
    let mut app = app(3, 1);
    app.world_mut()
        .spawn((TerminalCamera::default(), Projection2d::default()));
    app.world_mut().spawn((
        Pixel::new(Color::Green),
        Transform::from_xyz(-1.0, 0.5, 2.0),
    ));
    app.world_mut()
        .spawn((Pixel::new(Color::Green), Transform::from_xyz(0.0, 0.5, 0.0)));
    app.world_mut().spawn((
        PixelBlock::new("rrr", BRICK),
        Transform::from_xyz(0.0, 0.5, 1.0),
    ));

    app.update();

    insta::assert_snapshot!(composed_styled_frame(&app));
}

#[test]
fn braille_cameras_stamp_pixel_blocks_at_dot_resolution() {
    let mut app = app(4, 1);
    app.world_mut().spawn((
        TerminalCamera::default(),
        Projection2d::default(),
        SubcellMode::Braille,
    ));
    app.world_mut().spawn((
        PixelBlock::new("rrrr\nr  r\nr  r\nrrrr", BRICK),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}
