# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Upstream `bevy_ui_widgets` run on `bevy_ui` nodes.** `BuiPlugin` installs
  bevy_ui's picking backend beside the fontless `bevy_ui_widgets` plugins
  `UiPlugin` installs, on the mouse pointer the terminal reports to
  `bevy_picking`, so bevy's own `Button`, `Checkbox`, `RadioGroup`, `Slider`,
  menus, dialogs and `ScrollArea` work on a node tree unchanged - hover, press,
  click, click-to-focus, Enter and Space, and wheel ticks that scroll one row.
  - Upstream's `TextInput` is a text field here too, so a tree shared with a
    window spawns one field for both. Upstream's own observers decide what each
    key and press means, and the edits they queue apply to plurimus's text
    engine instead of a font engine, so `EditableText::value`, `clear` and
    `set_text` answer as on a window. The field draws one row in its
    `TextColor`, windowed to keep the caret in view, with the caret and
    selection in its `TextCursorStyle`'s colours where it carries one and
    `UiTheme`'s otherwise, and the terminal's own caret sits on it while it
    holds focus. A press or drag lands on the cluster drawn in its cell at any
    `CellSize`; `max_characters` holds, and `visible_width` and `visible_lines`
    size the node in cells. Copy, cut and paste use the terminal's clipboard,
    and a bracketed paste types into the focused field. `EditableTextFilter` is
    not applied, being unreadable outside `bevy_text`, and the field takes no
    part in arrow navigation, as no node does. A native macOS build takes
    upstream's command key to be Cmd, which no terminal delivers, so there the
    field's select-all and clipboard chords do nothing and only a bracketed
    paste pastes; the cell `TextField` binds Ctrl everywhere.
  - Widget hits are ordered above node hits and block what is beneath, so a
    plurimus widget drawn over a node takes the press and the hover alone; a
    tick over a widget that does not scroll on the ticked axis reaches no node
    beneath it. While a plurimus overlay is open, only the nodes of its own
    subtree are picked inside it - an overlay built of nodes works, and one
    drawn over nodes covers them - and nothing is picked outside it, where a
    press or a tick dismisses it.
  - A dialog dragged by its `DialogDragHandle` stays on the terminal, since
    upstream keeps the dragging pointer 16 px inside the window and the window
    now has the terminal's size. That margin is sixteen cells at the one-pixel
    `CellSize` default, so an app dragging dialogs names its cell.
  - `bevy_ui` nodes are picked, stacked and painted in `UiStack` order, so
    `ZIndex` and `GlobalZIndex` order both, and a root spawned later stands over
    an earlier one, as in a window.
  - A node beneath a widget in the tree draws directly after that widget, in its
    band, rather than beneath every widget: a `Text` inside the frame an
    overlay's `UiWidget` clears is readable, so a tree shared with a window
    needs no second copy of the string. A node under no widget still draws
    beneath every widget, and a node carrying a widget of its own is not lifted,
    so a label mirrored on a button's entity stays beneath the button.
    `ExtractedWidget` gains the `layer` that orders this: the widget sort is
    `(order, source, layer)`, and a pipeline drawing what a widget frames spawns
    its own at the widget's source and `with_layer(1)`.
- **`bevy_ui` text draws in a bridged tree.** `bevy_ui::widget::Text` roots with
  `bevy_text::TextSpan` children render in cells: `TextColor` is the foreground,
  a weight from semibold up is bold, a non-normal style italic, `Strikethrough`
  and `Underline` their modifiers, `TextLayout`'s justify and
  `LineBreak::NoWrap` honoured. `font_size`, fonts, `TextShadow` and the
  underline and strikethrough colours are ignored; text is one row tall. A
  `Color::NONE` text draws nothing.
- **`Outline`** draws as a one-cell ring outside the node's border box, at its
  rounded offset.
- **`Visibility` hides a node subtree.** `BuiPlugin` propagates it into
  `InheritedVisibility`, so a hidden node is neither drawn, picked nor reached
  by Tab, and a `Visibility::Visible` child of a hidden parent shows.
- **`plurimus_core::CellSize`**, pixels per cell on each axis, default one by
  one. A tree sized for a window - `8 x 16` puts a 560 px card in 70 columns and
  a 16 px control in one row - lays out the same in the terminal once the cell
  is named; the camera mirror, layout, text measurement, decoration, picking,
  `RemSize`, the 2d projection and the virtual window all follow it.
- **`examples/todo`**: one `bevy_ui` tree of upstream widgets tagged by `Part`,
  run in the terminal or, with `--desktop`, in a bevy window assembled from the
  granular crates. Each target skins the same tree - cells and box-drawing in
  one, pixels and a card in the other - with a `Display::Grid` ledger of
  unwrapped, right-justified cells. The field is upstream's `TextInput` on both
  targets, committed on Enter and from the Add button through `EditableText`.

### Changed

- **Breaking: the single-line text engine is `plurimus_ui`'s, and the cell field
  is `TextField`.** `TextInput`, `TextInputKeys` and `TextInputAction` move to
  `plurimus_ui`, which `plurimus_widgets` re-exports, so every host of a
  single-line field edits through one engine. `TextInput::apply`,
  `select_word_at`, `copy_selection`, `row` and `char_at_cell` are public, with
  the `TextRow` renderer and `DrawnRow` it draws, and the `unbound_text`,
  `cluster_len_before` and `cluster_len_after` rules a multi-line editor shares.
  `plurimus_widgets`' `EditableText` is renamed `TextField` and `editable_text`
  is `text_field`, so no plurimus name shadows `bevy_text`'s `EditableText`.
  `plurimus_term::Clipboard` is the public system parameter every copying widget
  writes through.
- **Breaking: the terminal render sub-app is built by `bevy_extract`.**
  `plurimus_core` depends on `bevy_extract` and re-exports it; its own world
  swap is gone, and `ExtractSchedule`, `MainWorld`, `MainEntity` and `Extract`
  at its root are bevy's, so an extract system reads the main world through
  `Extract<Query<..>>`, whose query state persists across frames, or through
  `Res<MainWorld>` where it must write. Every extract system in the workspace
  but the web backend's two now takes `Extract`.
  - `TerminalRenderSystems` gains `ExtractCommands`, where what extraction
    spawned and inserted is applied, and `Cleanup`, where the frame's extracted
    entities are despawned, around the three phases.
  - `ExtractedCamera` and `ExtractedWidget` require bevy's `TemporaryEntity`, so
    they live for one frame's `TerminalRender` and a pipeline spawning its own
    despawns nothing; after `app.update()` the render world holds none.
  - `TerminalSize`, `ColorDepth`, `TerminalCursor` and `DefaultCamera` are
    `ExtractResource`s, copied into the render world when the main world's copy
    changes.
  - An extracted camera carries bevy's `MainEntity` in place of `SourceCamera`,
    `camera_buffer_mut` takes `Query<(MainEntity, &mut CameraBuffer)>`, and
    `ExtractedWidget.source` is a `MainEntity`; `ExtractedWidget::new` still
    takes the source `Entity`.
