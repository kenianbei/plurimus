use bevy_app::{App, AppExit, FixedUpdate, Startup, Update};
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::prelude::{
    ChildOf, Commands, Component, MessageReader, MessageWriter, Query, Res, ResMut, Resource, With,
    Without,
};
use bevy_math::Vec2;
use bevy_time::{Time, Timer, TimerMode};
use bevy_transform::components::Transform;
use plurimus::core::TerminalCamera;
use plurimus::core::ratatui_core::style::{Color, Style};
use plurimus::render2d::{Glyph, Pixel, Projection2d};
use plurimus::term::{KeyCode, KeyKind, KeyMessage};
use plurimus::ui::UiWidget;
use plurimus::widgets::ratatui_widgets::paragraph::Paragraph;

const COURT_HALF_WIDTH: f32 = 38.0;
const COURT_HALF_HEIGHT: f32 = 21.0;
const WALL_CELL_WIDTH: f32 = 1.0;
pub const CELL_HEIGHT: f32 = 2.0;
const PADDLE_X: f32 = 35.0;
const PADDLE_HALF_HEIGHT: f32 = 4.0;
const BALL_SPEED: f32 = 32.0;
const MAX_DEFLECTION: f32 = 0.8;
const SERVE_RISE: f32 = 0.35;
const SERVE_DELAY_SECONDS: f32 = 1.0;
const WALL_GLYPH_COUNT: i32 = (2.0 * COURT_HALF_WIDTH / WALL_CELL_WIDTH) as i32;
const PADDLE_GLYPH_COUNT: i32 = (2.0 * PADDLE_HALF_HEIGHT / CELL_HEIGHT) as i32;

#[derive(Component)]
pub struct Paddle {
    pub direction_keys: (KeyCode, KeyCode),
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

#[derive(Resource)]
struct ServeDelay(Timer);

impl Default for ServeDelay {
    fn default() -> Self {
        Self(Timer::from_seconds(SERVE_DELAY_SECONDS, TimerMode::Once))
    }
}

pub fn add_game(app: &mut App) {
    app.init_resource::<Score>();
    app.init_resource::<ServeDelay>();
    app.add_systems(Startup, spawn_scene);
    app.add_systems(FixedUpdate, (auto_serve, move_ball));
    app.add_systems(Update, (move_paddles, handle_keys, update_score_line));
}

fn spawn_scene(mut commands: Commands) {
    commands.spawn((TerminalCamera::default(), Projection2d::default()));
    commands.spawn((UiWidget::new(Paragraph::new("0 : 0").centered()), ScoreLine));
    spawn_court(&mut commands);
    spawn_paddle(
        &mut commands,
        -PADDLE_X,
        (KeyCode::Char('w'), KeyCode::Char('s')),
    );
    spawn_paddle(&mut commands, PADDLE_X, (KeyCode::Up, KeyCode::Down));
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
                Glyph::new("─").style(wall_style),
                Transform::from_xyz(column, row, 0.0),
            ));
        }
    }
}

fn spawn_paddle(commands: &mut Commands, x: f32, direction_keys: (KeyCode, KeyCode)) {
    let root = commands
        .spawn((Paddle { direction_keys }, Transform::from_xyz(x, 0.0, 0.0)))
        .id();
    let style = Style::new().fg(Color::Cyan);
    for step in 0..PADDLE_GLYPH_COUNT {
        let offset = -PADDLE_HALF_HEIGHT + CELL_HEIGHT / 2.0 + step as f32 * CELL_HEIGHT;
        commands.spawn((
            Glyph::new("█").style(style),
            Transform::from_xyz(0.0, offset, 1.0),
            ChildOf(root),
        ));
    }
}

fn move_paddles(
    mut keys: MessageReader<KeyMessage>,
    mut paddles: Query<(&Paddle, &mut Transform)>,
) {
    for key in keys.read() {
        if key.kind == KeyKind::Release {
            continue;
        }
        for (paddle, mut transform) in &mut paddles {
            let (up, down) = paddle.direction_keys;
            let step = if key.code == up {
                CELL_HEIGHT
            } else if key.code == down {
                -CELL_HEIGHT
            } else {
                continue;
            };
            let limit = COURT_HALF_HEIGHT - PADDLE_HALF_HEIGHT - CELL_HEIGHT / 2.0;
            transform.translation.y = (transform.translation.y + step).clamp(-limit, limit);
        }
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
    mut keys: MessageReader<KeyMessage>,
    score: Res<Score>,
    mut balls: Query<&mut Ball>,
    mut exit: MessageWriter<AppExit>,
) {
    for key in keys.read() {
        if key.kind != KeyKind::Press {
            continue;
        }
        let ctrl_c = key.modifiers.ctrl && key.code == KeyCode::Char('c');
        if key.code == KeyCode::Char('q') || ctrl_c {
            exit.write(AppExit::Success);
        }
        if key.code == KeyCode::Char('r')
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
