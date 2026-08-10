//! Lunar lander: the 3d pipeline composed with the ui HUD. W/space burns
//! the main thruster, a/d tilt, land gently on the pad. `t` cycles the
//! pixel-to-cell strategy (halfblocks, shading and ascii luminance,
//! braille), `e` cycles the sobel edge overlay (off, luminance, depth,
//! both), `r` resets, `q` or
//! ctrl-c quits. The first frames take a few seconds while GPU pipelines
//! compile.

mod effects;
mod game;
mod hud;
mod scene;

use std::time::Duration;

use bevy_app::{App, AppExit, ScheduleRunnerPlugin, Startup};
use bevy_asset::AssetPlugin;
use bevy_gltf::GltfPlugin;
use bevy_gltf::extensions::GltfExtensionHandlers;
use bevy_pbr::PbrPlugin;
use bevy_world_serialization::WorldSerializationPlugin;
use plurimus::core::CorePlugin;
use plurimus::crossterm::CrosstermPlugin;
use plurimus::render3d::{Plugin3d, Render3dPlugins};
use plurimus::widgets::WidgetsPlugin;

const FRAME_INTERVAL: Duration = Duration::from_millis(16);
const ASSET_ROOT: &str = "examples/lander/assets";

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins((
        ScheduleRunnerPlugin::run_loop(FRAME_INTERVAL),
        CorePlugin,
        CrosstermPlugin::default(),
        WidgetsPlugin,
        AssetPlugin {
            file_path: ASSET_ROOT.into(),
            ..AssetPlugin::default()
        },
        Render3dPlugins,
    ));
    // The material system and glTF loading are the app's to assemble:
    // PbrPlugin's build registers a glTF material handler, so the
    // registry has to exist before it, and GltfPlugin's finish reads
    // RenderPlugin's, so it comes after the render stack.
    app.init_resource::<GltfExtensionHandlers>();
    app.add_plugins(PbrPlugin::default());
    app.add_plugins((WorldSerializationPlugin, GltfPlugin::default()));
    app.add_plugins(Plugin3d);
    game::add_game(&mut app);
    effects::add_effects(&mut app);
    app.add_systems(Startup, scene::spawn_scene);
    app.run()
}

#[cfg(test)]
mod tests {
    use bevy_ecs::prelude::With;
    use bevy_math::Vec2;
    use bevy_time::TimeUpdateStrategy;
    use bevy_transform::components::Transform;
    use plurimus::core::ratatui_core::style::Color;
    use plurimus::core::{Background, TerminalCamera, TerminalSize, Viewport};
    use plurimus::render3d::{EdgeOverlay, RAMP_ASCII, RAMP_SHADING, Strategy3d};
    use plurimus::term::KeyCode;
    use plurimus::ui::ComputedWidgetArea;
    use plurimus::widgets::Button;
    use plurimus_test::{click, composed_styled_frame, write_key};

    use super::game::{
        FUEL_START, Fuel, GROUND_CLEARANCE, Lander, PAD_CENTER_X, Phase, START_POSITION, add_game,
    };
    use super::*;

    const STEP: Duration = Duration::from_millis(50);

    fn headless_app() -> App {
        let mut app = App::new();
        app.add_plugins((CorePlugin, WidgetsPlugin));
        app.insert_resource(TerminalSize { cols: 80, rows: 24 });
        app.insert_resource(TimeUpdateStrategy::ManualDuration(STEP));
        add_game(&mut app);
        app.world_mut().spawn((
            Lander::default(),
            Transform::from_translation(START_POSITION.extend(0.0)),
        ));
        app.update();
        app
    }

    fn lander_state(app: &mut App) -> (Vec2, f32) {
        let world = app.world_mut();
        let mut landers = world.query::<(&Lander, &Transform)>();
        let (lander, transform) = landers.single(world).unwrap();
        (lander.velocity, transform.translation.y)
    }

    fn place_lander(app: &mut App, position: Vec2, velocity: Vec2) {
        let world = app.world_mut();
        let mut landers = world.query::<(&mut Lander, &mut Transform)>();
        let (mut lander, mut transform) = landers.single_mut(world).unwrap();
        transform.translation.x = position.x;
        transform.translation.y = position.y;
        lander.velocity = velocity;
    }

    fn restart_button_center(app: &mut App) -> (u16, u16) {
        let world = app.world_mut();
        let mut buttons = world.query_filtered::<&ComputedWidgetArea, With<Button>>();
        let area = buttons.single(world).unwrap().0;
        (area.x + area.width / 2, area.y + area.height / 2)
    }

    #[test]
    fn panel_renders_uncamouflaged_over_the_scene() {
        let mut app = headless_app();
        app.world_mut().spawn(TerminalCamera {
            viewport: Viewport::Fill,
            background: Background::Clear(Color::Rgb(43, 44, 47)),
            ..TerminalCamera::default()
        });

        app.update();
        app.update();

        let frame = composed_styled_frame(&app);
        let (symbols, styles) = frame.split_once("\n--\n").unwrap();
        let body_row = symbols.lines().nth(1).unwrap();
        assert!(body_row.contains("FUEL"), "panel body row: {body_row}");
        let body_column = body_row.chars().position(|symbol| symbol == 'F').unwrap();
        let row_styles = styles.lines().nth(1).unwrap();
        let panel_style = row_styles.chars().nth(body_column).unwrap();
        let scene_style = row_styles.chars().next().unwrap();
        assert_ne!(
            panel_style, scene_style,
            "panel camouflaged into the scene:\n{frame}"
        );
        assert!(
            frame.contains(&format!("{panel_style}: fg:Some(Reset) bg:Some(Reset)")),
            "panel style not terminal-default:\n{frame}"
        );
    }

