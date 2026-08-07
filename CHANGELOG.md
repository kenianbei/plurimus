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
- **`Table`**, a widget for tabular text over ratatui's table engine. Rows are
  child entities carrying their own cells, so an app builds a table the way it
  builds anything else. `TableColumns` sets the widths, `TableHeader` and
  `TableFooter` mark a row as a band, `TableStripe` bands alternate body rows,
  `TableCheckedStyle` paints the selected ones, and `TableLayout` sets column
  spacing and where spare width goes. Adding a `ScrollArea` makes it scrollable,
  as it does a list box.
- **`TableSelection`** makes a table interactive at row, column, or cell
  granularity - a table without it draws and hovers but is not a tab stop and
  consumes no keys. Selection emits `ValueChange<TablePosition>` in every mode,
  and `table_self_update` applies it to `Checked` for uncontrolled use, with
  `TableMultiSelect` for multiple rows.
- **`TableKeys`**, a table's movement bindings as data rather than a closed
  match, so an app remaps arrows to `j`/`k` by editing a list instead of
  reimplementing movement beside the widget. `TableAction` names what a key
  does; the default map keeps the arrows, `Home`/`End`, `PageUp`/`PageDown`, and
  Enter and space.
- **`TableHeaderClick`**, reporting the column whose header was clicked so an
  app can sort. The crate supplies the geometry; the ordering stays with the
  app, which is the only side that knows whether a column is text or numbers.
- **`Key`** is re-exported from `plurimus_widgets`, so an app can name the key
  type `TableKeys` holds without depending on `bevy_input` directly.
- **A `table` example**: a sortable, scrollable process list showing striping, a
  bolded header, a totals footer, remapped movement keys, and app-side sorting
  driven by header clicks.

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
- **Many accessors and builder methods are now `const fn`**, so they can be
  called in `const` contexts: `PixelGrid::width`/`height`,
  `HalfblockGrid::subcell_area` and its braille counterpart,
  `TerminalSize::rect`, `PresenterPlugin::new`, `CrosstermPlugin::with_writer`
  and its `mouse`/`paste`/`detect_color_depth` builders, `ScrollArea::new`,
  `max_offset`, `SliderRange::new`/`start`/`end`, `PopoverSide::mirror`,
  `Sprite`'s `style` and `mirrored` builders, and the text-state cursor
  accessors.
- **The spawn-bundle constructors are `#[must_use]`** - `listbox()`,
  `menu_popup()`, `scrollbar()`, and `slider()` - so dropping the bundle instead
  of spawning it is now a warning rather than a silent no-op.

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
