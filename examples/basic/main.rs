//! Every stock ratatui widget as an entity, on default features alone -
//! no ui or widgets crate. Press `q` or ctrl-c to quit.

mod widgets;

use std::time::Duration;

use bevy_app::{App, AppExit, ScheduleRunnerPlugin, Startup, Update};
use bevy_ecs::prelude::{
    Added, Commands, Component, DetectChanges, DetectChangesMut, Local, MessageReader,
    MessageWriter, Query, Res,
};
use plurimus::core::ratatui_core::layout::Rect;
use plurimus::core::{CorePlugin, TerminalCamera, TerminalSize, UiArea, UiWidget};
use plurimus::crossterm::CrosstermPlugin;
use plurimus::term::{KeyCode, KeyKind, KeyMessage};

use widgets::{TILES, TileSpec};

const COLUMNS: u16 = 4;
const ROWS: u16 = 3;
const ANIMATION_PERIOD: u32 = 4;

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins((
        ScheduleRunnerPlugin::run_loop(Duration::from_millis(16)),
        CorePlugin,
        CrosstermPlugin::default(),
    ));
    app.add_systems(Startup, spawn_gallery);
    app.add_systems(Update, (quit_on_q, layout_tiles, animate_tiles));
    app.run()
}

/// A tile's slot in the grid; the layout system resolves it to an area.
#[derive(Component)]
struct Slot(u16);

/// Rebuilds the tile's content as frames advance.
#[derive(Component)]
struct Animated(&'static TileSpec);

fn spawn_gallery(mut commands: Commands) {
    commands.spawn(TerminalCamera::default());
    for (index, spec) in (0u16..).zip(TILES.iter()) {
        let mut tile = commands.spawn((Slot(index), (spec.build)(spec.title, 0)));
        if spec.animated {
            tile.insert(Animated(spec));
        }
    }
}

// Areas are camera-local cells, so a resize is just a fresh division
// of the terminal.
fn layout_tiles(
    size: Res<TerminalSize>,
    spawned: Query<(), Added<Slot>>,
    mut tiles: Query<(&Slot, &mut UiArea)>,
) {
    if !size.is_changed() && spawned.is_empty() {
        return;
    }
    for (slot, mut area) in &mut tiles {
        area.set_if_neq(UiArea::Fixed(slot_rect(slot.0, size.rect())));
    }
}

const fn slot_rect(index: u16, screen: Rect) -> Rect {
    let width = screen.width / COLUMNS;
    let height = screen.height / ROWS;
    Rect::new(
        (index % COLUMNS) * width,
        (index / COLUMNS) * height,
        width,
        height,
    )
}

// Widget content is immutable: animating means replacing the component.
fn animate_tiles(mut tiles: Query<(&Animated, &mut UiWidget)>, mut frames: Local<u32>) {
    *frames += 1;
    if !(*frames).is_multiple_of(ANIMATION_PERIOD) {
        return;
    }
    for (animated, mut widget) in &mut tiles {
        let spec = animated.0;
        *widget = (spec.build)(spec.title, *frames);
    }
}

fn quit_on_q(mut keys: MessageReader<KeyMessage>, mut exit: MessageWriter<AppExit>) {
    for key in keys.read() {
        let ctrl_c = key.modifiers.ctrl && key.code == KeyCode::Char('c');
        if key.kind == KeyKind::Press && (key.code == KeyCode::Char('q') || ctrl_c) {
            exit.write(AppExit::Success);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TILES, animate_tiles, layout_tiles, spawn_gallery};
    use bevy_app::{App, Startup, Update};
    use plurimus::core::{CorePlugin, TerminalSize};
    use plurimus_test::composed_frame;

    #[test]
    fn every_widget_tile_renders_its_title() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(TerminalSize {
            cols: 100,
            rows: 30,
        });
        app.add_systems(Startup, spawn_gallery);
        app.add_systems(Update, (layout_tiles, animate_tiles));
        app.update();

        let frame = composed_frame(&app);
        for spec in &TILES {
            assert!(
                frame.contains(spec.title),
                "{} tile missing from the frame:\n{frame}",
                spec.title
            );
        }
    }
}
