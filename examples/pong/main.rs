//! Pong: the 2d pipeline composed with the ui pipeline in one camera.
//! W/S steps the left paddle one cell, Up/Down the right; the ball serves
//! itself after a short delay, `r` serves immediately, `q` or ctrl-c quits.

mod game;

use std::time::Duration;

use bevy_app::{App, AppExit, ScheduleRunnerPlugin};
use plurimus::core::CorePlugin;
use plurimus::crossterm::CrosstermPlugin;
use plurimus::render2d::Plugin2d;
use plurimus::widgets::WidgetsPlugin;

const FRAME_INTERVAL: Duration = Duration::from_millis(16);

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins((
        ScheduleRunnerPlugin::run_loop(FRAME_INTERVAL),
        CorePlugin,
        CrosstermPlugin::default(),
        WidgetsPlugin,
        Plugin2d,
    ));
    game::add_game(&mut app);
    app.run()
}

#[cfg(test)]
mod tests {
    use bevy_math::Vec2;
    use bevy_time::TimeUpdateStrategy;
    use bevy_transform::components::Transform;
    use plurimus::core::TerminalSize;
    use plurimus::input::KeyCode;
    use plurimus_test::write_key;

    use super::game::{Ball, CELL_HEIGHT, Paddle, Score, add_game};
    use super::*;

    const STEP: Duration = Duration::from_millis(50);

    fn headless_app() -> App {
        let mut app = App::new();
        app.add_plugins((CorePlugin, WidgetsPlugin, Plugin2d));
        app.insert_resource(TerminalSize { cols: 80, rows: 24 });
        app.insert_resource(TimeUpdateStrategy::ManualDuration(STEP));
        add_game(&mut app);
        app.update();
        app
    }

    fn ball_state(app: &mut App) -> (Vec2, Vec2) {
        let world = app.world_mut();
        let mut balls = world.query::<(&Ball, &Transform)>();
        let (ball, transform) = balls.single(world).unwrap();
        (ball.velocity, transform.translation.truncate())
    }

    #[test]
    fn serve_sets_the_ball_in_motion() {
        let mut app = headless_app();
        assert_eq!(ball_state(&mut app).0, Vec2::ZERO);

        write_key(&mut app, KeyCode::Char('r'));
        app.update();
        app.update();

        let (velocity, position) = ball_state(&mut app);
        assert!(velocity.length() > 0.0);
        assert!(position != Vec2::ZERO);
    }

    #[test]
    fn ball_bounces_off_the_top_wall() {
        let mut app = headless_app();
        let world = app.world_mut();
        let mut balls = world.query::<&mut Ball>();
        balls.single_mut(world).unwrap().velocity = Vec2::new(-4.0, 30.0);

        let bounced = (0..60).any(|_| {
            app.update();
            ball_state(&mut app).0.y < 0.0
        });

        assert!(bounced);
    }

    #[test]
    fn missed_ball_scores_for_the_opponent_and_resets() {
        let mut app = headless_app();
        write_key(&mut app, KeyCode::Char('r'));

        let scored = (0..80).any(|_| {
            app.update();
            app.world().resource::<Score>().right > 0
        });

        assert!(scored);
        assert_eq!(ball_state(&mut app), (Vec2::ZERO, Vec2::ZERO));
    }

    #[test]
    fn key_press_steps_the_left_paddle_one_cell() {
        let mut app = headless_app();
        write_key(&mut app, KeyCode::Char('w'));

        for _ in 0..4 {
            app.update();
        }

        let world = app.world_mut();
        let mut paddles = world.query::<(&Paddle, &Transform)>();
        let left = paddles
            .iter(world)
            .find(|(paddle, _)| paddle.direction_keys.0 == KeyCode::Char('w'))
            .unwrap();
        assert_eq!(left.1.translation.y, CELL_HEIGHT);
    }

    #[test]
    fn ball_serves_itself_after_the_delay() {
        let mut app = headless_app();

        let served = (0..40).any(|_| {
            app.update();
            ball_state(&mut app).0 != Vec2::ZERO
        });

        assert!(served);
    }
}
