# Plurimus

[![Crate Badge]][Crate] [![Docs Badge]][Docs] [![CI Badge]][CI]
[![Deps Badge]][Deps] [![License Badge]][License]

A Bevy-native terminal renderer: cameras, widgets, and 2d/3d pipelines drawn to
terminal cells.

![The ratman example: a yellow ratatui rat and four colored bevy birds chasing
it through a blue maze studded with cheese][Ratman Screenshot]

[Try ratman and the lander in the browser.][Demos]

Plurimus renders a Bevy world to the terminal the way `bevy_render` renders one
to a window. The program is an ordinary Bevy `App`, and plurimus adds a render
sub-app that turns that world into cells and presents them.

No single draw callback owns the frame. Any number of systems contribute to it,
and one presenter writes to the terminal, diffing against the previous frame so
only changed cells are sent. Multiple `TerminalCamera`s with cell-space
viewports split the screen the way multiple cameras split a window - a map view,
a sidebar, and a minimap are three cameras, composited in order.

The cell model and widget ecosystem are ratatui's: `Buffer`, `Cell`, `Style`,
and the stock widgets, wrapped as components. Plurimus adds viewports and
z-ordered composition, pointer routing and hit testing, focus and directional
navigation, flexbox layout, world-space 2d, and GPU camera readback.

See [ARCHITECTURE.md](ARCHITECTURE.md) for how the crates fit together.

## Features

Each feature enables one crate, and the tiers stack. The default set -
`plurimus_core`, `plurimus_term`, `plurimus_crossterm` - renders a live terminal
app and reads its input. Anything not enabled is not compiled.

### Core (`plurimus_core`)

The render sub-app, cameras, compositing, and the presenter. Always compiled.

A terminal camera is a bevy `Camera`, and `TerminalCamera` says where it sits in
the cell grid: `Full`, a `Fixed` cell rect, a `Docked` edge strip, or `Fill` for
what remains. Compositing order, activity, and what untouched cells become are
`Camera`'s own `order`, `is_active`, and `clear_color`. Entities carrying a
`UiWidget` - any ratatui widget, including stateful ones - are placed by
`UiArea`, ordered by `UiOrder`, and drawn every frame. The `TerminalSize`
resource is the target's cell dimensions, so a resize is a value change layout
systems react to - written by `plurimus_term` when a real terminal reports one,
and by the app otherwise.

The presenter is generic over ratatui's `Backend`, so core renders without a
terminal. Dependencies are `bevy_app`, `bevy_ecs`, `bevy_camera`,
`bevy_extract`, `bevy_color`, `bevy_math`, and `ratatui-core`.

### Terminal (`plurimus_term`)

Everything that needs a terminal to mean anything, in both directions.

Inbound arrives in two shapes from the same events. Discrete messages - bevy's
`KeyboardInput`, `MouseButtonInput`, `MouseWheel`, `CursorMoved` and
`WindowFocused`, plus `PasteMessage` and `TerminalResized` - for text entry and
keymaps; polled `ButtonInput<KeyCode>` and `ButtonInput<MouseButton>` for game
loops. The pointer sits at a cell's centre in `CellSize` pixels, which is what
`bevy_picking` hit-tests against.

Outbound is `TerminalRequest`: copy to a clipboard selection, set the window
title. Best-effort, because nothing a terminal is asked can be confirmed.

`InputCapabilities` records what the terminal reports. Terminals implementing
the kitty keyboard protocol give real press, repeat, and release; elsewhere
releases are synthesized on a `ReleaseTimeout`, which is degraded but documented
rather than silent. Losing focus releases every held key on any tier, since a
terminal reports nothing while unfocused.

### Crossterm (`plurimus_crossterm`)

Terminal ownership: raw mode, alternate screen, mouse capture, bracketed paste,
and the kitty keyboard protocol where available, restored on exit and on panic.
It detects color support from the environment, translates crossterm events into
input messages, reports resizes, and supplies the backend to core's presenter.

