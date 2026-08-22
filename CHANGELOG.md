# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **`SliderKeys`, `MenuKeys` and `TextInputKeys`**, the last three widgets' keys
  as data. Sliders, menus and the single-line field matched their keys inline,
  so an app could not remap them at all; each now holds a bindings component of
  its own with a `Default` binding what it always bound - and `TextInputKeys`
  carries `Submit` among its actions, so a form can move the field's commit to
  `Ctrl+Enter` and keep plain Enter for itself. A menu's table lives on its
  popup, one per menu rather than one per row. Every widget in the crate now
  takes its keys from a component.
- **`HeldModifiers`**, the system parameter every key observer reads the held
  modifiers through, and **`KeyModifiers::none`**, nothing held in a `const`
  context so a binding table can be built at compile time.
- **A click count on `PointerPress`, `Click` and `Pressed`**, so a widget with a
  double-click gesture reads a number instead of keeping its own clock. No
  terminal reports one, so it is synthesized against the same real clock key
  releases are: presses run together while they land on the same widget at the
  same cell within `MultiClickWindow` (500 ms by default, and `Duration::ZERO`
  turns runs off) of one another. A press that reaches no widget - one that
  dismissed an overlay, one a disabled widget absorbed, one over nothing - ends
  the run rather than counting towards it, so the press after it starts at 1.
  The count saturates rather than wrapping, what a long run means being the
  widget's to decide, and `with_count` sets it on either event. It also rides
  the `Pressed` component for the length of the gesture, which is what lets a
  `PointerDrag` observer tell a drag through the second press of a run from one
  through the first - the gesture no event carries a count for. The run itself
  is `plurimus_ui`'s and private: `MultiClickWindow` is the whole of what an app
  sets, and it stays in `plurimus_term` beside `ReleaseTimeout`, read against
  the same clock.
- **`Popover::cell`**, attaching a popover to one cell of its anchor's content
  rather than to the whole of its area - a completion list under a caret, a
  tooltip at a cursor. The cell is named in content space and the anchor's own
  `ScrollOffset` maps it, so an editor says where its caret is once, in the
  component it already publishes it in; a cell scrolled out of view places the
  popover nowhere. Everything else the widget does - the preferred side, the
  mirror when it will not fit, the alignment, the clamp - applies unchanged,
  because a cell anchor is the same placement against a one-cell rect.
- **`Popover::camera`**, the camera a popover draws on and is bounded by, when
  it should not be its anchor's. A menu anchored to a docked one-row strip has
  nowhere to open within that row; naming a full-terminal camera gives it the
  whole screen to be clamped into, while the anchor still supplies the rect. Per
  popover, so a menu stays where its button is while a palette escapes.
- **`Popover::with_cell`, `with_camera`, `with_side` and `with_align`**, one
  `const fn` builder per field, so the type is configured the same way
  throughout rather than half by builder and half by assignment.
- **`TableGeometry::cell_rect`**, where a `Table`'s cells are on screen. A cell
  is text in its row rather than an entity, so it cannot hold a widget; a host
  that wants one there - a field editing a cell in place - floats it over the
  rect, which the table's own docs now say. The rect accounts for the cursor
  gutter, the scroll offset, and the bands, is clipped to what the table shows,
  and is `None` for a cell scrolled out of view or a row with no line left. It
  is the same column solve the click router uses, so what it names is what the
  pointer reaches.
- **`ActivateKeys`**, the `KeyBinding`s that activate a focused `Button`,
  `Checkbox`, or `RadioButton`. All three require it, defaulting to Enter and
  space, so nothing changes until an app replaces it - and replacing it is the
  whole of remapping. Binding space alone is what lets a form keep Enter for its
  submit: a key the widget is not bound to activates nothing and propagates, the
  way one on a disabled widget already did. An empty list turns the keyboard
  path off without disabling the widget, which a click still activates. Menu
  items keep their fixed Enter and space.
- **`PressPassThrough`**, press transparency: a widget carrying it is invisible
  to press hit-testing, so a press lands on whatever it covers. Presses only -
  hover, the wheel, and navigation still see the widget. On a disabled widget it
  restores the fall-through that used to be the default.
- **`PressFocusDisabled`** keeps a press from moving focus while the press
  itself still lands - `Pressed`, drags, and `Click` all arrive. A toolbar
  control can be tab-reachable through its `TabIndex` without a click on it
  taking the keyboard off the editor; the browser's preventDefault-on-mousedown.

- **`plurimus_test::write_focus` and `send_focus`**, the focus half of the
  injection helpers, following the module's queue-only and queue-then-tick
  families.
- **`UiWidget` implements `Default`**, drawing nothing - the value a widget
  holds before its first restyle replaces it. A widget library no longer needs a
  blank of its own, and `#[require(UiWidget)]` can supply one with no suffix.
- **`StylistCache::with_value`** sets the `value_bits` a redraw comparison turns
  on, without the full `StateQuery` tuple `observed` demands. The seam for a
  stylist that resolves its own interaction state - a painter drawing one
  resting style - which could reach the mechanism no other way.
