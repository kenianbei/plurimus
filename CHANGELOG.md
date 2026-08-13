# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **`plurimus_test::write_focus` and `send_focus`**, the focus half of the
  injection helpers, following the module's queue-only and queue-then-tick
  families.

### Fixed

- **A held key is reported as held.** A terminal that honors the kitty
  protocol's event types without detectable autorepeat encodes a held key as a
  release immediately followed by a press, so nothing ever produced
  `KeyKind::Repeat` and a held key read as roughly twenty-five presses a second:
  a focused button activated that often, a checkbox toggled that often,
  directional navigation moved that often, and `just_pressed` and
  `just_released` both fired on every cycle. The crossterm backend now reports
  such a pair as the repeat it is. A terminal reporting repeats natively is
  unaffected and pays nothing for the check - the encoding is learned from what
  arrives rather than assumed.
- **Keys held when the terminal loses focus are released.** A terminal reports
  no key events while unfocused, so the release of a key held across an alt-tab
  never arrived; on the kitty tier nothing expired it either and the key stayed
  down until the user returned and tapped it. `FocusMessage`, which until now
  had no consumer anywhere, releases every held key. Keys only: a synthetic key
  release corrects held state and triggers nothing, while a pointer release
  would complete a click that was never made.
- **A repeat refreshes polled state**, so a hold survives a press that went
  missing rather than reading as released until the user lets go.
- **A release cancels its press whatever modifiers it carries.** Synthesized
  releases were matched on the key together with its modifier bits, so a shift+a
  gesture - which ends with an `a` release carrying nothing, since an event
  reports the state it leaves behind - left the key held to expire later as a
  phantom release.
- **A shifted letter carries the same modifiers on every terminal.** The kitty
  protocol reports the shifted character alongside the key and the shift bit was
  dropped with it, while a legacy terminal reports the same uppercase character
  and sets the bit. A shifted symbol still carries what the terminal sent, since
  nothing in the event says which key produced it, which `KeyModifiers` now
  documents.

## [0.6.0] - 2026-08-12

### Added

- **The published types that grow now say so.** Fifteen enums and thirty-nine
  structs carry `#[non_exhaustive]`, so adding a pipeline phase, a cursor shape,
  a 3d strategy, a theme state or a field to a config is a minor release rather
  than a breaking one. Which types those are is stated in `ARCHITECTURE.md`
  rather than left to be rediscovered: a type is sealed when its vocabulary is
  open - defined by terminals, by pipeline phases, or by an app's needs - and
  deliberately left open when an app has to handle every case to be correct, so
  that growth is a compile error rather than a `_` arm that swallows it.
  `KeyKind`, `ClipboardTarget`, `TableSelection`, `UiArea`, `PopoverSide`,
  `PopoverAlign` and `Edge` are open by decision, and each now says so where it
  is declared.
- **Constructors for everything sealed**, because an attribute that leaves a
  type unbuildable is a wall rather than forward compatibility. `TerminalSize`
  and `UiTheme` gain `const fn new`, since a size and a theme are often built in
  a `const` context where `Default` cannot be called; `TerminalCamera`,
  `UiTheme` and `InteractionState` gain `with_*` builders; `Popover`,
  `Scrollbar`, `WheelAxes`, `TerminalCursor`, `Pixel`, the four backend message
  types, `TerminalContext`, `ExtractedWidget`, `ExtractedCamera`, the two 3d
  ramps, and the widget events (`Click`, `PointerPress`, `PointerDrag`,
  `PointerRelease`, `ScrollBy`, `ValueChange`, `TableHeaderClick`) all gain a
  `new`. A type whose `Default` and public fields already build it got nothing
  new.

- **A scrolled pane can be scrolled from the keyboard.** `ScrollKeys` binds keys
  to `ScrollAction`s - page, jump to either end, move by a row - and is the
  whole opt-in: it carries `TabIndex`, so adding it makes the widget a tab stop
  that can be sent a key at all. Bindings are data scanned in order, the
  treatment `ListBoxKeys` and `TableKeys` already had, so an app remaps a pane
  to vim keys by replacing the component. Deliberately not required by
  `ScrollArea`, since a list box, table, or editor owning its own movement keys
  would otherwise answer one press twice. Horizontal actions exist but ship
  unbound, so an area that does not overflow sideways leaves the left and right
  arrows to directional navigation. A bound key is consumed whether or not the
  offset moved, so a pane sitting at an extreme does not lose focus to a
  neighbour instead of ignoring the key.