The writer is generic: stdout by default, or the controlling terminal via
`CrosstermPlugin::tty()` so stdout stays free for piped output.

### Web (`plurimus_web`)

The browser in place of a terminal, for `wasm32-unknown-unknown`: a WebGL2
canvas in the page (through ratzilla), the browser's keys - with real releases -
pointer, wheel, paste and focus translated into the same input messages, and a
`requestAnimationFrame` runner. The font size is chosen once from a `GridFit`,
such as the largest size that still holds a minimum grid, and the canvas follows
its element's size after that. Clipboard copies and window titles are served,
and an app's exit hands the page a `plurimus-exit` event to act on.

### UI (`plurimus_ui`)

Interaction over any entity with an area. It computes widget areas and is the
`bevy_picking` backend for them, so hover, press, drag, release, and click are
bevy's pointer events, and it routes wheel ticks itself, by z-order. Focus runs
over `bevy_input_focus`, with a directional navigation map for arrow-key
movement, scrolling through `ScrollArea` and `ScrollIntoView` - a wheel tick
arriving as a `ScrollBy` an app's own key handler can send too - and the
modal-overlay primitives menus and popovers are built from.

Keys are bound as data: a `KeyBinding` is a `Key` and the modifiers it must be
pressed under, and `first_bound` is the scan every bindings component shares, so
a widget written outside this workspace remaps the same way the table and the
text widgets do.

It also owns the styling contract a widget library builds on, rather than
inventing its own: the `UiTheme` resource, `UiStyle` to patch one entity's
style, and `StylistDisabled` to take one entity's look over entirely - plus the
engine that consumes them. `StylistCache` records what a widget last drew and
`StylistCache::redraws` is the comparison that keeps an idle frame free, so a
widget library outside this workspace gets the same machinery the stock widgets
use. A `UiLabel` is the text a stylist draws, a ratatui `Line`.

Nothing here is specific to stock widgets - an entity of your own with an area
is hoverable, clickable, and focusable.

### Widgets (`plurimus_widgets`)

Buttons, checkboxes, radio groups, sliders, scrollbars, list boxes, tables, tab
bars, panes, menus, popovers, single-line text input, and a multi-line text
editor. What is this crate's own is the stylists themselves - one per widget,
resolving the `UiTheme` and driving the cache that `plurimus_ui` owns.

The component and event vocabulary mirrors `bevy_ui_widgets`: widgets are
stateless controllers emitting `Activate` and `ValueChange`, applied by the app
for controlled behavior or by the stock `*_self_update` observers for
uncontrolled. The button, checkbox, radio group, slider, list box, tab bar and
menu are upstream's own types and observers, drawn in cells, so they answer to
upstream's fixed keys and follow its models - a list's cursor and `Selected`
rows, a tab bar's roving focus, a menu that closes when focus leaves it. The
widgets upstream has no counterpart for take their keys from a bindings
component - `TableKeys`, `TextInputKeys`, `TextEditorKeys` - each with a default
table, so remapping a table to vim keys is a component swap rather than a
rewrite.

### `bevy_ui` Layout (`plurimus_bui`)

`bevy_ui`'s layout stack - `Node` trees computed by taffy - run against terminal
cameras at `CellSize` pixels per cell, with backgrounds, borders, gradients, and
text. Layout, stacking, and picking run; `bevy_ui`'s font, focus, and asset
systems stay out, and text is measured by grapheme width rather than font
rasterization.

Use it instead of hand-computed `Rect`s for responsive panels and nested rows
and columns. Nodes are picked by `bevy_picking` beside the widgets, so they
hover, click, and scroll like them, and upstream `bevy_ui_widgets` - buttons,
sliders, menus, the text field - run on a node tree unchanged.

### 2d Rendering (`plurimus_2d`)