- **`TextInput::handle`, `TextInput::apply` and `TextInput::paste`** apply a
  key, an action, or pasted text to the single-line field's editing state. Every
  entry point was a focused-input observer before, so a host that routes its own
  keys - a command palette typing while a list takes the arrows, a field that is
  not a tab stop at all - had to reimplement the field to drive one, losing the
  grapheme-cluster stepping that is the hard part. `handle` takes the field's
  own `TextInputKeys` beside the key, so a host and the stock observer resolve
  it the same way; `apply` is the step for a host that resolved the action
  itself. The stock observers are callers of the same methods.
- **`Submit`**, an entity event carrying the value a field was submitted with.
  Enter and focus loss both emitted an identical final `ValueChange<String>`, so
  committing an entry could not be told from abandoning one without inspecting
  focus state. Both events still fire on Enter; only `Submit` distinguishes it.
- **`UiTheme::caret`**, the style patched over the character a widget's own
  caret covers, with a `with_caret` builder. It defaults to the reverse video
  the text field previously hardcoded, so an unstyled app looks unchanged.
- **`local_area`**, the inverse of `resolve_area`'s offset: a screen rect
  expressed camera-locally, for storing in a `UiArea::Fixed`. Only the forward
  direction was public, so every crate computing a screen rect it had to store
  rewrote the origin subtraction, with nothing keeping the two in step.
- **`ComputedUiCamera`**, the camera a widget actually draws on, resolved every
  frame in the new `CameraSystems::PropagateCameras`. Read it rather than
  pairing `UiCamera` with `DefaultCamera` by hand.
- **`CameraViewports`**, a system param resolving a camera to its viewport with
  the default-camera fallback applied, so a system placing its own widgets
  states that rule by calling it rather than by repeating it.
- **`Marked`**, a second marker channel for a list row. `Checked` is the
  selection channel `listbox_self_update` writes, so an app marking a row for a
  reason of its own - a command already in force - had to borrow it and hope
  nothing attached that observer broadly. Nothing in the crate writes `Marked`;
  the gutter lights for either.
- **`ListItemTrailing`**, per-row content the list right-aligns against its own
  drawn width. A row is built before the list is placed, so no row builder
  outside the widget can hold the number to align against.
- **`StylistCache::with_focused`**, the seam for a container drawing one part of
  itself as active while keyboard focus sits elsewhere. It keeps any `UiStyle`
  patch, unlike rebuilding the cache by hand.

### Changed

- **`Pressed` carries the click count of the press that set it**, rather than
  being a unit marker. A widget that only asks whether it is pressed is
  unaffected - `Has<Pressed>` and `contains::<Pressed>()` read the same - while
  one inserting it by hand writes `Pressed(1)` or `Pressed::default()`, which is
  the lone press the router used to assume for a gesture it did not start.
- **A key binding carries its modifiers.** `ScrollKeys`, `ListBoxKeys` and
  `TableKeys` hold `KeyBinding`s - a `Key` and the `KeyModifiers` it must be
  pressed under - rather than bare `Key`s, so a list can be bound to `Ctrl+D`
  and a form's submit to `Ctrl+Enter` while the button under it keeps plain
  Enter. Every modifier but shift is matched exactly, the "chorded" a text field
  already refuses to type under; shift is matched exactly for a named key and
  only when asked for on a character, because a shifted symbol carries the bit
  on some terminals and not others - so `G` and `:` are spelled as themselves,
  and `with_shift` is for `Shift+Tab`, the shifted arrows and `Shift+Space`. A
  bare `Key` converts with `.into()`, and every stock default binds what it
  bound before. `first_bound` takes the held modifiers beside the input, and
  `KeyBinding::matches` is the same test for a host routing its own keys. The
  modifiers are the ones held when the key arrived, polled - bevy's
  `KeyboardInput` carries none - which is right for every case but a chord
  landing in the frame its modifier is released.
- **A `Table` with no stated column widths divides its own width**, rather than
  leaving ratatui to divide the area it renders into. The two rules agreed, so
  nothing is drawn differently; what changes is that one solve now feeds both
  the drawing and the click routing, which is what makes `cell_rect` able to
  promise that where it says a cell is, is where the pointer finds it. Such a
  table also redraws when it is resized, its columns having been divided from a
  width that changed.
- **A disabled widget absorbs the press instead of hiding from it.** A widget
  with `InteractionDisabled` was invisible to press hit-testing, so a press on
  greyed chrome was not swallowed but routed to whatever the widget covered. It
  now wins arbitration like anything else and the press stops there: no event,
  no focus movement, nothing beneath pressed - and a widget disabled mid-gesture
  no longer clicks on release. Wheel ticks still fall through a disabled widget,
  consuming being that router's own opt-in, and an open menu is still dismissed
  by a press on disabled chrome outside it. Anything relying on the old
  fall-through opts back in with `PressPassThrough`.