- **`plurimus_ui::first_bound`**, the scan behind all three bindings components,
  so a widget library outside this workspace states "first match wins, a release
  binds to nothing" by calling it rather than by copying it. `plurimus_ui` now
  also re-exports `bevy_input`'s `Key`, the type those bindings are written in.
- **A paste key can be correct.** `LastCopied` records what an app last asked
  the clipboard for, filled by a stock system from the `TerminalRequest` stream
  in the new `RequestSystems::Echo` set. Plurimus still never reads the system
  clipboard back - the escape that would is widely disabled - so this is an echo
  of what was asked, not a reading of what the terminal holds: a backend may
  drop a copy, `plurimus_crossterm` writes none until
  `CrosstermPlugin::clipboard` is set, and another program may replace the
  selection a moment later. What it settles is that every widget in an app that
  asks gets the same answer, which none of them could arrange while the request
  stream stayed one-way. Apps may also write it, to seed what a paste key
  inserts.

### Fixed

- **An empty 3d ramp no longer panics.** `LuminanceRamp` and `DepthRamp` carry a
  public `characters` slice, and converting a frame indexed it as `len() - 1`,
  which underflowed on an empty one - reachable from safe code by any app that
  assigned the field. An empty ramp names no character to draw, so it now leaves
  cells alone.
- **A `ScrollArea` with no widget of its own now scrolls.** Every scroll system
  reads the resolved area, which was attached only to entities that draw, so an
  area drawn by something else - a bevy_ui subtree, an app's own rasterizer -
  silently refused the wheel. `ScrollArea` now requires `ComputedWidgetArea`.
- **`TerminalWidget`'s documentation was wrong about its own trait.** It claimed
  coherence precludes implementing it directly; the `headless` example does
  exactly that and CI compiles it. Implementing the trait directly is how a
  widget that does not follow ratatui's `Widget for &Self` convention joins the
  pipeline, and the docs now say so.

### Changed

- **The 2d sprite builders are named `with_*`.** `Glyph::style` and
  `GlyphBlock::style` are now `with_style`, and `PixelBlock::mirrored` is
  `with_mirrored`, matching every other builder on a sealed type.
  `CrosstermPlugin`'s configuration methods keep their bare names: its fields
  are private, so it was never a sealed type needing a builder path.
- **`TerminalRenderAppExt` is sealed.** Bevy's `App` is its only sensible
  implementor, and sealing is what lets a registration method appear as new
  sub-app phases land. No consumer implements it.
- **`WheelScroll` is now `ScrollBy`, and its step is `(i32, i32)`.** The event
  was never about the wheel - it is the one way anything asks a widget to
  scroll, and the keyboard is now a second producer, so the name said something
  false the moment it had one. The wider step lets a jump to a content edge be
  an ordinary saturated step rather than a special case, and every consumer
  clamps it against its own extent as before. Apps observing the event rename
  the type and widen the tuple; nothing else about it moved.
- **A `TextEditor` copy now leaves the app.** ctrl+c and ctrl+x still copy and
  cut as ratatui-textarea would, but also offer the text to the terminal through
  `TerminalRequest`, where before both moved it into a buffer private to the one
  widget. Neither sends anything when there is no selection, an empty write
  being worse than none. **ctrl+v now pastes** what the app last copied, so a
  copy in one editor is a paste in another; it previously paged down, which the
  `PageDown` key already does. ctrl+y is unchanged and still takes the engine's
  own kill ring, which ctrl+k and ctrl+w fill - the two are deliberately
  separate, so a copy elsewhere in the app cannot displace a kill the user has
  not yet put back.

## [0.5.0] - 2026-08-10

### Added

- **An app can ask the terminal for things, not only be told about them.**
  `TerminalRequest` is the outbound half of the contract `plurimus_term` already
  had one direction of: `CopyToClipboard` writes a selection through OSC 52,
  `SetTitle` sets the window title. Copying is opt-in through
  `CrosstermPlugin::clipboard`, off by default, and a copy too large for one
  escape sequence is dropped with a warning rather than truncated - a clipboard
  holding most of a selection is worse than one holding none. There is
  deliberately no clipboard _read_: the escape is widely disabled and no backend
  here parses a reply, so text still arrives by `PasteMessage`.
- **The terminal's own cursor.** `TerminalCursor` names the screen cell it sits
  in and the shape it takes; `None` hides it. A widget instead attaches
  `WidgetCursor`, in its own content space, and the focused one wins - its caret
  is mapped through the widget's area and scroll offset, and hidden when
  scrolled out of view. This is what a screen reader follows and what an input
  method anchors composition to, neither of which can see a reverse-video cell.
  The stock text widgets keep drawing their own caret for now, since switching
  would change how every existing field looks.