- **Breaking: a terminal camera is a bevy `Camera`.** `plurimus_core` depends on
  `bevy_camera`, and `TerminalCamera` requires `Camera` and is now the placement
  alone - `Full`, `Fixed(rect)`, `Docked { edge, cells }` or `Fill`, resolved
  into `ResolvedViewport` as before. Compositing order, activity and what
  untouched cells become are `Camera`'s `order`, `is_active` and `clear_color`:
  `Default` keeps the terminal's own background, `None` lets lower cameras show
  through, and `Custom(color)` pre-fills the view with that colour, downsampled
  like every other cell colour, so a clear can no longer name a terminal palette
  entry. Core mirrors each camera's resolved rect and the terminal size into
  `Camera.viewport` and its target info in `CellSize` pixels, written only when
  they change, for every camera that is not a `Camera3d`, and fits a
  `Projection` the camera carries to that view, setting its `clip_from_view` as
  bevy_render would. A `Camera3d` is left to bevy_render's own writer, which
  sizes it from its image. That also fixes a 3d camera in a bridged app
  receiving a cell-pixel viewport from the bridge that bevy then clamped into
  the image. `plurimus_core` re-exports `bevy_camera`. In an app with
  bevy_render installed, a terminal camera that is not also a `Camera3d` logs
  bevy_render's "no render graph" warning once at spawn, which no terminal build
  has a subscriber for; a projected one, such as a 2d camera, is sized by
  bevy_render as well, against the virtual window, which has the terminal's
  size, so the two agree.
- **Breaking: a 2d view is bevy's `Camera2d`.** `plurimus_2d` draws for every
  terminal camera carrying `Camera2d`, projected through its
  `OrthographicProjection` by `Camera::world_to_viewport`: the camera's
  translation is the view's centre, its rotation and scale now shape the view
  too, and zoom is the projection's `scale` in world units per pixel. A cell is
  core's `CellSize` in pixels, so an app drawing 2d sets
  `CellSize { x: 1.0, y: 2.0 }` for a world unit to be a column wide and half a
  row tall, the shape the old default drew. A sprite beyond the projection's
  near or far plane, `-1000` to `1000` by default, is not drawn. A change to the
  projection's `scale` made in `Update` draws from the next frame, since core
  fits the projection in `PreUpdate`. `RenderLayers` is `bevy_camera`'s, still
  named `plurimus_2d::RenderLayers`: `Clone` rather than `Copy`, with `usize`
  layers and `intersects` taking a reference.
- **Breaking: plurimus speaks bevy's interaction vocabulary.** `plurimus_ui`'s
  `Hovered`, `Pressed`, `Checked`, `InteractionDisabled`, `Activate` and
  `ValueChange` are now bevy's own types, re-exported: `Hovered` from
  `bevy_picking`, the three markers from `bevy_ui`, the two events from
  `bevy_ui_widgets`. Imports keep working; what changes is their shape.
  - `Pressed` is a unit marker. The click count it carried is bevy's, on
    `PointerPress::count` and `PointerClick::count`.
  - `ValueChange { source, value, is_final }` replaces `ValueChange::new` and
    carries bevy's `is_final`, which plurimus widgets set to `true`.
  - `Hovered` is a component holding a `bool`, written by `bevy_picking` to the
    topmost widget under the pointer and its ancestors - so a widget must carry
    it to be picked at all, and `WheelReceptive` requires it.
- **Breaking: `bevy_ui` nodes interact through `bevy_picking` alone.** The
  bridges that published node rects as plurimus widget areas and wheel targets
  are gone: a node is hovered, pressed, focused and scrolled by bevy's stack,
  and a scrollable node is a `bevy_ui_widgets::ScrollArea`, not an `Overflow`
  alone. Nodes no longer join plurimus_ui's directional navigation map; Tab
  reaches them as before. Plurimus widgets placed at node rects keep doing so
  through `UiArea`.
- **Breaking: `plurimus_bui`'s `Text`, `TextSpan` and `TextStyle` are removed**
  in favour of bevy_ui's text, above. A bridged tree's text is
  `bevy_ui::widget::Text`.
- **A border with one side draws straight.** A top-only border runs end to end
  and a left-only one the full height; a corner glyph appears only where both of
  its sides are present.
- **Breaking: plurimus widgets are picked by `bevy_picking`.** `plurimus_ui` is
  a picking backend: each frame it reports the widgets under the pointer, per
  terminal camera, and bevy's hover map, pointer capture, pointer events and
  click counts replace the crate's own router. A widget observes bevy's
  `PointerPress`, `PointerDrag`, `PointerRelease`, `PointerClick` and
  `PointerCancel`, named through `plurimus_ui::bevy_picking`; `pointer_cell`
  reads the cell under any of them, `click_cell` the cell a click ended on when
  that is inside the widget, and `PointerModifiers` the modifiers the terminal
  sent with the frame's mouse input, which bevy's events have no field for. A
  press captures the pointer to the widget it lands on, so its drag and release
  reach it wherever the pointer goes; a focus loss sends that widget a
  `PointerCancel`. `UiPlugin` installs `PickingPlugin`, `InteractionPlugin` and
  `PointerInputPlugin`; a frame's batch ends at every press and release, so what
  follows is hit-tested against the state the click left.
  - Click counts are per widget on the wall clock, within
    `PickingSettings::multi_click_interval`: a second press anywhere on the same
    widget runs with the first, and a press on another widget starts over.
  - A press on a widget with no `TabIndex` at or above it clears focus, as a
    press on nothing does; `PressFocusDisabled` and an open overlay's root keep
    it, as before.
  - A disabled widget is still picked, so it blocks what is beneath and is
    hovered while it does; no gesture starts on it. A widget that should be
    transparent to the pointer carries bevy's `Pickable::IGNORE`, which replaces
    `PressPassThrough` and removes its hover too.
  - Outside every open overlay the one thing picked is a barrier, so a press or
    a tick there dismisses the overlays and reaches nothing - a node beneath
    included - and an enabled `ModalityToggle` is picked in its place.
  - A right or middle press is a press, as it is on a node; the old router took
    the left button alone.
- `plurimus_ui` re-exports `bevy_picking` and `bevy_ui_widgets`; `plurimus_bui`
  re-exports `bevy_text` and `bevy_ui_widgets`.
