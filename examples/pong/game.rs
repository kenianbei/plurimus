use bevy_app::{App, AppExit, FixedUpdate, Startup, Update};
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::prelude::{
    ChildOf, Commands, Component, IntoScheduleConfigs, MessageReader, MessageWriter, Query, Res,
    ResMut, Resource, With, Without, resource_exists,
};
use bevy_math::Vec2;
use bevy_time::{Time, Timer, TimerMode};
use bevy_transform::components::Transform;
use plurimus::core::bevy_camera::Camera2d;
use plurimus::core::ratatui_core::style::{Color, Style};
use plurimus::core::{CellSize, TerminalCamera};
use plurimus::render2d::{Glyph, Pixel};
use plurimus::term::HeldModifiers;
use plurimus::term::bevy_input::ButtonState;
use plurimus::term::bevy_input::keyboard::{Key, KeyboardInput};
use plurimus::ui::UiWidget;
use plurimus::widgets::ratatui_widgets::paragraph::Paragraph;

const COURT_HALF_WIDTH: f32 = 38.0;
const COURT_HALF_HEIGHT: f32 = 21.0;
const WALL_CELL_WIDTH: f32 = 1.0;
pub const CELL_HEIGHT: f32 = 2.0;
const PADDLE_X: f32 = 35.0;
const PADDLE_HALF_HEIGHT: f32 = 4.0;
const PADDLE_LIMIT: f32 = COURT_HALF_HEIGHT - PADDLE_HALF_HEIGHT - CELL_HEIGHT / 2.0;
const DEMO_PADDLE_SPEED: f32 = 24.0;
const DEMO_AIM: f32 = 0.5;
const BALL_SPEED: f32 = 32.0;
const MAX_DEFLECTION: f32 = 0.8;
const SERVE_RISE: f32 = 0.35;
const SERVE_DELAY_SECONDS: f32 = 1.0;
const WALL_GLYPH_COUNT: i32 = (2.0 * COURT_HALF_WIDTH / WALL_CELL_WIDTH) as i32;
const PADDLE_GLYPH_COUNT: i32 = (2.0 * PADDLE_HALF_HEIGHT / CELL_HEIGHT) as i32;

#[derive(Component)]
pub struct Paddle {
    pub direction_keys: (Key, Key),
}

#[derive(Component, Default)]
pub struct Ball {
    pub velocity: Vec2,
}

#[derive(Resource, Default)]
pub struct Score {
    pub left: u32,
    pub right: u32,
}

#[derive(Component)]
struct ScoreLine;

/// Present when both paddles play themselves, for recording.
#[derive(Resource)]
pub struct Demo;

#[derive(Resource)]
struct ServeDelay(Timer);

impl Default for ServeDelay {
    fn default() -> Self {
        Self(Timer::from_seconds(SERVE_DELAY_SECONDS, TimerMode::Once))
    }
}

pub fn add_game(app: &mut App) {
    app.insert_resource(CellSize {
        x: WALL_CELL_WIDTH,
        y: CELL_HEIGHT,
    });
    app.init_resource::<Score>();
    app.init_resource::<ServeDelay>();
    app.add_systems(Startup, spawn_scene);
    app.add_systems(
        FixedUpdate,
        (
            auto_serve,
            track_ball.run_if(resource_exists::<Demo>),
            move_ball,
        ),
    );
    app.add_systems(Update, (move_paddles, handle_keys, update_score_line));
}

fn spawn_scene(mut commands: Commands) {
    commands.spawn((TerminalCamera::default(), Camera2d));
    commands.spawn((UiWidget::new(Paragraph::new("0 : 0").centered()), ScoreLine));
    spawn_court(&mut commands);
    spawn_paddle(
        &mut commands,
        -PADDLE_X,
        (Key::Character("w".into()), Key::Character("s".into())),
    );
    spawn_paddle(&mut commands, PADDLE_X, (Key::ArrowUp, Key::ArrowDown));
    commands.spawn((
        Ball::default(),
        Pixel::new(Color::White),
        Transform::default(),
    ));
}

fn spawn_court(commands: &mut Commands) {
    let wall_style = Style::new().fg(Color::DarkGray);
    for step in 0..=WALL_GLYPH_COUNT {
        let column = -COURT_HALF_WIDTH + step as f32 * WALL_CELL_WIDTH;
        for row in [-COURT_HALF_HEIGHT, COURT_HALF_HEIGHT] {
            commands.spawn((
                Glyph::new("─").with_style(wall_style),
                Transform::from_xyz(column, row, 0.0),
            ));
        }
    }
}

fn spawn_paddle(commands: &mut Commands, x: f32, direction_keys: (Key, Key)) {
    let root = commands
        .spawn((Paddle { direction_keys }, Transform::from_xyz(x, 0.0, 0.0)))
        .id();
    let style = Style::new().fg(Color::Cyan);
    for step in 0..PADDLE_GLYPH_COUNT {
        let offset = -PADDLE_HALF_HEIGHT + CELL_HEIGHT / 2.0 + step as f32 * CELL_HEIGHT;
        commands.spawn((
            Glyph::new("█").with_style(style),
            Transform::from_xyz(0.0, offset, 1.0),
            ChildOf(root),
        ));
    }
}