    #[test]
    fn gravity_pulls_the_lander_down() {
        let mut app = headless_app();
        let before = lander_state(&mut app).1;

        for _ in 0..5 {
            app.update();
        }

        let (velocity, altitude) = lander_state(&mut app);
        assert!(altitude < before);
        assert!(velocity.y < 0.0);
    }

    #[test]
    fn thrust_burns_fuel_and_pushes_up() {
        let mut app = headless_app();
        write_key(&mut app, KeyCode::Char('w'));

        app.update();

        let (velocity, _) = lander_state(&mut app);
        assert!(velocity.y > 0.0);
        assert!(app.world().resource::<Fuel>().0 < FUEL_START);
    }

    #[test]
    fn hard_free_fall_crashes() {
        let mut app = headless_app();

        let crashed = (0..200).any(|_| {
            app.update();
            *app.world().resource::<Phase>() == Phase::Crashed
        });

        assert!(crashed);
    }

    #[test]
    fn gentle_pad_touchdown_lands() {
        let mut app = headless_app();
        place_lander(
            &mut app,
            Vec2::new(PAD_CENTER_X, GROUND_CLEARANCE + 0.35),
            Vec2::new(0.0, -0.5),
        );

        for _ in 0..10 {
            app.update();
        }

        assert_eq!(*app.world().resource::<Phase>(), Phase::Landed);
    }

    #[test]
    fn falling_onto_the_base_crashes_midair() {
        let mut app = headless_app();
        place_lander(&mut app, Vec2::new(12.0, 3.5), Vec2::new(0.0, -1.0));

        for _ in 0..20 {
            app.update();
        }

        assert_eq!(*app.world().resource::<Phase>(), Phase::Crashed);
        assert!(lander_state(&mut app).1 > 1.0, "should stop on the dome");
    }

    #[test]
    fn flying_above_the_base_stays_flying() {
        let mut app = headless_app();
        place_lander(&mut app, Vec2::new(9.0, 8.0), Vec2::new(3.0, 0.0));

        for _ in 0..5 {
            app.update();
        }

        assert_eq!(*app.world().resource::<Phase>(), Phase::Flying);
    }

    #[test]
    fn reset_restores_flight() {
        let mut app = headless_app();
        place_lander(&mut app, Vec2::new(START_POSITION.x, 0.0), Vec2::ZERO);
        for _ in 0..3 {
            app.update();
        }
        assert_ne!(*app.world().resource::<Phase>(), Phase::Flying);

        write_key(&mut app, KeyCode::Char('r'));
        app.update();

        assert_eq!(*app.world().resource::<Phase>(), Phase::Flying);
        assert_eq!(app.world().resource::<Fuel>().0, FUEL_START);
        assert!(lander_state(&mut app).1 > 10.0);
    }

    #[test]
    fn clicking_restart_restores_flight() {
        let mut app = headless_app();
        place_lander(&mut app, Vec2::new(START_POSITION.x, 0.0), Vec2::ZERO);
        for _ in 0..3 {
            app.update();
        }
        assert_ne!(*app.world().resource::<Phase>(), Phase::Flying);

        let (x, y) = restart_button_center(&mut app);
        click(&mut app, x, y);

        assert_eq!(*app.world().resource::<Phase>(), Phase::Flying);
        assert_eq!(app.world().resource::<Fuel>().0, FUEL_START);
        assert!(lander_state(&mut app).1 > 10.0);
    }

    #[test]
    fn strategy_cycles_through_all_four_modes() {
        let mut app = headless_app();
        let camera = app.world_mut().spawn(Strategy3d::default()).id();

        let mut seen = Vec::new();
        for _ in 0..4 {
            write_key(&mut app, KeyCode::Char('t'));
            app.update();
            seen.push(*app.world().entity(camera).get::<Strategy3d>().unwrap());
        }

        assert!(matches!(seen[0], Strategy3d::Luminance(ramp) if ramp.characters == RAMP_SHADING));
        assert!(matches!(seen[1], Strategy3d::Luminance(ramp) if ramp.characters == RAMP_ASCII));
        assert_eq!(seen[2], Strategy3d::Braille);
        assert_eq!(seen[3], Strategy3d::Halfblocks);
    }

    #[test]
    fn edge_overlay_cycles_through_its_sources() {
        use plurimus::render3d::EdgeSource;

        let mut app = headless_app();
        let camera = app.world_mut().spawn(Strategy3d::default()).id();

        let mut seen = Vec::new();
        for _ in 0..4 {
            write_key(&mut app, KeyCode::Char('e'));
            app.update();
            seen.push(
                app.world()
                    .entity(camera)
                    .get::<EdgeOverlay>()
                    .map(|overlay| overlay.source),
            );
        }

        assert_eq!(
            seen,
            [
                Some(EdgeSource::Luminance),
                Some(EdgeSource::Depth),
                Some(EdgeSource::Both),
                None,
            ]
        );
    }
}