- **A `headless` example**, the lean tier proven by building: core alone driving
  two cameras, a hand-written `TerminalWidget` over the halfblock subcell grid,
  compositing and downsampling into a `TestBackend`. CI builds the examples with
  default features off, so a terminal dependency leaking into core's example
  surface now fails the build.
- **`screen_cell`**, the inverse of `content_cell`. It refuses rather than
  clamps: a caret whose character is scrolled off has no honest screen cell, and
  the nearest edge would put it beside a different character.

### Changed

- **`plurimus_input` is now `plurimus_term`**, because it carries the terminal
  contract in both directions and had been describing itself as "mostly inbound"
  to stay honest. The boundary against `plurimus_core` moved with the name,
  sorted by one question - does this mean anything against a `Backend` with no
  terminal attached? `TerminalResized` and `TerminalCursorStyle` failed it and
  moved to `plurimus_term`; `TerminalSize`, `ColorDepth` and the cursor's cell
  passed it and stayed, as target configuration rather than terminal contract.
  `plurimus_core` now declares no message types at all. **Breaking**: import
  from `plurimus_term`, or `plurimus::term` on the facade where
  `plurimus::input` used to serve; the facade's `input` feature is now `term`;
  `InputPlugin` is `TermPlugin` and requires `CorePlugin` first;
  `plurimus_core::{TerminalResized, TerminalCursorStyle}` moved, and
  `TerminalCursor::style` is gone. `InputSystems`, `InputCapabilities` and
  `bevy_compat` keep their names: each describes input specifically, which is
  still what it does.

### Fixed

- **Terminal focus changes are reported.** `FocusMessage` was declared,
  registered, and translated from the backend's events, but nothing ever asked
  the terminal to send them - mouse capture enables its own modes and not that
  one - so on a spec-following terminal the message never fired at all. Apps
  watching for focus now receive it.

## [0.4.0] - 2026-08-10

### Added

- **The stylist engine is `plurimus_ui`'s**, so a widget library built on the ui
  pipeline alone gets the machinery the stock widgets use instead of writing it
  again. `StylistCache` records what a widget last drew and
  `StylistCache::redraws` is the compare-and-swap a stylist gates on - it stores
  the next state and answers whether to rebuild, taking the dirty term that
  makes a theme swap repaint, which was previously hand-written at every stylist
  and silently wrong if omitted. `observed` reads an entity's state through
  `StateQuery`, `restyle` runs the loop for label-driven widgets, `Stylable` is
  the filter that honors `StylistDisabled`, and `decorate` and `hashed_bits` are
  the two helpers they need.
- **`content_cell`** turns a pointer cell into a content cell for a widget with
  a scroll offset. It clamps into the area rather than refusing, so a drag
  captured to a widget goes on addressing its nearest cell after the cursor
  leaves it, and is `None` only for an area with no cells at all.
- **`bevy_compat::held_modifiers`** reads bevy's polled key state back into
  `KeyModifiers`, for the focused-input observers that need a chord and cannot
  get one from bevy's own key events. It fills every flag, both sides of each.

### Changed

- **Focus dispatch has a stated position in the frame.** `plurimus_ui` now
  orders it after `bevy_input`'s update and between `UiSystems::Areas` and
  `UiSystems::Hover`, and `UiSystems` documents the guarantee: a `FocusedInput`
  observer reads this frame's `ComputedWidgetArea` and a settled `ButtonInput`.
  Previously that order held only when `WidgetsPlugin` happened to be installed,
  so a widget library built on `plurimus_ui` alone had to rediscover and
  re-assert it. Work that must see what such an observer did now belongs after
  `InputFocusSystems::Dispatch` rather than after `UiSystems::Areas`.
- **`UiLabel` moved to `plurimus_ui`**, with the rest of the drawing vocabulary.
  **Breaking**: `plurimus_widgets::UiLabel` no longer exists - import it from
  `plurimus_ui`, or from `plurimus::ui` on the facade, where `plurimus::widgets`
  used to serve it. Nothing about how a label is written or drawn changed.

### Fixed

- **A horizontally scrolled table resolves a click to the column under it.** Its
  columns are laid out against the content width, but only the vertical axis was
  mapped into content space, so every scrolled-off column shifted the answer by
  one. Unscrolled tables were never affected.
- **A list box and a table page by the height they are actually drawn at.** Both
  read their visible height inside a focused-input observer, which ran before
  the areas were computed, so on the first frame the height was zero and
  `PageDown` moved a single row; after a resize it moved by the previous size.
  The clamp that hid this is what made it a wrong page rather than a stuck
  cursor, which is why it survived unnoticed.

## [0.3.0] - 2026-08-10

