use std::time::Duration;

use bevy_animation::RepeatAnimation;
use bevy_app::PluginsState;
use bevy_asset::Assets;
use bevy_ecs::prelude::{Entity, With, World};
use bevy_render::RenderApp;
use bevy_render::render_resource::PipelineCache;
use bevy_time::{TimePlugin, TimeUpdateStrategy};
use bevy_world_serialization::WorldAsset;
use plurimus::core::TerminalSize;
use plurimus::render3d::{EdgeOverlay, EdgeSource, RAMP_ASCII, RAMP_SHADING};
use plurimus::term::TermPlugin;
use plurimus_test::{character, composed_styled_frame, write_key};

use super::*;

const STEP: Duration = Duration::from_millis(50);

fn keys_app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, TermPlugin));
    app.insert_resource(TerminalSize::new(80, 24));
    render_keys::add_render_keys(&mut app);
    app.update();
    app
}

#[test]
fn ready_scene_starts_the_run_repeating() {
    let mut app = App::new();
    app.add_plugins((
        TimePlugin,
        AssetPlugin::default(),
        WorldSerializationPlugin,
        AnimationPlugin,
    ));
    let mut fox = World::new();
    fox.spawn(AnimationPlayer::default());
    let scene = app
        .world_mut()
        .resource_mut::<Assets<WorldAsset>>()
        .add(WorldAsset::new(fox));
    let (graph, index) = AnimationGraph::from_clip(Handle::default());
    let graph_handle = app
        .world_mut()
        .resource_mut::<Assets<AnimationGraph>>()
        .add(graph);
    app.world_mut()
        .spawn((
            AnimationToPlay {
                graph_handle: graph_handle.clone(),
                index,
            },
            WorldAssetRoot(scene),
        ))
        .observe(play_animation_when_ready);

    app.update();

    let world = app.world_mut();
    let mut players = world.query::<(&AnimationPlayer, &AnimationGraphHandle)>();
    let (player, graph) = players.single(world).expect("the scene spawned a player");
    assert_eq!(graph.0, graph_handle);
    let run = player.animation(index).expect("the run is playing");
    assert_eq!(run.repeat_mode(), RepeatAnimation::Forever);
}

#[test]
fn t_cycles_through_all_four_strategies() {
    let mut app = keys_app();
    let camera = app.world_mut().spawn(Strategy3d::default()).id();

    let mut seen = Vec::new();
    for _ in 0..4 {
        write_key(&mut app, character('t'));
        app.update();
        seen.push(*app.world().entity(camera).get::<Strategy3d>().unwrap());
    }

    assert!(matches!(seen[0], Strategy3d::Luminance(ramp) if ramp.characters == RAMP_SHADING));
    assert!(matches!(seen[1], Strategy3d::Luminance(ramp) if ramp.characters == RAMP_ASCII));
    assert_eq!(seen[2], Strategy3d::Braille);
    assert_eq!(seen[3], Strategy3d::Halfblocks);
}

#[test]
fn e_cycles_the_edge_overlay_and_back_off() {
    let mut app = keys_app();
    let camera = app.world_mut().spawn(Strategy3d::default()).id();

    let mut seen = Vec::new();
    for _ in 0..4 {
        write_key(&mut app, character('e'));
        app.update();
        let overlay = app.world().entity(camera).get::<EdgeOverlay>();
        seen.push(overlay.map(|overlay| overlay.source));
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

#[test]
fn q_quits() {
    let mut app = keys_app();

    write_key(&mut app, character('q'));
    app.update();

    assert_eq!(app.should_exit(), Some(AppExit::Success));
}

const LOAD_POLL: Duration = Duration::from_millis(10);
const MAX_LOAD_POLLS: usize = 3000;
const SETTLED_FRAMES: usize = 5;
const PLAYED_FRAMES: usize = 5;

fn gpu_app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, TermPlugin));
    app.insert_resource(TerminalSize::new(80, 24));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(STEP));
    add_scene(&mut app);
    while app.plugins_state() == PluginsState::Adding {
        bevy_tasks::tick_global_task_pools_on_main_thread();
    }
    app.finish();
    app.cleanup();
    app
}

/// Updates until `found` answers, polling the asset loader's threads.
fn update_until<T>(app: &mut App, mut found: impl FnMut(&mut App) -> Option<T>) -> T {
    for _ in 0..MAX_LOAD_POLLS {
        app.update();
        if let Some(value) = found(app) {
            return value;
        }
        std::thread::sleep(LOAD_POLL);
    }
    panic!("gave up waiting after {MAX_LOAD_POLLS} frames");
}

fn set_paused(app: &mut App, player: Entity, is_paused: bool) {
    let mut entity = app.world_mut().entity_mut(player);
    let mut player = entity.get_mut::<AnimationPlayer>().unwrap();
    if is_paused {
        player.pause_all();
    } else {
        player.resume_all();
    }
}

#[test]
#[ignore = "requires a wgpu adapter"]
fn the_run_animates_through_readback() {
    let mut app = gpu_app();
    let player = update_until(&mut app, |app| {
        let world = app.world_mut();
        let mut players = world.query_filtered::<Entity, With<AnimationGraphHandle>>();
        players.iter(world).next()
    });
    set_paused(&mut app, player, true);
    let mut settled = 0;
    let mut previous = String::new();
    let still = update_until(&mut app, |app| {
        let frame = composed_styled_frame(app);
        // An unchanged frame proves nothing while a pipeline is still compiling.
        let is_compiled = app
            .sub_app(RenderApp)
            .world()
            .resource::<PipelineCache>()
            .waiting_pipelines()
            .next()
            .is_none();
        settled = if is_compiled && frame == previous {
            settled + 1
        } else {
            0
        };
        previous = frame;
        (settled == SETTLED_FRAMES).then(|| previous.clone())
    });

    set_paused(&mut app, player, false);
    for _ in 0..PLAYED_FRAMES {
        app.update();
    }

    assert_ne!(
        composed_styled_frame(&app),
        still,
        "the running fox drew the same frame as the paused one"
    );
}