- **Breaking: buttons, checkboxes and radio groups are `bevy_ui_widgets`' own.**
  `plurimus_widgets`' `Button`, `Checkbox`, `RadioButton`, `RadioGroup`,
  `checkbox_self_update` and `radio_self_update` are upstream's, re-exported,
  and upstream's observers drive them on a cell widget as on a node: a click
  that ends on one, or Enter or space while it holds focus. `UiPlugin` installs
  the fontless upstream plugins and `UiScale`, so a ui-only app has them too.
  The `button`, `checkbox` and `radio` bundles carry `Hovered` and
  `StylistCache`, which upstream's markers do not require: a bare marker on a
  `UiWidget` is neither picked nor styled until the app adds them.
  - Enter and space are fixed and matched by physical key with no modifier
    check, so Ctrl+Enter activates as Enter does, and no key is remapped.
  - The radio group is the tab stop. `radio_group()` spawns it with a `TabIndex`
    and a widget that draws nothing, to which the app gives a `UiArea` spanning
    its options; `radio()` is no tab stop. A press on an option checks it and
    focuses the group, arrows, Home and End move the check and wrap, and the
    checked option draws focused while the group holds focus. Checking the
    checked option again emits nothing.
  - These three are not captured on press: a release off one reaches what is
    under the pointer, so dragging off and letting go activates nothing, as on a
    window. A release landing in the same frame as the move off still clicks. A
    focus loss with nothing captured sends its `PointerCancel` to every pressed
    widget, so one of these three pressed at the time is released.
  - Their press, release and click stop at the widget, as upstream's observers
    stop them.
  - A checkbox press focuses the checkbox whatever `PressFocusDisabled` says,
    and a disabled checkbox too, since upstream's handler sets focus itself.
  - A disabled container writes `InteractionDisabled` onto every descendant,
    since upstream's observers check the entity alone, and takes back only what
    it wrote; an app's own flag on an entity already flagged that way goes with
    its ancestor's.
  - A press or a click inside a `Popover` goes no higher than it, so a menu
    parented to its button does not press or activate the button.
- **Breaking: sliders, list boxes, tab bars and menus are `bevy_ui_widgets`'
  own.** Their types, keys and models are upstream's, re-exported from
  `plurimus_widgets`, and the constructors carry what a cell widget is drawn and
  picked with. What stays plurimus's is the drawing, the placement, and the
  pointer routing a cell rect needs where upstream's measures a node.
  - The slider is upstream's `Slider`, `SliderValue`, `SliderRange` and
    `SliderStep`, with `SetSliderValue`, `SliderPrecision` and
    `slider_self_update`. The side arrows step and repeat while held, Home and
    End jump, and a bound arrow at the range's edge is consumed and reports an
    unchanged final value rather than passing on. The default step is 1. The
    value components are immutable, so an app inserts a new one. `slider()`
    takes its bounds in either order; a reversed `SliderRange` inserted by hand
    warns and panics upstream's way.
  - The list box is upstream's `ListBox`, `ListItem`, `ListBoxMultiSelect` and
    `ActiveDescendant`, the cursor the table shares, now immutable and written
    by insert. Selection is `bevy_ui`'s `Selected`, re-exported from
    `plurimus_ui`, and `listbox_update_selection` is the stock single-select
    updater. The arrows wrap, Left and Right move too, there are no page keys,
    and no key repeats. Enter, space or a click on the selected row sends
    `ReselectListRow`, and a disabled row takes the cursor and selects nothing.
    A disabled list's arrows still move its cursor. The list's own Enter repeats
    propagate to its ancestors. Losing focus clears the cursor, and gaining it
    restores the selected row or the first. A press or drag still moves the
    cursor under the pointer, and a click selects through upstream's
    `SetSelected`. Copy stays, on a fixed Ctrl+c.
  - The tab bar is upstream's `TabList` and `Tab`. `TabBar` is a marker that
    requires a `TabList`, so `(tab_bar(), TabList { .. })` changes its
    orientation or activation, `Manual` by default. Focus sits on a tab with a
    roving `TabIndex`. The arrows along the bar move it and wrap, Enter or space
    selects, and `Automatic` selects as focus moves. Selection is the list's
    `SelectedTab`, mirrored onto its tabs as `Selected` the frame after, and a
    change is a `ValueChange<Option<Entity>>` on the list that
    `tablist_self_update` applies. `TabBarLook` loses `orientation`, the layout
    and joins reading `TabList::orientation`. A press on a tab focuses the tab,
    so `PressFocusDisabled` goes on the tabs rather than the bar.
  - Directional navigation leaves a negative `TabIndex` out, as Tab does, so a
    roving strip is one arrow target at its resting tab.
  - A menu is upstream's `MenuButton`, `MenuPopup` and `MenuItem`, opened and
    closed through the bubbling `MenuEvent`. A plurimus observer answers it for
    a popup holding a `Popover`, so a menu of nodes still answers its own. The
    button opens on the press, and Up or Left on it opens at the last item. The
    open popup takes focus onto an item and closes when focus leaves it, so a
    press outside closes the menu and still acts on what it hit, and a wheel
    tick outside no longer closes it. Tab cycles the items, Escape refocuses the
    button, and the keys are fixed. The popup no longer carries `ModalOpen`. It
    is a pick: a press on its frame keeps focus inside, and a tick over it
    scrolls nothing beneath.
  - Tabs and menu items are not captured on press, as buttons are not, so a
    release off one is no click on it.
  - `Scrollbar` and `scrollbar()` take `ControlOrientation`; the bar fills the
    strip it is given.
- **Breaking: input is bevy's own messages.** A backend reports what its
  terminal said through `plurimus_term::TerminalInput`, and an app reads bevy's
  `KeyboardInput`, `MouseButtonInput`, `MouseWheel`, `CursorMoved` and
  `WindowFocused`, written for the primary window - where the app opened none,
  the `VirtualWindow` `TermPlugin` spawns, kept at the terminal's size in
  `CellSize` pixels with a `WindowResized` for each change - as `bevy_winit`
  would write them, each also in the ordered `WindowEvent` stream.
  `PasteMessage` and `TerminalResized` stay, having no bevy counterpart.
  - A key names bevy's `Key` and the `KeyCode` `physical_code` infers from it: a
    letter or digit is its own key whatever its case, a named key its namesake,
    a modifier its left side. `KeyboardInput::text` carries a character press.
    Modifiers are not on the message: `HeldModifiers`, now at `plurimus_term`'s
    root, polls them from `ButtonInput<KeyCode>` for a key observer, and
    `PointerModifiers`, now `plurimus_term`'s too, keeps the ones the terminal
    sent with the frame's button press. On a terminal that sends modifiers only
    as bits, the sink writes the modifier key presses and releases they imply,
    so a shift-click holds `ShiftLeft` there as well.
  - The pointer sits at the cell's centre in `CellSize` pixels; a `CursorMoved`
    is written only when the cell changes, and a wheel notch is one cell in
    pixels, so a `bevy_ui` node scrolls one row or column per notch. A press or
    a release still ends the frame's batch for pointer input, now in the sink
    rather than in `plurimus_ui`, which runs `bevy_picking`'s
    `PointerInputPlugin` - mouse on, touch off - instead of a feed of its own.
  - Input runs in `First`: `InputSystems::Pump`, then `Synthesize`, after bevy's
    clock and message swap and before `bevy_picking` reads the frame's events.
    `InputSystems::Update` is gone; polled `ButtonInput<KeyCode>` and
    `ButtonInput<MouseButton>` are fed by `bevy_input`'s `InputPlugin`, which
    `TermPlugin` installs.
  - The sink owns the write to `InputCapabilities`: a backend raises what its
    terminal has proven through `TerminalInput::learn`, and a pump reading the
    resource beside the sink conflicts with it.
  - A focus loss releases every held key with a `KeyboardFocusLost` beside the
    releases, drops every held button from `ButtonInput<MouseButton>` without
    writing a release, and cancels the captured widget's gesture through bevy's
    `PointerInput::Cancel`, which `plurimus_ui` writes.
  - `plurimus_web` copies to the browser clipboard through `bevy_clipboard`.
  - `plurimus_test`'s key helpers take bevy's `Key`, `character('a')` naming a
    typed character, and its mouse helpers are named: `move_to`, `press_at`,
    `release_at`, `click`, `scroll_at`, `scroll_sideways` and `send_press_at`
    tick, the `write_*` forms queue, and `write_press_at_with` carries
    modifiers. A chord is `press_key_with`; a drag is a move while a button is
    held.