`Glyph`, `GlyphBlock`, `Pixel`, and `PixelBlock` entities positioned by
`Transform`s, projected per camera through bevy's `Camera2d` and its
orthographic projection, so panning and zooming are camera properties. Set
`CellSize { x: 1.0, y: 2.0 }` for squares to draw square on tall cells.
`RenderLayers` is bevy's, masking which cameras see which entities, and
`SubcellMode` renders in halfblocks or braille for two or eight times the
vertical resolution of a cell.

### 3d Rendering (`plurimus_3d`)

A GPU camera read back and converted to cells. `Render3dPlugins` assembles a
headless bevy render stack; the camera renders to an image, the pixels are read
back, and a `Strategy3d` converts them - halfblock color, luminance ramps
(ASCII, blocks, braille, shading), or depth ramps. Depth readback drives
cross-camera occlusion and sobel edge overlays. Set
`CellSize { x: 1.0, y: 2.0 }` as in 2d: the projection and `Aspect3d`'s
letterbox take the cell's shape from it.

This is the only tier pulling in `bevy_render` and wgpu, and it needs a GPU
adapter. Scene building stays with the app: plurimus stops at the render stack,
and the app adds its material system (`PbrPlugin`) and asset loading such as
`bevy_gltf`.

## Usage

### Adding it

```toml
[dependencies]
plurimus = "0.10"
bevy_app = "0.20"
bevy_ecs = "0.20"
ratatui-widgets = "0.3"
```

Plurimus does not re-export the bevy crates, the same way `bevy_pbr` does not
re-export `bevy_reflect`: add whichever you use at bevy 0.20 and cargo unifies
them with plurimus's. The exception is what its own contracts are written in:
`bevy_input_focus` and `bevy_input`'s `Key` from the `ui` and `widgets` tiers,
`ButtonInput` and `MouseButton` from `term`. `ratatui_core` is re-exported as
`plurimus::core::ratatui_core`; the stock widget set is your own dependency
unless the `widgets` feature is on, which re-exports it.

| feature     | crate                | gives you                           |
| ----------- | -------------------- | ----------------------------------- |
| _(none)_    | `plurimus_core`      | rendering, always on                |
| `term`      | `plurimus_term`      | the terminal contract, both ways    |
| `crossterm` | `plurimus_crossterm` | a live terminal (implies `term`)    |
| `web`       | `plurimus_web`       | a browser page (implies `term`)     |
| `ui`        | `plurimus_ui`        | interaction, focus (implies `term`) |
| `widgets`   | `plurimus_widgets`   | stock controls (implies `ui`)       |
| `bevy-ui`   | `plurimus_bui`       | flexbox layout (implies `ui`)       |
| `2d`        | `plurimus_2d`        | world-space sprites                 |
| `3d`        | `plurimus_3d`        | GPU camera readback                 |

`default = ["crossterm"]`. `default-features = false` gives core alone,
rendering into your own `Backend`.

### A first app

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

### Driving the frame loop

A terminal app has no window to pace it, so add
`ScheduleRunnerPlugin::run_loop(interval)` from `bevy_app` or the app updates
once and stops. The interval is the frame budget; the presenter writes only
changed cells, so an idle screen stays cheap at any tick rate. In a browser,
`WebPlugin` installs its own runner, paced by the display.

### Reading input

Both APIs come from the same events, so systems can mix them:

```rust,no_run
use bevy_ecs::prelude::{MessageReader, Res};
use plurimus::term::bevy_input::ButtonState;
use plurimus::term::bevy_input::keyboard::{Key, KeyCode, KeyboardInput};
use plurimus::term::{ButtonInput, HeldModifiers};

// Discrete: one action per press.
fn handle_commands(mut keys: MessageReader<KeyboardInput>, held: HeldModifiers) {
    for key in keys.read() {
        let is_press = key.state == ButtonState::Pressed && !key.repeat;
        let is_r = key.logical_key == Key::Character("r".into());
        if is_press && is_r && !held.get().ctrl {
            // reset something
        }
    }
}

// Polled: continuous, for movement.
fn move_player(keys: Res<ButtonInput<KeyCode>>) {
    if keys.pressed(KeyCode::KeyW) {
        // step forward while held
    }
}
```

