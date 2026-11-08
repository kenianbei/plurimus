use bevy_app::{App, AppExit, FixedUpdate, Update};
use bevy_ecs::prelude::{
    Commands, Component, Entity, MessageReader, MessageWriter, Query, Res, ResMut, Resource,
};
use bevy_math::Vec2;
use bevy_time::Time;
use bevy_transform::components::Transform;
use plurimus::render3d::{EdgeOverlay, EdgeSource, LuminanceRamp, RAMP_SHADING, Strategy3d};
use plurimus::term::HeldModifiers;
use plurimus::term::bevy_input::ButtonState;
use plurimus::term::bevy_input::keyboard::{Key, KeyboardInput};

const GRAVITY: f32 = 3.0;
const IMPULSE_UP: f32 = 1.4;
const IMPULSE_SIDE: f32 = 0.7;
pub const FUEL_START: f32 = 64.0;
const FUEL_PER_BURN: f32 = 2.0;
const SAFE_VERTICAL_SPEED: f32 = 2.5;
const SAFE_LATERAL_SPEED: f32 = 1.5;
const PAD_HALF_WIDTH: f32 = 3.5;
pub const PAD_CENTER_X: f32 = 0.0;
pub const GROUND_CLEARANCE: f32 = 0.0;
pub const START_POSITION: Vec2 = Vec2::new(-9.0, 20.0);

struct BaseBlock {
    min_x: f32,
    max_x: f32,
    top_y: f32,
}

// Model-local extents traced from moonbase.glb; world x adds PAD_CENTER_X.
const BASE_BLOCKS: &[BaseBlock] = &[
    BaseBlock {
        min_x: 5.2,
        max_x: 10.8,
        top_y: 3.05,
    },
    BaseBlock {
        min_x: 3.4,
        max_x: 15.0,
        top_y: 2.2,
    },
];

#[derive(Component, Default)]
pub struct Lander {
    pub velocity: Vec2,
}

#[derive(Resource)]
pub struct Fuel(pub f32);

#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    #[default]
    Flying,
    Landed,
    Crashed,
}

pub fn add_game(app: &mut App) {
    app.insert_resource(Fuel(FUEL_START));
    app.init_resource::<Phase>();
    app.add_systems(FixedUpdate, fall_and_touch_down);
    app.add_systems(Update, (quit_on_key, handle_keys, handle_render_keys));
    crate::hud::add_hud(app);
}

fn fall_and_touch_down(
    time: Res<Time>,
    mut phase: ResMut<Phase>,
    mut landers: Query<(&mut Lander, &mut Transform)>,
) {
    if *phase != Phase::Flying {
        return;
    }
    let Ok((mut lander, mut transform)) = landers.single_mut() else {
        return;
    };
    lander.velocity.y -= GRAVITY * time.delta_secs();
    transform.translation.x += lander.velocity.x * time.delta_secs();
    transform.translation.y += lander.velocity.y * time.delta_secs();
    if hits_base(transform.translation.x, transform.translation.y) {
        *phase = Phase::Crashed;
        lander.velocity = Vec2::ZERO;
        return;
    }
    if transform.translation.y <= GROUND_CLEARANCE {
        transform.translation.y = GROUND_CLEARANCE;
        *phase = touchdown_phase(&lander, transform.translation.x);
        lander.velocity = Vec2::ZERO;
    }
}

fn hits_base(x: f32, y: f32) -> bool {
    let local_x = x - PAD_CENTER_X;
    BASE_BLOCKS
        .iter()
        .any(|block| local_x >= block.min_x && local_x <= block.max_x && y <= block.top_y)
}

fn touchdown_phase(lander: &Lander, x: f32) -> Phase {
    let is_gentle = lander.velocity.y.abs() <= SAFE_VERTICAL_SPEED
        && lander.velocity.x.abs() <= SAFE_LATERAL_SPEED;
    if is_gentle && (x - PAD_CENTER_X).abs() <= PAD_HALF_WIDTH {
        Phase::Landed
    } else {
        Phase::Crashed
    }
}

fn quit_on_key(
    mut keys: MessageReader<KeyboardInput>,
    held: HeldModifiers,
    mut exit: MessageWriter<AppExit>,
) {
    for key in keys.read() {
        let ctrl_c = held.get().ctrl && key.logical_key == Key::Character("c".into());
        if key.state == ButtonState::Pressed
            && (key.logical_key == Key::Character("q".into()) || ctrl_c)
        {
            exit.write(AppExit::Success);
        }
    }
}