- **Breaking: hiding and camera naming run on bevy's hierarchy propagation.**
  `plurimus_core` installs `bevy_app::HierarchyPropagatePlugin` for both. A
  subtree is hidden by `Propagate(Hidden)` on its root, which lands `Hidden` on
  the root and everything beneath it and takes it back when removed; `Hidden` is
  what every reader reads, in the frame it was hidden, a mid-frame hide
  included. A camera is named by `Propagate(UiCamera(camera))`, which lands
  `UiCamera` the same way, a nearer one winning; `ComputedUiCamera` is that or
  the default camera, resolved once a frame and never removed. A bare `UiCamera`
  on an entity nothing propagates to still names its own camera, so a leaf spawn
  needs no change, but a parent that carried its children with a bare `UiCamera`
  now needs the `Propagate` form. `plurimus_core` re-exports `Propagate`.
- **A pane asks bevy whether focus is within it.** Its stylist reads
  `IsFocusedHelper::is_focus_within`; nothing marks ancestors any more.
- **The wheel router measures a `ScrollArea` at the tick.** Whether a receptive
  widget can use the ticked axis is read from its `ScrollArea`'s content against
  its viewport when a tick arrives; a receptive widget without one claims both
  axes, as the default `WheelAxes` had it. `ScrollArea::max_offset` is that
  measure, public.
- **Colour averages are `LinearRgba` arithmetic.** `raster::linear_average`
  folds sRGB byte samples through `bevy_color` and hands back the linear mean.

### Removed

- `plurimus_core::SourceCamera`, for bevy's `MainEntity`.
- `plurimus_2d::Projection2d`, for bevy's `Camera2d` and
  `OrthographicProjection`, and plurimus's own 64-layer `RenderLayers`, for
  `bevy_camera`'s.
- `plurimus_core::Viewport` and `Background`, and `TerminalCamera`'s
  `with_order`, `with_viewport`, `with_active` and `with_background`: the
  placement is the camera itself and the rest is bevy's `Camera`.
  `ExtractedCamera` carries `clear_color` in place of `background`.
- `plurimus_bui`'s camera mirror and its own `CellSize`; the bridge keeps only
  the `IsDefaultUiCamera` toggle and reads core's `CellSize`.
- The `InheritedVisibility::VISIBLE` stand-in `BuiPlugin` registered for nodes;
  visibility is now computed.
- `plurimus_ui`'s `Click`, `PointerPress`, `PointerDrag`, `PointerRelease`,
  `PointerCancel`, `PressCount`, `PressPassThrough`, `PointerTargets` and
  `UiSystems::Hover`, and `plurimus_term`'s `MultiClickWindow`: the pointer
  events, the click run and the hit test are `bevy_picking`'s.
- `plurimus_term`'s `KeyMessage`, `KeyCode`, `KeyKind`, `ModifierKey`,
  `MouseMessage`, `MouseKind`, `CursorCell`, `FocusMessage`, `bevy_compat`,
  `InputSystems::Update` and `KeyModifiers::holds`; `plurimus_test`'s
  `send_mouse`, `write_mouse`, `drag_to` and `press_chord`. Every input type is
  bevy's, above.
- `plurimus_core::UiHidden` and `ComputedHidden`, now `Propagate<Hidden>` and
  `Hidden`; `plurimus_core::raster::LinearColorSum`, now `linear_average`; and
  `plurimus_ui::FocusWithin`, `WheelAxes`, `NavigationConfig`, `ScrollKeys` and
  `ScrollAction`. A focused `ScrollArea` no longer scrolls from the keyboard by
  itself: an app binds the keys it wants in a `FocusedInput<KeyboardInput>`
  observer and triggers `ScrollBy`. The navigation map's rebuild from widget
  geometry can no longer be switched off; hand-made edges survive it as before.

- `plurimus_widgets::ActivateKeys`: a button, checkbox or radio answers to
  upstream's fixed Enter and space.
- `plurimus_widgets`' `SliderKeys`, `SliderAction`, `ListBoxKeys`,
  `ListBoxAction`, `TabBarKeys`, `TabBarAction` and `MenuKeys`: those widgets
  answer to upstream's fixed keys. `TabBarOrientation`, `TabItem` and
  `TabBarLook::with_orientation`, now `TabList` and `Tab`; `MenuOpen`, now
  `MenuFocusState`; and the menu's own `MenuAction`, now upstream's, whose
  variants are its event's. `listbox_self_update` and `tab_bar_self_update`, now
  `listbox_update_selection` and `tablist_self_update`; a list has no stock
  multi-select updater.

### Fixed

- **`plurimus_ui` adopts the app's window.** The focus stack's virtual
  `PrimaryWindow` is spawned only where no entity carries `PrimaryWindow` at
  startup. An app that opens a real window through `WindowPlugin` keeps that one
  window, and arrow-key navigation observes it instead, so a plurimus_ui app can
  run beneath `bevy_winit` without a second window opening.

## [0.9.0] - 2026-10-27

### Added

- **`Val::Em` and `Val::Rem` in `bevy_ui` trees.** bevy 0.20's new font-relative
  units each resolve to one cell under `BuiPlugin`, since layout runs at one
  pixel per cell and text is one row tall. `Val::Em(2.)` is two cells.
  - An app can rescale both through the `RemSize` resource.
  - Every node's `EmSize` follows the rem, because bridged text has no font size
    to derive one from.

### Changed

- **Breaking: plurimus is built on bevy 0.20.** Every bevy crate an app adds
  alongside has to move to 0.20 too.
  - Shape primitives such as `Cuboid` and `Sphere` now come from `bevy_shape`,
    not `bevy_math::primitives`.
  - A `Camera3d` with `Tonemapping::None` now logs a warning unless it also
    carries `DebandDither::Disabled`, since dither has no effect without a
    tonemapper.
- **Breaking: the minimum supported Rust version is 1.97.1.** bevy 0.20's shader
  compiler requires it.
- **`bevy_ui` text wraps to its own content box.** A text node no longer widens
  its parent past the viewport to fit an unwrapped line. Its minimum width is
  its longest word, as in CSS.
  - A `height: 100%` node whose wrapped text needs more rows now grows to fit
    them rather than dropping the extra lines. This follows bevy 0.20's layout
    engine.
- **`bevy_ui` nodes stay reachable by Tab.** bevy 0.20's tab navigation skips
  entities that are not inherited-visible. Nothing computes that for a bridged
  tree, so `BuiPlugin` makes nodes visible by default. As before, `Visibility`
  hides nothing in a bridged tree unless the app runs bevy's visibility plugin.

## [0.8.0] - 2026-09-30

### Added

