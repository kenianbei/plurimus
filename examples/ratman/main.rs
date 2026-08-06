//! Rat-Man on the 2d pipeline: a ratatui rat eats cheese through a maze
//! while four bevy birds hunt it, all drawn as halfblock pixel art.
//!
//! Arrows or WASD steer, `r` starts over, `q` or ctrl-c quits. The maze
//! wants a terminal of at least 280 by 76 cells.

mod actor;
mod chase;
mod game;
mod ghosts;
mod hud;
mod input;
mod maze;
mod sprites;
#[cfg(test)]
mod tests;
mod walls;

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