### Added

- **`WidgetSystems`**, the set the stock widget systems run in, so an app can
  order its own systems against widget layout or styling instead of guessing.
- **`StylistDisabled`** in `plurimus_ui`, which exempts one entity from a widget
  library's stylists. The widget keeps its behavior - selection, keys,
  scrolling, events - while the app owns what it draws.
- **`UiStyle`** in `plurimus_ui`, a style patched over the one an entity would
  otherwise resolve to. On a widget it composes with hover and focus rather than
  replacing them; on a list row it styles the full row, which is what a striped
  or state-colored list needs.
- **`ListItemText`**, which draws one list row as several terminal rows -
  explicit line breaks only, since the list truncates rather than wraps. A row
  carrying it is as tall as its text has lines, and the list box measures every
  row by height rather than by count: the scroll extent, the row a click lands
  in, and the reveal that keeps the cursor visible all follow the taller row.
  Rows without it are unchanged, and every other labelled widget keeps drawing
  the single-line `UiLabel`.
- **`ListBoxSelectionMarker`** and **`ListBoxCursor`**, so a list box's marker
  column and cursor symbol are the app's to choose. An empty cursor symbol gives
  bar-style selection with no gutter at all. Both are read when the list is
  styled, so removing one from a live list leaves the old gutter until something
  else about the list changes - the caveat a table's band markers already carry.
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
- **`ListBoxKeys`**, the same treatment for a list box: its movement bindings
  are data an app replaces rather than a closed match it has to reimplement
  beside the widget. `ListBoxAction` names what a key does, and the default map
  keeps the arrows, `Home`/`End`, and Enter and space. Bindings are scanned in
  order, so the first entry for a key wins and two keys can share an action.
- **A list box pages** with `PageUp` and `PageDown`, moving the cursor by the
  rows currently visible - keys that previously fell through a focused list.
- **`TableHeaderClick`**, reporting the column whose header was clicked so an
  app can sort. The crate supplies the geometry; the ordering stays with the
  app, which is the only side that knows whether a column is text or numbers.
- **`Key`** is re-exported from `plurimus_widgets`, so an app can name the key
  type `TableKeys` and `ListBoxKeys` hold without depending on `bevy_input`
  directly.
- **A `table` example**: a sortable, scrollable process list showing striping, a
  bolded header, a totals footer, remapped movement keys, and app-side sorting
  driven by header clicks.
- **`CompositeSystems`**, the ordered passes of the composite phase. Its
  `PostProcess` set runs after cameras merge and before color depth is reduced,
  so an app can mutate or read the composed frame while every color is still
  what the widgets chose.

### Changed

- **The theming contract moved from `plurimus_widgets` to `plurimus_ui`**, so a
  widget library that builds on the ui pipeline alone can honor the app's theme
  without depending on the stock widgets. `UiTheme`, `UiStyle`, and
  `StylistDisabled` are now `plurimus_ui` items and `UiPlugin` initializes the
  theme resource; `plurimus_widgets` no longer exports any of the three.
  **Breaking**: `plurimus_widgets::UiTheme` and its two siblings no longer
  exist - import them from `plurimus_ui`, or from `plurimus::ui` on the facade,
  where `plurimus::widgets` used to serve them. Nothing about how a widget looks
  or behaves changed.
- **`UiTheme::resolve` is public**, so a widget outside the stock library
  resolves the same style for the same state instead of reimplementing the
  precedence. It takes the new `InteractionState` - `hovered`, `pressed`,
  `disabled`, `focused` - and applies the documented order: disabled over
  pressed over hovered over normal, with focused patched over the winner.
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
- **A list box costs nothing to leave alone.** It used to collect every row and
  hash all of their text and styles on each frame purely to decide it need not
  redraw, so a long list was a steady per-frame cost that nothing could opt out
  of. A row's edit now reaches its list when it happens, and an idle frame reads
  two components and compares them. Lists of a few hundred rows are the ones
  that notice.
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
- **The `widgets` example's log pane is a focusable multi-line list** rather
  than a paragraph, each entry a heading over an indented detail line, and its
  themed panes now stretch to the width of their half of the terminal with the
  log taking the height left above the status line.

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

[0.6.0]: https://crates.io/crates/plurimus/0.6.0
[0.5.0]: https://crates.io/crates/plurimus/0.5.0
[0.4.0]: https://crates.io/crates/plurimus/0.4.0
[0.3.0]: https://crates.io/crates/plurimus/0.3.0
[0.2.0]: https://crates.io/crates/plurimus/0.2.0
[0.1.0]: https://crates.io/crates/plurimus/0.1.0