- **Browser demos.** The `ratman` and `lander` examples also build for
  `wasm32-unknown-unknown` with the `web` feature, drawing through `WebPlugin`
  where the terminal build uses crossterm. `examples/` serves them as a static
  site under a landing page, published at
  <https://kenianbei.github.io/plurimus/> by a Pages workflow run by hand.
  Before downloading a demo, its page checks for WebGL2, and for the lander a
  hardware WebGPU adapter. When a demo can't run or stops, the page says why and
  links back instead of leaving a blank canvas, and quitting returns to the
  landing page.
- **`InputCapabilities` compares.** It implements `PartialEq` and `Eq`.
- **`ScrollArea::viewport`.** The part of an area a `ScrollArea`'s content is
  drawn into, once its scrollbars have taken their column and row. Pass it to
  `max_offset` in place of the whole area.
- **More `plurimus_test` injection helpers.** `set_focus` gives an entity input
  focus without ticking. `press_at` and `write_press_at` move to a cell and
  press there, and `release_at` and `write_release_at` release. `send_paste` and
  `write_paste` deliver a bracketed paste. Each follows the module's families:
  `write_*` only queues, and the rest tick.
- **`InputSystems::Synthesize`.** The input no terminal sends (releases for
  expired holds and for every key held across a focus loss, and that focus
  loss's pointer cancel) is written in a set of its own, chained between `Pump`
  and `Update`, so a system can order against it. `Update` now derives state
  only, and every system in it sees the frame's synthetic input.
- **Pointer cancel.** `MouseKind::Cancel` ends every pointer gesture without a
  release, and `plurimus_term` writes one when the terminal loses focus.
  `ButtonInput<MouseButton>` drops every held button on it, and `plurimus_ui`
  sends the pressed widget a `PointerCancel` in place of `PointerRelease` and
  `Click`. A slider whose drag is cancelled emits a final `ValueChange` at its
  current value.
- **`bevy_input_focus` re-exported.** `plurimus_ui` and `plurimus_widgets`
  re-export it, so `InputFocus`, `TabIndex`, `FocusedInput` and
  `InputFocusSystems` are reachable without a dependency pinned by hand.
- **`ComputedHidden` and `ComputedDisabled`.** Markers on every entity that
  `UiHidden` or `InteractionDisabled` reaches, its own or an ancestor's through
  `ChildOf`. The authored markers still mean "set here"; read the computed ones
  to ask whether an entity is hidden or disabled. `ComputedHidden` resolves at
  the start of the frame and again in `Last`, and `ComputedDisabled` in
  `UiSystems::Areas`, so disabling a widget after that takes effect from the
  next frame.
- **`TerminalRenderAppExt::terminal_backend`.** Reads the backend a
  `PresenterPlugin` draws through back from the `App`, which is how a headless
  app checks what it presented. It returns `None` when no presenter for that
  backend type is installed, where reaching into the render sub-app by hand
  panicked.
- **The terminal's title handed back on exit.** `plurimus_crossterm` saves the
  terminal's title when it takes over and restores it on exit or panic, so a
  title set through `TerminalRequest::SetTitle` no longer outlives the app. It
  uses the terminal's own title stack, since the escape that reads a title back
  is widely disabled as a data-exfiltration risk; a terminal without a stack
  ignores both sequences and keeps the app's title, as before.
- **`TextMask`, for a password field.** An `EditableText` carrying
  `TextMask(char)` draws one of that glyph per grapheme cluster in place of its
  text, with the caret on the glyph its cursor sits at. Only the drawn row
  changes: `TextInput`, `ValueChange` and `Submit` still carry the plaintext,
  and editing is unchanged. A double-width glyph takes two cells. The multi-line
  `TextEditor` masks through its engine's own `set_mask_char`.
- **The terminal's cursor sits on a focused text field.** `EditableText` now
  requires `WidgetCursor` and publishes its caret's cell there, masked or not,
  so the terminal's own cursor, the one a screen reader follows and an input
  method anchors to, is placed on the focused field. An app with a focused field
  therefore shows the terminal cursor where it showed none before. The drawn
  caret stays, so a terminal with its cursor hidden still shows where typing
  goes. The terminal cursor is a steady bar, because a block cursor drawn by
  inverting its cell would cancel the reversed caret. An app can set another
  shape with `WidgetCursor::with_style`, which the field keeps, and the
  terminal's own shape returns when the field loses focus. Adding
  `StylistDisabled` clears the cell, leaving the caret to the app that now draws
  the row. The multi-line `TextEditor` still draws only its engine's caret.
- **`TextEditorKeys`, to remap the multi-line editor.** `TextEditor` now
  requires a table of `(KeyBinding, TextEditorAction)` pairs, scanned
  first-match-wins like every other widget's keys. An action names one of the
  engine's operations. Moving and selecting carry the engine's own `CursorMove`,
  and scrolling its `Scrolling`, so any motion it has can be bound. The rest are
  newline, tab, the character, word and line deletions, undo, redo, yank from
  the kill ring, copy, cut, paste, select all, and cancelling the selection. An
  unbound unchorded character still types itself.

- **A selection in the single-line text field.**
  - **Keys.** `Shift` with the arrows, `Home` and `End` selects, `Ctrl+Shift`
    with the arrows selects by word, and `Ctrl+a` selects everything. A plain
    motion ends the selection and steps from the cursor, as the multi-line
    editor does.
  - **Mouse.** A click places the caret where it lands, a drag selects from the
    press, a shifted click extends from the caret, a double click selects a
    word, and a triple click selects everything. A double click on a masked
    field selects everything, since a word would show where the hidden value
    breaks.
  - **Editing.** Typing, `Backspace`, `Delete`, the word deletions and a paste
    all replace the selection. Losing focus ends it.
  - **`TextInput`** gains `selection`, `selected_text`, `select_to`,
    `select_all`, `clear_selection` and `delete_selection`. Its moves end the
    selection, and its edits replace it.
  - **Drawing.** The selection is drawn in `UiTheme::selection`, and the drawn
    caret is left out while something is selected, so reversed text is not
    patched over reversed text. The terminal's own cursor stays on the end that
    moves.
- **Copy, cut and paste in the single-line text field.** `Ctrl+c`, `Ctrl+x` and
  `Ctrl+v` act as the multi-line editor's do.
  - A copy goes to the terminal through `TerminalRequest` and to `LastCopied`,
    and a paste inserts `LastCopied`, so text moves between the two text
    widgets.
  - With nothing selected, copy and cut send nothing.
  - A field carrying `TextMask` copies and cuts nothing.
  - `TextInput::handle` leaves the three actions to its caller, as it leaves
    `Submit`, since the field cannot reach a clipboard. A host routing its own
    keys acts on them through `selected_text`, `delete_selection` and `paste`.
- **`UiTheme::selection`.** The style patched over the characters a text
  selection covers, reversed by default, with `with_selection`. Like `caret`, it
  reaches only a selection a widget draws into its own cells. The multi-line
  editor's selection is its engine's, set with `set_selection_style`.
- **`PointerPress::modifiers`.** The modifiers the terminal reported with the
  press, with `with_modifiers`. They are read from the mouse event rather than
  from the held keys. A terminal without modifier key events learns what is held
  only from key messages, so a shift reported with a click would otherwise go
  unseen.
- **Copying rows from a list box or a table.** `Ctrl+c` on a focused `ListBox`
  or an interactive `Table` copies to the terminal through `TerminalRequest` and
  to `LastCopied`, so the text pastes into either text widget. It is bound
  through the new `ListBoxAction::Copy` and `TableAction::Copy`, so an app can
  remap or remove it.
  - **Which rows.** The cursor row. A `ListBoxMultiSelect` list or
    `TableMultiSelect` table with checked rows copies all of them instead, in
    child order. A single-select container copies the cursor row even when
    another row is checked, and `Marked` rows are never copied.
  - **List rows** copy their `ListItemText` when they have one, else their
    label. `ListItemTrailing` is left out.
  - **Table rows** copy their cells tab-separated, one row per line. A table
    selecting columns copies the cursor's column from every body row, and one
    selecting cells copies the cursor's cell. The header and footer are never
    copied.
  - **Nothing to copy.** With no cursor row, or no column in a table that
    selects columns, the key goes on to the container's ancestors.

### Changed

- **Breaking: `TextInputAction` has eleven new variants.** They are
  `SelectLeft`, `SelectRight`, `SelectWordLeft`, `SelectWordRight`,
  `SelectHome`, `SelectEnd`, `SelectAll`, `CancelSelection`, `Copy`, `Cut` and
  `Paste`.
  - A `match` over every variant has to handle them. `Copy`, `Cut` and `Paste`
    are left to the caller, like `Submit`.
  - The enum stays closed, so the compiler points out every action a host
    routing its own keys has to carry out.
  - The select variants sit beside the motions, which moves the numeric
    discriminants of `Backspace` through `Submit`.
  - `CancelSelection` is not bound by default, so `Escape` still reaches
    whatever holds the field.
- **Breaking: `content_cell` and `screen_cell` take the scroll offset as
  `Option<&ScrollOffset>`.** Both took a bare `Position`. Pass the component a
  query hands back, or `None` for a widget that is not scrolled: a widget
  carrying no `ScrollOffset` is scrolled to the origin, and the two functions
  now apply that rule themselves.
  - `ScrollOffset::resolve` is removed with it, having had no other use. Code
    that called it before either function drops the call.
  - Scroll state held somewhere other than the component goes in as
    `Some(&ScrollOffset(position))`.
- **A focused text field takes `Ctrl+a`, `Ctrl+c`, `Ctrl+x` and `Ctrl+v`.**
  These chords used to propagate from a field. An ancestor handling them no
  longer sees them while a field has focus, even when nothing is selected.
  Remove the bindings from the field's `TextInputKeys` to hand them back.
- **A click on a text field places its caret.** A press used to focus the field
  and leave the cursor where it was.
- **A focused list box or interactive table takes `Ctrl+c`** when it has
  something to copy. An ancestor handling that chord no longer sees it then.
  Remove the binding from `ListBoxKeys` or `TableKeys` to hand it back.

- **The multi-line editor's default keys are conventional, not emacs.**
  `TextEditor` used to take ratatui-textarea's emacs keymap. Its default table
  now binds the arrows, `Home` and `End`, and `Ctrl` with them to move by word,
  by paragraph and to either end of the text, each selecting with `Shift`.
  `PageUp` and `PageDown` scroll, `Enter` breaks the line, and `Backspace` and
  `Delete` delete, shifted or not, and by word with `Alt`. `Ctrl+z` undoes,
  `Ctrl+y` redoes, `Ctrl+c`, `Ctrl+x` and `Ctrl+v` copy, cut and paste as
  before, `Ctrl+a` selects all, and `Escape` ends a selection. The emacs chords
  (`Ctrl+a`, `e`, `f`, `b`, `n`, `p`, `h`, `d`, `k`, `j`, `w`, `u`, `r` and `y`
  in their emacs meanings, and every `Alt` chord but word deletion) no longer
  act unless an app binds them. `Shift` with `PageUp` or `PageDown` no longer
  extends a selection. A chord the table does not bind now reaches the editor's
  ancestors instead of being swallowed, and `Enter` with `Shift`, `Ctrl` or
  `Alt` is such a chord rather than a line break. A plain motion always ends the
  selection, even where the cursor cannot move. Deleting a multi-scalar grapheme
  cluster is now one edit to undo.
- **`plurimus_test`'s key injectors release what they press.** `press_key`,
  `press_key_with` and `repeat_key` send the key's release on a second tick, as
  `press_chord` already did, so a keystroke no longer stays held in
  `ButtonInput<KeyCode>` for the rest of the app. A test that needs a key held
  queues it with `write_key` and ticks.
- **A press on a `ModalityToggle` closes the overlays it does not own.** Pressed
  outside the open overlays, a toggle used to leave every one of them open, so
  clicking one menu's button with another menu open left both open. It now
  dismisses each open overlay whose `ModalOpen` root is not beneath it, then
  takes the press, so one click switches from File to Edit; the rest of that
  batch of input waits a frame for the close. A menu that closes returns focus
  to its button only if focus was still inside it, so the press that switched
  keeps the focus it moved.
- **A bound key that changes nothing is no longer consumed.** `ScrollKeys`,
  `ListBoxKeys`, `TableKeys`, `TabBarKeys` and `SliderKeys` used to swallow
  every key they bind. They now let a key through when it would move nothing: a
  scroll area already at that end or fitting that axis, a list or table cursor
  on its first or last row, a tab bar on its first or last tab, a slider at its
  bound. The key reaches the widget's ancestors, so a fresh arrow press at an
  edge moves focus to the neighbouring widget; a held arrow still stops there,
  since directional navigation ignores repeats. Select keeps consuming, its
  repeats included, except on a list with no cursor. A slider key at a bound no
  longer emits a `ValueChange`. A scroll consumer other than a `ScrollArea`,
  such as a bevy_ui node, lets a key through for an axis its `WheelAxes` rules
  out. `ScrollKeys` now binds Left and Right to `LineLeft` and `LineRight` by
  default.
- **`UiHidden` hides a whole subtree.** Everything beneath a hidden entity is
  neither drawn, its scrolled content included, nor hit-tested, so hiding a
  `Popover` hides its content too. A menu now hides its popup alone; `menu_item`
  no longer spawns with `UiHidden`.
- **`InteractionDisabled` disables a whole subtree.** Everything beneath a
  disabled entity absorbs presses, ignores its keys and the wheel, drops out of
  directional navigation and is drawn disabled - a button inside a disabled
  container included. A tab item of a disabled bar is no longer `Pressed`.
- **A press focuses the nearest focusable ancestor.** A press on an entity with
  no `TabIndex` focuses the first one at or above it, so a press on a tab
  focuses its bar on the way down rather than on the click. The search stops at
  `PressFocusDisabled`, which now covers everything inside the widget carrying
  it, and at an open overlay's `ModalOpen` root, so a press on a menu row keeps
  focus in the menu.

### Fixed

- **The terminal cursor stays where it was placed.** Drawing changed cells moves
  a terminal's cursor to the last cell written, and the presenter only placed
  the cursor again when `TerminalCursor` changed, so a frame redrawing any other
  widget left the cursor there. That showed a second cursor away from the caret,
  or none on it. The presenter now puts a shown cursor back after every frame
  that draws, and writes the cells and the move in one flush, so the cursor no
  longer shows for a moment where the drawing stopped.
- **Left and Right in a row-selection table.** In a `TableSelection::Row` table,
  the column keys jumped the cursor to the first row. They now leave it where it
  is and pass the key on.

- **Tab out of an open menu.** Tab moved focus to another tab stop and left the
  popup open behind it, still swallowing the pointer. `MenuKeys` now binds Tab
  and Shift+Tab to close the menu by default, returning focus to its button as
  Escape does.
- **A wheel tick routed ahead of the click before it.** In one batch of input, a
  tick after a click that opened a menu scrolled whatever the popup was about to
  cover. Ticks now route in order with presses, so a tick behind a click that
  opens or closes an overlay waits a frame for it. A tick that closes an overlay
  now makes the rest of its batch wait a frame too, as a press that closes one
  does.
- **A pointer press surviving a focus loss.** A widget pressed when the terminal
  lost focus stayed `Pressed`, kept receiving drags, and clicked if the release
  that eventually arrived landed inside it; `ButtonInput<MouseButton>` kept the
  button held. The gesture now ends at the focus loss, with no click, and the
  next press starts a new click run.
- **Bevy's key state lagging a focus loss by a frame.** When the terminal lost
  focus, the keys released for it reached `ButtonInput<KeyCode>` that frame but
  bevy's `KeyboardInput` and its own `ButtonInput` only on the next, so a
  `FocusedInput` observer could see a key held that no longer was. Both now
  release in the same frame.
- **`plurimus_3d` in a browser.** It now enables `bevy_render`'s WebGPU backend
  on wasm itself. Its headless render stack has no surface, so WebGL2 could
  never provide an adapter, and every browser app had to enable the feature on
  its own or fail at startup.
- **A text field narrower than the character under its cursor.** A one-column
  `EditableText` whose cursor sat on a double-width character (CJK, most emoji)
  panicked in debug builds and could lose its caret in release. It now draws the
  caret in its one column.
- **A slider with reversed bounds.** `SliderRange::new` puts its bounds in
  order, so `SliderRange::new(100.0, 0.0)` and `slider(100.0, 0.0, value)` mean
  the same range as `0.0..=100.0`, instead of panicking on the first key step or
  drag.
- **A terminal left in raw mode after a failed start.** If `CrosstermPlugin`
  failed partway through taking over the terminal, it panicked with raw mode and
  the alternate screen still on, leaving a shell that echoed nothing. The
  terminal is now restored before the panic is reported.
- **A cursor shape outliving the app.** Setting `TerminalCursorStyle` back to
  `Default` now returns the terminal to the user's configured shape instead of
  keeping the last one, and exiting or panicking resets a shape the app set. An
  app that never sets a shape leaves the shell's own alone.
- **Keys stuck held on a terminal that half-supports the kitty protocol.**
  `plurimus_crossterm` no longer takes a terminal's answer to the kitty keyboard
  query as proof that it reports key releases and modifier keys. Both
  capabilities start absent and turn on with the first release and the first
  modifier-key event, so a terminal that answers the query but ignores those
  flags keeps release synthesis and modifier tracking instead of reading every
  key as held. Until then, an app reading `InputCapabilities` sees both off.
- **A panic popping the shell's kitty keyboard flags.** A panic restores the
  terminal twice, from the panic hook and again as the app drops, and the second
  restore popped the kitty keyboard flags again - on the main screen by then,
  taking whatever the shell had pushed. Each thing restore undoes is now undone
  once.
- **The last row and column of a scroll area hidden under its scrollbars.** A
  `ScrollArea` clamped its offset against its whole area while its scrollbars
  took a column and a row of it, so the wheel, `ScrollIntoView`, a `Scrollbar`
  and `ScrollKeys` all stopped one short and the content's edge stayed under the
  bar. They now measure against the space left beside the bars. Scrolling to the
  end reaches one row or column further than before, a `ScrollKeys` page is the
  visible height, and content that fits one axis exactly while overflowing the
  other can now be wheeled along it. List boxes and tables with automatic
  scrollbars never show a horizontal bar, and scroll as before.
- **A widget that vanished when its `ScrollArea` was removed.** Removing the
  component left the widget drawn by nothing. It is now drawn again, unscrolled.
- **A bevy_ui node cut by its camera's edge drawn shifted instead of cut.** A
  node hanging off the left or top of its viewport, or scrolled partly out of a
  container at the top of one, drew as if it began at the edge: a border corner
  that isn't there, the first characters and lines of its text instead of the
  visible ones, and its gradients and shadow moved with it. It now draws from
  its true edges. Its text wraps at its laid-out width, not at the width left
  visible, and a wide character cut by the left edge leaves its visible column
  blank. A node wholly off-screen still draws the part of its `BoxShadow` that
  falls on-screen.
- **`ComputedNodeRect` running past its camera.** Its rects were cut to the
  viewport on the left and top only, so a node overflowing its camera on the
  right or bottom took clicks inside whatever camera sat beside it. They are now
  cut on all four sides, and are zero for a node wholly outside the viewport.
- **A lower pixel showing through a higher one in braille.** In a
  `SubcellMode::Braille` camera, a pixel drawn over another on the same dot
  mixed its color with the one beneath, so a higher-z sprite came out tinted by
  what it covered. The higher pixel's color now replaces it. Distinct dots in
  one cell still share the average of their colors, since a braille cell has one
  foreground.
- **Cameras with the same `order` stacking at random.** Cameras with equal
  `order` were composited in query order, which can change as components are
  added and removed. They now composite in `Entity` order, the tie-break used
  when choosing the default camera and when docking viewports, so their stacking
  is fixed. Two equal-order cameras spawned one after the other may swap
  compared with before; give cameras distinct orders when their stacking
  matters.
- **Terminal requests taken from every other reader.** `plurimus_crossterm`
  consumed each frame's `TerminalRequest`s by emptying the stream, so a system
  reading them after extraction, or one that had not read them yet, saw nothing.
  It now reads them the way `plurimus_web` already did, leaving them for every
  other reader. With it installed, a copy written in `Last` after
  `RequestSystems::Echo` now reaches `LastCopied` on the next frame instead of
  never. `plurimus_test`'s `clipboard_writes` no longer takes the requests
  either: it returns the copies made since its previous call and leaves the
  stream as it found it.
- **Rows that lost a component still drawn with it.** Removing `ListItemText`,
  `UiLabel` or `ListItem` from a list box row, or `TableHeader`, `TableFooter`
  or `TableRow` from a table row, left the container drawn as before until
  something else repainted it. It now redraws in the same frame, and a list's
  scroll extent follows a row losing its text or ceasing to be a row. The other
  way round, a tab bar no longer redraws its chrome when the active tab changes,
  and a table no longer redraws when a row loses `Marked` or `ListItemTrailing`,
  neither of which it draws.

## [0.7.2] - 2026-09-26

### Added

- **`plurimus_web`**, behind the facade's new `web` feature: a plurimus app in a
  browser page instead of a terminal. `WebPlugin` takes `CrosstermPlugin`'s
  place after `CorePlugin` and draws on a WebGL2 canvas (through ratzilla) that
  fills an element of the page and follows its size. The browser's keyboard -
  presses, repeats and real releases, with modifier keys as keys - pointer,
  wheel, paste and focus arrive as the same input messages a terminal backend
  writes. While the canvas has focus every key reaches the app except reload and
  devtools, which `WebPlugin::passthrough` extends. The app runs once per
  animation frame, holding its first update until the page's font has loaded.
  `GridFit` picks the font size once: a size, a column count, or the largest
  size that still holds a minimum grid. Clipboard copies and window titles are
  served, and when the app exits the page's title is restored and a
  `plurimus-exit` event carrying the exit code bubbles from the canvas to the
  page. It builds for `wasm32-unknown-unknown` without wasm threads; the new
  `web` example runs it in a page, built with a size-optimized `wasm-release`
  profile.

## [0.7.1] - 2026-08-26

### Added

- **`ListBoxStripe`** bands a list box the way `TableStripe` bands a table: a
  `Style` patched over every second row counting from the second, in child order
  so stripes do not crawl as the list scrolls, with a multi-line `ListItemText`
  row counting once. It sits beneath a row's own `UiStyle` and beneath the
  cursor's highlight, so an app keeps its per-row override and its cursor over a
  banded list. Editing the stripe repaints; removing it waits for the next
  repaint, as removing `ListBoxSelectionMarker` does. The widgets example
  stripes its themed list.
- **`TabBar`**, a focusable strip of `TabItem` children. The bar is the one tab
  stop; its items are child entities labelled by `UiLabel`, placed by the bar
  every frame and drawn by themselves, so each hovers, presses, disables and
  takes a `UiStyle` on its own. The active item is the one carrying `Checked`,
  as a radio group's selected option is: stepping activates rather than moving a
  cursor, every activation is `ValueChange<Entity>` on the bar, and
  `tab_bar_self_update` moves `Checked` for an uncontrolled bar. A click on an
  item activates it and hands focus to the bar, unless the bar carries
  `PressFocusDisabled`; a disabled item is stepped over and a disabled bar
  consumes nothing.
- **`TabBarLook`**, the bar's look as composable fields rather than variants:
  `orientation` (horizontal, or vertical with every item at the bar's width),
  `border` (any ratatui `BorderType`, making the bar three cells thick),
  `divider` (a `Line` drawn in the one cell between items), `padding`, and
  `joined` - the edge of the bar the active box opens onto, drawing a baseline
  along it with the closed boxes teed into it. A quadrant border, or an edge
  along the bar's axis, draws closed boxes. `thickness()` tells an app how thick
  to make the bar's area. Items past the bar's extent draw nothing.
- **`TabBarActiveStyle`**, patched over the theme's style for the active item
  - label and border alike - and beneath the item's own `UiStyle`, so the active
    tab reads while the bar does not hold focus. Defaults to bold alone; an app
    naming a colour names a foreground, a background filling the whole box.
    While the bar holds focus the theme's `focused` patch lands on top.
- **`TabBarKeys` and `TabBarAction`** - `Previous`, `Next`, `First`, `Last`, and
  `Select(index)` over the enabled items - defaulting to the arrows on both
  axes, `Home` and `End`, and no printable key; `[`/`]` and digits are an app's
  remap. Stepping stops at the ends, a held arrow keeps stepping, and a `Select`
  past the last item activates nothing and propagates.
- **`examples/tabs`**, a joined top bar switching panels under remapped keys,
  with a vertical bar on its Settings panel that restyles it.

### Changed

- The stock `UiTheme`'s `pressed` style is `fg Cyan` with `DIM`, where it was
  `fg Black` on a `bg Cyan` fill. A press focuses what it lands on, and
  `focused` names its own foreground, so the old foreground was overwritten on
  every press and the cell fill was all that told pressed from hovered - a solid
  block, out of scale with a theme that otherwise colours foregrounds only.
  Under focus the new style resolves to `Yellow BOLD | DIM`, distinguishable
  from hovered-and-focused without painting a background. Apps with a `pressed`
  of their own see no change.

## [0.7.0] - 2026-08-21

### Added

- **`SliderKeys`, `MenuKeys` and `TextInputKeys`**, the last three widgets' keys
  as data. Sliders, menus and the single-line field matched their keys inline,
  so an app could not remap them at all; each now holds a bindings component of
  its own with a `Default` binding what it always bound - and `TextInputKeys`
  carries `Submit` among its actions, so a form can move the field's commit to
  `Ctrl+Enter` and keep plain Enter for itself. A menu's table lives on its
  popup, one per menu rather than one per row. Every widget in the crate now
  takes its keys from a component.
- **`ScrollOffset::resolve`**, what a widget is scrolled by given whether it
  carries an offset at all. A widget with no `ScrollOffset` is scrolled to the
  origin, which every caller of `content_cell` and `screen_cell` had to know and
  state for itself; the rule now comes from the crate that owns the component,
  so a widget family written elsewhere asks rather than assumes.
- **`KeyCode::held_as`**, the key a character is held as. A terminal reports the
  character produced rather than the key struck, so a fold to lowercase is what
  lets a hold be matched to the release that ends it; every other key is its own
  identity. Public because an app keeping its own held set needs the same rule
  the crate applies, and because it names what polled state is now keyed on.
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

- **Polled keys are keyed by identity, not by the character typed.**
  `ButtonInput<KeyCode>` holds `Char('w')` however the key was struck or
  released, so pressing shift midway through a hold no longer strands it down
  for the life of the app - the release arrives as `W`, and now cancels the `w`
  it ends. `pressed(KeyCode::Char('W'))` still compiles and is never true; poll
  for the lowercase key, or ask `KeyCode::held_as` for it. The message stream is
  unchanged and still reports what was typed.
- **A key held across a focus loss is released in the frame focus was lost**,
  rather than the frame after. The synthesized releases now go through one
  held-key registry that both synthesis paths share, written before polled state
  is derived instead of read back out of it; a release synthesized for a key no
  terminal will report names the key as held and carries no modifiers, which is
  the only pairing a backend could have sent.
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

[0.9.0]: https://crates.io/crates/plurimus/0.9.0
[0.8.0]: https://crates.io/crates/plurimus/0.8.0
[0.7.2]: https://crates.io/crates/plurimus/0.7.2
[0.7.1]: https://crates.io/crates/plurimus/0.7.1
[0.7.0]: https://crates.io/crates/plurimus/0.7.0
[0.6.0]: https://crates.io/crates/plurimus/0.6.0
[0.5.0]: https://crates.io/crates/plurimus/0.5.0
[0.4.0]: https://crates.io/crates/plurimus/0.4.0
[0.3.0]: https://crates.io/crates/plurimus/0.3.0
[0.2.0]: https://crates.io/crates/plurimus/0.2.0
[0.1.0]: https://crates.io/crates/plurimus/0.1.0