- **A list or table row is selected on release rather than on press**, and
  `Click` now carries the cell it was released on (`Click::new` takes it). Down-
  edge selection despawned the entity the pointer router was still owed a
  release for, which is what selection usually does - close the thing it was
  made in. A drag now names the row it ends on, a release outside every row
  selects nothing, and `TableHeaderClick` moves to the release with the rest. A
  list's press and drag still move the cursor; a table's do not, because its
  cursor gutter exists only while a row is current, so moving the cursor
  mid-gesture would shift the columns the release resolves against.
- **`WidgetCursor::cell` is `Option<Position>`**, so a widget whose caret has
  nowhere to sit can say so. Previously such a widget could only leave the last
  cell standing or remove the component, which discards the style an app set
  and, the component being required, never re-inserts it. `WidgetCursor::new` is
  unchanged, and a `nowhere()` constructor joins it - which is also the type's
  new `Default`, the spelling its sibling `TerminalCursor::hidden` already uses.
- **`restyle` takes the theme by reference plus a `theme_changed` flag** rather
  than `&Res<UiTheme>`, so a caller holding a plain `&UiTheme` can drive it.
- **`LabeledQuery` carries `Ref<UiLabel>`** rather than `&UiLabel`, which is
  what lets the label's own change tick reach the redraw decision.

### Fixed

- **A press inside an open modal overlay no longer closes it.** The guard asked
  only whether the pressed entity carried `ModalityToggle`, so a press on any
  unmarked child - a menu popup's own border included - dismissed every open
  overlay and fell through to whatever sat beneath it. "Inside" is now the
  overlay's own rect, which is what the wheel path always used: a press an
  overlay covers reaches that overlay's subtree or nothing at all, and only a
  press outside every open overlay dismisses. A press outside them on a
  `ModalityToggle` still routes, which is how an opener closes what it opened.
- **A wheel tick inside an open modal overlay scrolls the overlay's own
  content.** Every tick an overlay covered was swallowed, so a scrollable list
  inside a dialog could not be wheel-scrolled at all. Arbitration now runs with
  the candidates narrowed to the subtrees of the overlays under the pointer,
  which keeps the reason such ticks were swallowed - nothing an overlay covers
  may move - while letting the overlay's own content use them.
- **A container driven through `ActiveDescendant` shows its cursor row.** Both
  stylists resolved the cursor's style from the container's own focus, so a list
  stepped by a search field beside it - the case `ActiveDescendant` exists for -
  painted its cursor in the resting style: an invisible highlight.
- **A cursor whose row is gone re-points instead of dangling.** Filtering a list
  is despawning its rows and spawning new ones, and nothing repaired
  `ActiveDescendant` afterwards, so the cursor named a dead entity - it
  highlighted nothing and moved from nowhere. It now re-points to the first
  surviving row, or to none when none survives; a deliberately empty cursor
  stays empty.
- **The cursor row is scrolled into view whoever moved it.** Only the
  container's own key handler revealed, so a click, a rebuild, or an app driving
  the list from elsewhere scrolled nothing - including the crate's own press
  path. The reveal now follows the cursor.
- **A widget with no camera of its own follows its nearest ancestor's.** It fell
  straight to the default camera before, so a child of a widget on a dedicated
  camera drew on the wrong one unless every spawn site remembered to copy the
  parent's - a silent misplacement, and one that only appears in the
  multi-camera apps that need cameras at all. A widget carrying its own
  `UiCamera` is unaffected, and so is one whose ancestors carry none.
- **Only a focused text field draws its caret.** Every `EditableText` on screen
  painted a block at its cursor, so a form of eleven rows claimed eleven of them
  had the keys. The stylist already resolved the focus bit and dropped it on the
  way to the drawable.
- **A chorded character no longer types itself into a field.** `EditableText`
  read `Key::Character` with no modifier guard, so ctrl+c inserted a literal `c`
  while the same chord copied in the multi-line editor one module over. A
  character held with anything but shift is now left for whoever binds it -
  shift excepted, because the kitty protocol reports a shifted letter with the
  bit set and capitals must still reach the field.
- **Holding Enter in a field submits once.** A terminal autorepeats a held key
  many times a second and each repeat emitted a final `ValueChange<String>`,
  committing one intent as many times over; the activation path already ignored
  repeats for this reason. The key is still consumed on a repeat, being the
  field's own.
- **A widget repaints when its label changes.** The stylist cache compares
  interaction state, which an edited `UiLabel` leaves untouched, so a button,
  checkbox, radio button, menu item, or pane whose text was set after its first
  paint kept drawing the old text - a pane could never be retitled at all. The
  stylists now fold the label's change tick into the same dirty flag a theme
  swap uses. An untouched widget still costs a comparison and no rebuild.
- **An entity handed back from `StylistDisabled` repaints.** Change detection
  compares against a system's last run, so an entity that sat outside every
  stylist query missed the theme changes that landed meanwhile and kept a stale
  drawing when the app gave it back. Removing the component now resets its
  `StylistCache`, making take-over and hand-back a contract rather than a
  caveat.

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
