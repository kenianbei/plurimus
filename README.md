# Plurimus

[![Crate Badge]][Crate] [![Docs Badge]][Docs] [![CI Badge]][CI]
[![Deps Badge]][Deps] [![License Badge]][License]

A Bevy-native terminal renderer: cameras, widgets, and 2d/3d pipelines drawn to
terminal cells.

![The pong example: two cyan paddles return a white ball across a dark court
under a score line][Pong Demo]

Plurimus renders a Bevy world to the terminal the way `bevy_render` renders one
to a window. Your program is an ordinary Bevy `App`; any number of systems
contribute to a frame, and one presenter writes only the cells that changed.
Multiple `TerminalCamera`s split the screen the way cameras split a window - a
map, a sidebar, and a minimap are three cameras. The cell model and widgets are
ratatui's.

[Try ratman and the lander in the browser.][Demos] See
[ARCHITECTURE.md](ARCHITECTURE.md) for how the crates fit together.

## Features

Each feature enables one crate, and the tiers stack.

| feature     | crate                | gives you                           |
| ----------- | -------------------- | ----------------------------------- |
| _(none)_    | `plurimus_core`      | cameras, compositing, any `Backend` |
| `term`      | `plurimus_term`      | input as bevy messages, clipboard   |
| `crossterm` | `plurimus_crossterm` | a live terminal                     |
| `web`       | `plurimus_web`       | a WebGL2 canvas in a browser        |
| `ui`        | `plurimus_ui`        | picking, focus, scrolling, theming  |
| `widgets`   | `plurimus_widgets`   | controls, tables, text editors      |
| `bevy-ui`   | `plurimus_bui`       | `bevy_ui` flexbox layout            |
| `2d`        | `plurimus_2d`        | world-space glyphs and pixel art    |
| `3d`        | `plurimus_3d`        | GPU camera readback                 |

`crossterm`, `web`, and `ui` imply `term`; `widgets` and `bevy-ui` imply `ui`.
The default is `crossterm`; `default-features = false` builds core only.

## Quick start

```toml
[dependencies]
plurimus = "0.10"
bevy_app = "0.20"
bevy_ecs = "0.20"
ratatui-widgets = "0.3"
```

Add whichever bevy crates you use at 0.20 and cargo unifies them with
plurimus's. Plurimus re-exports the crates its API uses: `ratatui_core` from
core, `bevy_input` from `term`, `bevy_input_focus` and `Key` from `ui`, and
`ratatui_widgets` from `widgets`.

```rust,no_run
use std::time::Duration;

use bevy_app::{App, AppExit, ScheduleRunnerPlugin, Startup};
use bevy_ecs::prelude::Commands;
use plurimus::core::{CorePlugin, TerminalCamera, UiArea, UiWidget};
use plurimus::crossterm::CrosstermPlugin;
use ratatui_widgets::block::Block;
use ratatui_widgets::paragraph::Paragraph;

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins((
        ScheduleRunnerPlugin::run_loop(Duration::from_millis(16)),
        CorePlugin,
        CrosstermPlugin::default(),
    ));
    app.add_systems(Startup, spawn_ui);
    app.run()
}

fn spawn_ui(mut commands: Commands) {
    commands.spawn(TerminalCamera::default());
    commands.spawn((
        UiWidget::new(Paragraph::new("hello, terminal").block(Block::bordered())),
        UiArea::Fill,
    ));
}
```

A terminal has no window to pace it, so without `ScheduleRunnerPlugin` the app
updates once and stops. Exiting is the app's: write `AppExit`, and the crossterm
tier restores the terminal on the way out.

## Testing headlessly

Nothing but the presenter touches the terminal, so a test builds a real app,
drives it, and reads the composed `FrameBuffer` out of the render sub-app:

```rust
use bevy_app::App;
use plurimus::core::{CorePlugin, FrameBuffer, TerminalRenderApp};
use plurimus::core::{TerminalCamera, TerminalSize, UiArea, UiWidget};
use ratatui_widgets::paragraph::Paragraph;

let mut app = App::new();
app.add_plugins(CorePlugin);
app.insert_resource(TerminalSize::new(24, 3));
app.world_mut().spawn(TerminalCamera::default());
app.world_mut().spawn((UiWidget::new(Paragraph::new("ready")), UiArea::Fill));
app.update();

let frame = &app.sub_app(TerminalRenderApp).world().resource::<FrameBuffer>().0;
let top_row: String = (0..frame.area.width)
    .filter_map(|x| frame.cell((x, 0)).map(|cell| cell.symbol()))
    .collect();
assert!(top_row.starts_with("ready"));
```

## Examples

Run one with `cargo run --example <name> --features <features>`.

| example    | features                | shows                                |
| ---------- | ----------------------- | ------------------------------------ |
| `basic`    | _(default)_             | every stock ratatui widget           |
| `headless` | `--no-default-features` | core alone into a `TestBackend`      |
| `widgets`  | `widgets,bevy-ui`       | the control library, two layouts     |
| `todo`     | `widgets,bevy-ui`       | one tree, terminal or `-- --desktop` |
| `table`    | `widgets`               | a sortable table with vim keys       |
| `tabs`     | `widgets`               | tab bars in every look               |
| `pong`     | `widgets,2d`            | 2d sprites and a widget score line   |
| `ratman`   | `widgets,2d`            | a maze chase in pixel art            |
| `lander`   | `widgets,3d`            | 3d readback with a widget HUD        |
| `web`      | `web,widgets`           | the browser tier, built as below     |

### In the browser

`web`, `ratman`, and `lander` build for `wasm32-unknown-unknown` and are served
as static pages, using the `wasm-bindgen` CLI at the version `Cargo.lock` pins:

```sh
cargo build --profile wasm-release --example ratman \
  --target wasm32-unknown-unknown --no-default-features --features web,widgets,2d
wasm-bindgen --target web --out-dir examples/ratman/pkg \
  target/wasm32-unknown-unknown/wasm-release/examples/ratman.wasm
python -m http.server -d examples
```

Swap in the example's name and features, and open its directory under the
server. The lander needs hardware WebGPU: Chrome on Linux falls back to software
unless started with `--enable-unsafe-webgpu --enable-features=Vulkan`.

## Requirements

- **Rust 1.97.1** or newer, edition 2024.
- **Bevy 0.20** and **ratatui-core 0.1**, for plurimus 0.10.
- **A terminal** crossterm supports. The kitty keyboard protocol adds real key
  releases; truecolor, 256-color, and 16-color terminals are detected.
- **A browser with WebGL2**, for the `web` tier instead.
- **A GPU adapter**, for the `3d` tier only - in a browser, hardware WebGPU.

## Status

Pre-1.0, versioned in lockstep across the workspace. The API still moves between
minor releases; [CHANGELOG.md](CHANGELOG.md) records what changed.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[Crate]: https://crates.io/crates/plurimus
[Demos]: https://kenianbei.github.io/plurimus/
[Crate Badge]:
  https://img.shields.io/crates/v/plurimus?logo=rust&style=flat-square&color=E05D44
[Docs]: https://docs.rs/plurimus
[Docs Badge]: https://img.shields.io/docsrs/plurimus?logo=rust&style=flat-square
[CI]: https://github.com/kenianbei/plurimus/actions/workflows/ci.yml
[CI Badge]:
  https://img.shields.io/github/actions/workflow/status/kenianbei/plurimus/ci.yml?style=flat-square&logo=github
[Deps]: https://deps.rs/repo/github/kenianbei/plurimus
[Deps Badge]:
  https://deps.rs/repo/github/kenianbei/plurimus/status.svg?style=flat-square
[License]: https://github.com/kenianbei/plurimus#license
[License Badge]: https://img.shields.io/crates/l/plurimus?style=flat-square
[Pong Demo]:
  https://raw.githubusercontent.com/kenianbei/plurimus/HEAD/examples/pong/demo.gif