fn move_paddles(
    mut keys: MessageReader<KeyboardInput>,
    mut paddles: Query<(&Paddle, &mut Transform)>,
) {
    for key in keys.read() {
        if key.state == ButtonState::Released {
            continue;
        }
        for (paddle, mut transform) in &mut paddles {
            let (up, down) = &paddle.direction_keys;
            let step = if key.logical_key == *up {
                CELL_HEIGHT
            } else if key.logical_key == *down {
                -CELL_HEIGHT
            } else {
                continue;
            };
            transform.translation.y =
                (transform.translation.y + step).clamp(-PADDLE_LIMIT, PADDLE_LIMIT);
        }
    }
}

// Leading the ball by part of a paddle keeps contact off-centre, so no rally
// flattens into a horizontal line.
fn track_ball(
    time: Res<Time>,
    balls: Query<(&Ball, &Transform)>,
    mut paddles: Query<&mut Transform, (With<Paddle>, Without<Ball>)>,
) {
    let Ok((ball, ball_transform)) = balls.single() else {
        return;
    };
    let lead = DEMO_AIM * PADDLE_HALF_HEIGHT * ball.velocity.y.signum();
    let intercept = (ball_transform.translation.y - lead).clamp(-PADDLE_LIMIT, PADDLE_LIMIT);
    let max_step = DEMO_PADDLE_SPEED * time.delta_secs();
    for mut transform in &mut paddles {
        let is_incoming = ball.velocity.x * transform.translation.x > 0.0;
        let target = if is_incoming { intercept } else { 0.0 };
        let y = transform.translation.y;
        transform.translation.y = y + (target - y).clamp(-max_step, max_step);
    }
}

fn auto_serve(
    time: Res<Time>,
    mut delay: ResMut<ServeDelay>,
    score: Res<Score>,
    mut balls: Query<&mut Ball>,
) {
    let Ok(mut ball) = balls.single_mut() else {
        return;
    };
    if ball.velocity != Vec2::ZERO {
        return;
    }
    if delay.0.tick(time.delta()).just_finished() {
        serve(&score, &mut ball);
        delay.0.reset();
    }
}

fn move_ball(
    time: Res<Time>,
    mut score: ResMut<Score>,
    mut balls: Query<(&mut Ball, &mut Transform)>,
    paddles: Query<&Transform, (With<Paddle>, Without<Ball>)>,
) {
    let Ok((mut ball, mut transform)) = balls.single_mut() else {
        return;
    };
    let position = transform.translation.truncate() + ball.velocity * time.delta_secs();
    transform.translation.x = position.x;
    transform.translation.y = position.y;
    bounce_off_walls(&mut ball, position);
    for paddle in &paddles {
        deflect_off_paddle(&mut ball, position, paddle.translation.truncate());
    }
    if position.x.abs() > COURT_HALF_WIDTH {
        award_point(&mut score, position.x);
        transform.translation.x = 0.0;
        transform.translation.y = 0.0;
        ball.velocity = Vec2::ZERO;
    }
}

fn bounce_off_walls(ball: &mut Ball, position: Vec2) {
    let limit = COURT_HALF_HEIGHT - CELL_HEIGHT / 2.0;
    if position.y >= limit && ball.velocity.y > 0.0 {
        ball.velocity.y = -ball.velocity.y;
    }
    if position.y <= -limit && ball.velocity.y < 0.0 {
        ball.velocity.y = -ball.velocity.y;
    }
}

fn deflect_off_paddle(ball: &mut Ball, position: Vec2, paddle: Vec2) {
    let moving_toward = (position.x - paddle.x) * ball.velocity.x < 0.0;
    let within_reach = (position.x - paddle.x).abs() <= WALL_CELL_WIDTH
        && (position.y - paddle.y).abs() <= PADDLE_HALF_HEIGHT;
    if !(moving_toward && within_reach) {
        return;
    }
    let offset = (position.y - paddle.y) / PADDLE_HALF_HEIGHT;
    let direction = Vec2::new(-ball.velocity.x.signum(), offset * MAX_DEFLECTION).normalize();
    ball.velocity = direction * BALL_SPEED;
}

fn award_point(score: &mut Score, ball_x: f32) {
    if ball_x > 0.0 {
        score.left += 1;
    } else {
        score.right += 1;
    }
}

fn handle_keys(
    mut keys: MessageReader<KeyboardInput>,
    held: HeldModifiers,
    score: Res<Score>,
    mut balls: Query<&mut Ball>,
    mut exit: MessageWriter<AppExit>,
) {
    for key in keys.read() {
        if key.state != ButtonState::Pressed || key.repeat {
            continue;
        }
        let ctrl_c = held.get().ctrl && key.logical_key == Key::Character("c".into());
        if key.logical_key == Key::Character("q".into()) || ctrl_c {
            exit.write(AppExit::Success);
        }
        if key.logical_key == Key::Character("r".into())
            && let Ok(mut ball) = balls.single_mut()
        {
            serve(&score, &mut ball);
        }
    }
}

fn serve(score: &Score, ball: &mut Ball) {
    if ball.velocity != Vec2::ZERO {
        return;
    }
    let toward_left = (score.left + score.right).is_multiple_of(2);
    let x = if toward_left { -1.0 } else { 1.0 };
    ball.velocity = Vec2::new(x, SERVE_RISE).normalize() * BALL_SPEED;
}

fn update_score_line(score: Res<Score>, mut lines: Query<&mut UiWidget, With<ScoreLine>>) {
    if !score.is_changed() {
        return;
    }
    for mut line in &mut lines {
        let text = format!("{} : {}", score.left, score.right);
        *line = UiWidget::new(Paragraph::new(text).centered());
    }
}