A message carries the character the terminal produced as its `logical_key`, case
and all; polled keys are keyed by the physical `KeyCode` the sink infers from
it, so `KeyW` stays held whether it was struck or released as `w` or `W`, and
shift pressed midway through a hold does not strand it. The modifiers held with
a key are polled through `HeldModifiers`, since bevy's `KeyboardInput` carries
none of its own.

### Exiting

No plurimus crate writes `AppExit`; exit policy belongs to the app. Write it
from a key handler, and the crossterm tier restores the terminal on the way out.
In a browser the runner stops, restores the page title and dispatches
`plurimus-exit` on the canvas, and the page decides what leaving means.

### Splitting the screen

Viewports compose: dock a status strip to the bottom, dock a sidebar to the
left, and let the main view `Fill` the rest, and the three stay correct across
resizes. A higher `Camera::order` composites a camera on top, which is how
overlays and modal layers are built.

Widgets drawn with default styling inherit whatever a 2d or 3d pipeline drew
beneath them, so a HUD over a rendered world belongs on its own camera: a docked
strip, or an overlay whose `clear_color` is `ClearColorConfig::None`.

### Testing headlessly

Nothing outside the presenter touches the terminal, so a test builds a real app,
drives it, and reads the composed frame - no terminal and no `CrosstermPlugin`.
The frame is a `FrameBuffer` resource in the render sub-app, holding the ratatui
`Buffer` the presenter would have written:

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

Input is injected the same way the backend delivers it - `plurimus_test`'s
helpers write bevy's messages into the world through the same sink a backend
uses and run `app.update()`.

## Examples

```sh
cargo run --example basic
cargo run --example headless --no-default-features
cargo run --example widgets --features widgets,bevy-ui
cargo run --example todo --features widgets,bevy-ui
cargo run --example todo --features widgets,bevy-ui -- --desktop
cargo run --example table --features widgets
cargo run --example tabs --features widgets
cargo run --example pong --features widgets,2d
cargo run --example ratman --features widgets,2d
cargo run --example lander --features widgets,3d
```

**basic** renders every stock ratatui widget as an entity in a grid on default
features, with no `ui` or `widgets` crate, and re-lays the tiles on resize. `q`
or ctrl-c quits.

**headless** is the lean tier: `plurimus_core` alone, driving two cameras, a
hand-written `TerminalWidget` over the halfblock subcell grid, compositing and
downsampling into a `TestBackend` that holds the cells in memory. It prints one
frame and exits, and the `--no-default-features` invocation is the point - there
is no terminal contract in the example's own graph at all.

**widgets** runs the control library twice side by side: themed widgets at fixed
cell rects on the left, the same widget logic under `bevy_ui` flex layout on the
right. Tab and Shift-Tab move focus, arrows navigate and adjust the focused
slider, Enter or Space activates, the mouse hovers, clicks, and drags, and a
menu resets or disables every widget. Esc unfocuses; `q` with nothing focused
quits. Wants roughly 80x30 or larger.

**todo** is a todo list on one `bevy_ui` tree of upstream `bevy_ui_widgets`, run
in the terminal or, with `--desktop`, in a bevy window: the tree states
structure and tags each node with a `Part`, and each skin answers with its own
sizes and colours - cells and box-drawing in one, pixels and a card in the
other. The ledger is a `Display::Grid` of unwrapped cells with right-justified
ids; the field is the one control each target supplies for itself. Wants roughly
60x16 or larger in a terminal.

**table** is a sortable, scrollable process table: a striped body under a bold
header and a footer of totals, sorted by clicking a header cell, with vim keys
bound through `TableKeys`.

