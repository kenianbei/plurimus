# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **`WidgetSystems`**, the set the stock widget systems run in, so an app can
  order its own systems against widget layout or styling instead of guessing.
- **`StylistDisabled`**, which exempts one entity from the stock stylists. The
  widget keeps its behavior - selection, keys, scrolling, events - while the app
  owns what it draws.
- **`UiStyle`**, a style patched over the one an entity would otherwise resolve
  to. On a widget it composes with hover and focus rather than replacing them;
  on a list row it styles the full row, which is what a striped or state-colored
  list needs.
- **`ListBoxSelectionMarker`** and **`ListBoxCursor`**, so a list box's marker
  column and cursor symbol are the app's to choose. An empty cursor symbol gives
  bar-style selection with no gutter at all.

### Changed

- **`UiLabel` carries a ratatui `Line`** rather than a `String`, so a label can
  hold per-span style - independently colored columns in a list row, a dimmed
  shortcut beside a menu item. Every widget constructor now takes
  `impl Into<Line<'static>>`; string literals still work unchanged, but a
  borrowed non-`'static` `&str` needs `.to_owned()`.
- **A focused list box highlights its cursor row** instead of repainting every
  row. A pane whose contents hold focus still shows it on the border, and rows
  keep whatever color they carry. Disabled list boxes still dim throughout.
- **A list box no longer draws its selection marker column** unless it carries
  `ListBoxSelectionMarker`, making every list two cells narrower by default.

## [0.2.0] - 2026-08-05

A complete rebuild as a multi-crate workspace. Nothing carries over from 0.1.0.

### Added

- **Terminal render sub-app** mirroring `bevy_render`: an extract phase into a
  dedicated sub-app, then rasterize, composite, and present.
- **Terminal cameras** with cell-space viewports - full, fixed rect, docked edge
  strip, or fill - composited in z-order.
- **Widget primitive**: any ratatui widget as an entity, placed by `UiArea` and
  drawn every frame.
- **Backend-generic presenter** that diffs each frame and writes only the
  changed cells, so any ratatui `Backend` presents.
- **Input contract** in both shapes: discrete key, mouse, paste, and focus
  messages, plus polled `ButtonInput` state. Real press, repeat, and release on
  kitty-protocol terminals, with capability-driven synthesis elsewhere.
- **Crossterm integration**: terminal lifecycle with restore on exit and on
  panic, color-depth detection, and event translation.
- **UI pipeline**: z-order hit testing, hover and pointer routing, focus over
  `bevy_input_focus`, directional navigation, scrolling, and modal overlays.
- **Widget library**: buttons, checkboxes, radio groups, sliders, scrollbars,
  list boxes, panes, menus, popovers, and single- and multi-line text editing,
  with theming.
- **bevy_ui layout bridge**: real `Node` trees laid out by taffy at one cell per
  pixel.
- **2d pipeline**: transform-positioned glyphs and subcell pixels projected per
  camera, with halfblock and braille modes and `RenderLayers` masking. Glyphs
  and pixels both sort by transform `z`, so overlapping entities layer
  deterministically.
- **`PixelBlock` sprites**: a palette-indexed pixel-art bitmap stamped one pixel
  per subcell, with optional horizontal mirroring.
- **3d pipeline**: GPU camera readback converted to cells by strategy, with
  depth readback, cross-camera occlusion, and edge overlays.
- **Examples**: `basic`, `widgets`, `pong`, `ratman`, and `lander`, each
  compiled as a test.

## [0.1.0] - 2026-03-03

Initial proof of concept: components and systems layered over bevy_ratatui.

[0.2.0]: https://crates.io/crates/plurimus/0.2.0
[0.1.0]: https://crates.io/crates/plurimus/0.1.0