fn handle_keys(
    mut keys: MessageReader<KeyboardInput>,
    mut phase: ResMut<Phase>,
    mut fuel: ResMut<Fuel>,
    mut landers: Query<(&mut Lander, &mut Transform)>,
) {
    for key in keys.read() {
        if key.state == ButtonState::Released {
            continue;
        }
        if key.logical_key == Key::Character("r".into()) {
            reset(&mut phase, &mut fuel, &mut landers);
        } else {
            burn(&key.logical_key, &mut fuel, &mut landers, *phase);
        }
    }
}

fn handle_render_keys(
    mut keys: MessageReader<KeyboardInput>,
    mut cameras: Query<(Entity, &mut Strategy3d, Option<&EdgeOverlay>)>,
    mut commands: Commands,
) {
    for key in keys.read() {
        if key.state == ButtonState::Released {
            continue;
        }
        if key.logical_key == Key::Character("t".into()) {
            cycle_strategies(&mut cameras);
        } else if key.logical_key == Key::Character("e".into()) {
            cycle_edges(&cameras, &mut commands);
        }
    }
}

fn cycle_strategies(cameras: &mut Query<(Entity, &mut Strategy3d, Option<&EdgeOverlay>)>) {
    for (_, mut strategy, _) in cameras.iter_mut() {
        *strategy = next_strategy(*strategy);
    }
}

fn next_strategy(strategy: Strategy3d) -> Strategy3d {
    match strategy {
        Strategy3d::Halfblocks => Strategy3d::Luminance(LuminanceRamp::new(RAMP_SHADING)),
        Strategy3d::Luminance(ramp) if ramp.characters == RAMP_SHADING => {
            Strategy3d::Luminance(LuminanceRamp::default())
        }
        Strategy3d::Luminance(_) => Strategy3d::Braille,
        _ => Strategy3d::Halfblocks,
    }
}

// The cycle is the demo: gray-on-gray shading is exactly where
// luminance edges struggle and depth silhouettes do not.
fn cycle_edges(
    cameras: &Query<(Entity, &mut Strategy3d, Option<&EdgeOverlay>)>,
    commands: &mut Commands,
) {
    for (camera, _, overlay) in cameras.iter() {
        match next_edge_source(overlay.map(|overlay| overlay.source)) {
            Some(source) => {
                commands
                    .entity(camera)
                    .insert(EdgeOverlay::default().with_source(source));
            }
            None => {
                commands.entity(camera).remove::<EdgeOverlay>();
            }
        }
    }
}

const fn next_edge_source(source: Option<EdgeSource>) -> Option<EdgeSource> {
    match source {
        None => Some(EdgeSource::Luminance),
        Some(EdgeSource::Luminance) => Some(EdgeSource::Depth),
        Some(EdgeSource::Depth) => Some(EdgeSource::Both),
        Some(_) => None,
    }
}

fn burn(
    key: &Key,
    fuel: &mut Fuel,
    landers: &mut Query<(&mut Lander, &mut Transform)>,
    phase: Phase,
) {
    let typed = match key {
        Key::Character(c) => c.as_str(),
        Key::ArrowUp => "w",
        Key::ArrowLeft => "a",
        Key::ArrowRight => "d",
        _ => return,
    };
    let impulse = match typed {
        "w" | " " => Vec2::new(0.0, IMPULSE_UP),
        "a" => Vec2::new(-IMPULSE_SIDE, 0.0),
        "d" => Vec2::new(IMPULSE_SIDE, 0.0),
        _ => return,
    };
    if phase != Phase::Flying || fuel.0 <= 0.0 {
        return;
    }
    if let Ok((mut lander, _)) = landers.single_mut() {
        lander.velocity += impulse;
        fuel.0 = (fuel.0 - FUEL_PER_BURN).max(0.0);
    }
}

pub fn reset(
    phase: &mut Phase,
    fuel: &mut Fuel,
    landers: &mut Query<(&mut Lander, &mut Transform)>,
) {
    if let Ok((mut lander, mut transform)) = landers.single_mut() {
        lander.velocity = Vec2::ZERO;
        transform.translation.x = START_POSITION.x;
        transform.translation.y = START_POSITION.y;
        *phase = Phase::Flying;
        fuel.0 = FUEL_START;
    }
}