**tabs** switches panels from a `TabBar` drawn in the joined look, driven by the
arrows and by `[`/`]` and digits the app binds itself; its Settings panel holds
a vertical bar that picks the top bar's look - plain, divided, boxed, joined,
padded. Tab moves between the two bars while Settings is open.

**pong** puts the 2d and ui pipelines in one camera: a halfblock ball and
paddles in world space under a widget score line. W/S steps the left paddle,
Up/Down the right, and `r` serves immediately.

**ratman** is a maze chase drawn entirely as halfblock pixel art: a ratatui rat
eats cheese while four bevy birds hunt it, each sprite traced from its logo.
Arrows or WASD steer, a power cheese turns the birds edible, and `r` starts
over. Wants roughly 280x76 or larger.

**lander** flies a moon lander through the 3d pipeline with a widget HUD. W or
space burns the main thruster, A/D tilt, `t` cycles the pixel-to-cell strategy,
`e` cycles the sobel edge overlay, `r` resets. The first frames take a few
seconds while GPU pipelines compile.

**web** is the browser tier: a list to click, an editor to type, paste, copy
from (select with shift-arrows, then ctrl-c) and scroll with the wheel, and
buttons that set the page's title and quit, which reloads the page. It builds
for wasm and is served as a static page, with the `wasm-bindgen` CLI at the
version `Cargo.lock` pins:

```sh
cargo build --profile wasm-release --example web \
  --target wasm32-unknown-unknown --no-default-features --features web,widgets
wasm-bindgen --target web --out-dir examples/web/pkg \
  target/wasm32-unknown-unknown/wasm-release/examples/web.wasm
python -m http.server -d examples/web
```

The `wasm-release` profile optimizes for size and strips symbol names, which
takes the page from about 23 MB to 6.

**ratman** and **lander** build for the browser the same way, adding `web` to
their features, and `examples/` serves them under a landing page exactly as the
[live demos][Demos] do:

```sh
cargo build --profile wasm-release --example ratman \
  --target wasm32-unknown-unknown --no-default-features --features web,widgets,2d
wasm-bindgen --target web --out-dir examples/ratman/pkg \
  target/wasm32-unknown-unknown/wasm-release/examples/ratman.wasm
python -m http.server -d examples
```

The lander is the same with `web,widgets,3d`, and needs hardware WebGPU: Chrome
on Linux falls back to software unless started with
`--enable-unsafe-webgpu --enable-features=Vulkan`, and the page says so rather
than crawl.

## Requirements

| plurimus | bevy | ratatui-core |
| -------- | ---- | ------------ |
| 0.10     | 0.20 | 0.1          |

- **Rust 1.97.1** or newer, edition 2024.
- **Bevy 0.20** for any bevy crates added alongside.
- **A terminal.** Anything crossterm supports; the kitty keyboard protocol adds
  real key releases. Truecolor, 256-color, and 16-color terminals are detected
  and composited down.
- **A browser with WebGL2**, for the `web` tier instead: built for
  `wasm32-unknown-unknown` without wasm threads.
- **A GPU adapter**, for the `3d` tier only - in a browser, hardware WebGPU.
  Every other tier is CPU-only.

## Status

Pre-1.0, versioned in lockstep across the workspace. The architecture is
settled; the API still moves between minor releases, and
[CHANGELOG.md](CHANGELOG.md) records what changed.

Types whose vocabulary is open carry `#[non_exhaustive]`, so they can gain a
field or a variant without a breaking release; each keeps a constructor or a
`Default` you build from. Types an app must handle exhaustively to be correct
are deliberately left open, so growth there is a compile error rather than a
silently-taken `_` arm. [ARCHITECTURE.md](ARCHITECTURE.md) states which are
which.

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
[Ratman Screenshot]:
  https://raw.githubusercontent.com/kenianbei/plurimus/HEAD/examples/ratman/screenshot.png
