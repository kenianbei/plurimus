//! The left half: stock widget bundles at fixed cell rects, drawn by the
//! stock stylists from a custom [`UiTheme`].

use bevy_app::{App, AppExit, Startup, Update};
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::prelude::{
    Added, ChildOf, Commands, Entity, MessageWriter, On, Query, Res, ResMut, With,
};
use bevy_ecs::system::SystemParam;
use bevy_input_focus::InputFocus;
use bevy_input_focus::directional_navigation::DirectionalNavigationMap;
use bevy_input_focus::tab_navigation::{TabGroup, TabIndex};
use bevy_math::CompassOctant;
use plurimus::core::ratatui_core::layout::{Rect, Size};
use plurimus::core::ratatui_core::style::{Color, Modifier, Style};
use plurimus::core::{UiArea, UiWidget};
use plurimus::ui::tui_scrollview::ScrollbarVisibility;
use plurimus::ui::{Checked, ScrollArea, ValueChange};
use plurimus::widgets::ratatui_textarea::TextArea;
use plurimus::widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus::widgets::ratatui_widgets::scrollbar::ScrollbarOrientation;
use plurimus::widgets::{
    Activate, RadioGroup, SliderStep, SliderValue, TextEditor, TextInput, UiLabel, UiTheme, button,
    checkbox, checkbox_self_update, editable_text, list_item, listbox, listbox_self_update,
    menu_button, menu_item, menu_popup, pane, radio, radio_self_update, scrollbar, slider,
    slider_self_update, text_editor,
};

use crate::{CHECKBOX_LABEL, DemoState, FIELD_TEXT, RADIO_LABELS, SLIDER_KEY_STEP, SLIDER_START};

const LIST_ITEMS: [&str; 6] = ["alpha", "beta", "gamma", "delta", "epsilon", "zeta"];
const EDITOR_TEXT: &str = "a multi-line editor:\nenter splits, arrows roam.";

/// One step up from the terminal's default background, enough to mark a
/// focused input without recoloring its text.
const FOCUS_BG: Color = Color::Rgb(48, 48, 64);

const SLIDER_TAB_INDEX: i32 = 1;
const CHECKBOX_TAB_INDEX: i32 = 2;
const RADIO_TAB_INDEX: i32 = 3;
const LIST_TAB_INDEX: i32 = 6;
const FIELD_TAB_INDEX: i32 = 7;
const EDITOR_TAB_INDEX: i32 = 8;
const MENU_TAB_INDEX: i32 = 20;
pub(crate) const DISABLE_ITEM: &str = "toggle disabled";

const HEADER: Rect = Rect::new(1, 0, 22, 1);
const MENU_BUTTON: Rect = Rect::new(25, 0, 10, 1);

const CONTROLS_PANE: Rect = Rect::new(0, 1, 36, 8);
const BUTTON: Rect = Rect::new(2, 2, 13, 1);
const SLIDER: Rect = Rect::new(2, 3, 21, 1);
const CHECKBOX: Rect = Rect::new(2, 4, 20, 1);
const RADIO_TOP: u16 = 5;
const RADIO_WIDTH: u16 = 14;

const OPTIONS_PANE: Rect = Rect::new(0, 9, 36, 6);
const LISTBOX: Rect = Rect::new(2, 10, 32, 4);

const TEXT_PANE: Rect = Rect::new(0, 15, 36, 7);
const TEXT_FIELD: Rect = Rect::new(2, 16, 32, 1);
const TEXT_EDITOR: Rect = Rect::new(2, 18, 32, 3);

const LOG_PANE: Rect = Rect::new(0, 22, 36, 6);
const LOG_VIEW: Rect = Rect::new(2, 23, 30, 4);
const LOG_SCROLLBAR: Rect = Rect::new(33, 23, 1, 4);
const LOG_ENTRIES: u16 = 20;

pub(crate) fn add_themed_side(app: &mut App) {
    app.insert_resource(demo_theme());
    app.add_systems(Startup, spawn_themed_side);
    app.add_systems(Update, style_editors);
}

// `focused` overrides the state colors but only merges their modifiers,
// so hover and press carry a modifier apiece to stay visible underneath
// it. On a `Pane`, whose whole rendering is its border, the same focused
// style reads as a border color change.
fn demo_theme() -> UiTheme {
    UiTheme {
        normal: Style::new().fg(Color::Gray),
        hovered: Style::new().fg(Color::White).add_modifier(Modifier::BOLD),
        pressed: Style::new()
            .fg(Color::LightGreen)
            .add_modifier(Modifier::REVERSED),
        disabled: Style::new().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        focused: Style::new().fg(Color::LightMagenta).bg(FOCUS_BG),
    }
}

// The editor draws through its own engine rather than a stylist, so the
// theme never reaches it: its focus cue is set on the engine instead. That
// also drops the engine's default underline on the cursor's line.
fn style_editors(
    focus: Res<InputFocus>,
    editors: Query<(Entity, &TextEditor)>,
    spawned: Query<(), Added<TextEditor>>,
) {
    if !focus.is_changed() && spawned.is_empty() {
        return;
    }
    for (entity, editor) in &editors {
        let lift = if focus.get() == Some(entity) {
            Style::new().bg(FOCUS_BG)
        } else {
            Style::new()
        };
        let mut area = editor.lock();
        area.set_style(lift);
        area.set_cursor_line_style(lift);
    }
}

fn spawn_themed_side(mut commands: Commands, mut map: ResMut<DirectionalNavigationMap>) {
    let root = commands.spawn(TabGroup::new(0)).id();
    commands.spawn((
        UiWidget::new(Paragraph::new("themed - fixed rects")),
        UiArea::Fixed(HEADER),
    ));
    let controls = spawn_pane(&mut commands, root, "controls", CONTROLS_PANE);
    let (first, last) = spawn_controls(&mut commands, controls);
    let options = spawn_pane(&mut commands, root, "options", OPTIONS_PANE);
    spawn_listbox(&mut commands, options);
    let text = spawn_pane(&mut commands, root, "text", TEXT_PANE);
    spawn_text_widgets(&mut commands, text);
    let log = spawn_pane(&mut commands, root, "log", LOG_PANE);
    spawn_log(&mut commands, log);
    // A hand-made wrap-around edge, preserved across map rebuilds: the
    // auto map never loops. It stays inside the pane because the list and
    // text widgets below claim the arrow keys for their own cursors.
    map.add_symmetrical_edge(last, first, CompassOctant::South);
    spawn_menu(&mut commands, root);
}

fn spawn_pane(commands: &mut Commands, root: Entity, title: &str, area: Rect) -> Entity {
    commands
        .spawn((pane(title), UiArea::Fixed(area), ChildOf(root)))
        .id()
}

// Returns the first and last arrow-navigable widgets in the pane.
fn spawn_controls(commands: &mut Commands, parent: Entity) -> (Entity, Entity) {
    let press = commands
        .spawn((button("press me"), UiArea::Fixed(BUTTON), ChildOf(parent)))
        .observe(|_on: On<Activate>, mut state: ResMut<DemoState>| {
            state.themed.presses += 1;
        })
        .id();
    commands
        .spawn((
            slider(0.0, 100.0, SLIDER_START),
            SliderStep(SLIDER_KEY_STEP),
            UiArea::Fixed(SLIDER),
            ChildOf(parent),
        ))
        .insert(TabIndex(SLIDER_TAB_INDEX))
        .observe(slider_self_update)
        .observe(|on: On<ValueChange<f32>>, mut state: ResMut<DemoState>| {
            state.themed.slider = on.value;
        });
    commands
        .spawn((
            checkbox(CHECKBOX_LABEL),
            UiArea::Fixed(CHECKBOX),
            ChildOf(parent),
        ))
        .insert(TabIndex(CHECKBOX_TAB_INDEX))
        .observe(checkbox_self_update);
    (press, spawn_radio_group(commands, parent))
}

fn spawn_radio_group(commands: &mut Commands, parent: Entity) -> Entity {
    let group = commands
        .spawn((RadioGroup, ChildOf(parent)))
        .observe(radio_self_update)
        .observe(
            |on: On<ValueChange<Entity>>, labels: Query<&UiLabel>, mut state: ResMut<DemoState>| {
                if let Ok(label) = labels.get(on.value) {
                    state.themed.radio = label.0.clone();
                }
            },
        )
        .id();
    let mut last = group;
    for (index, label) in RADIO_LABELS.iter().enumerate() {
        let row = RADIO_TOP + index as u16;
        last = commands
            .spawn((
                radio(*label),
                UiArea::Fixed(Rect::new(BUTTON.x, row, RADIO_WIDTH, 1)),
                ChildOf(group),
            ))
            .insert(TabIndex(RADIO_TAB_INDEX + index as i32))
            .id();
    }
    last
}

fn spawn_listbox(commands: &mut Commands, parent: Entity) {
    let list = commands
        .spawn((listbox(), UiArea::Fixed(LISTBOX), ChildOf(parent)))
        .insert(TabIndex(LIST_TAB_INDEX))
        .observe(listbox_self_update)
        .observe(
            |on: On<ValueChange<Entity>>, labels: Query<&UiLabel>, mut state: ResMut<DemoState>| {
                if let Ok(label) = labels.get(on.value) {
                    state.themed.choice = label.0.clone();
                }
            },
        )
        .id();
    for label in LIST_ITEMS {
        commands.spawn((list_item(label), ChildOf(list)));
    }
}

fn spawn_text_widgets(commands: &mut Commands, parent: Entity) {
    commands
        .spawn((
            editable_text(FIELD_TEXT),
            UiArea::Fixed(TEXT_FIELD),
            ChildOf(parent),
        ))
        .insert(TabIndex(FIELD_TAB_INDEX));
    commands
        .spawn((
            text_editor(EDITOR_TEXT),
            UiArea::Fixed(TEXT_EDITOR),
            ChildOf(parent),
        ))
        .insert(TabIndex(EDITOR_TAB_INDEX));
}

// The bar draws the scroll state, so the area must not draw its own.
fn spawn_log(commands: &mut Commands, parent: Entity) {
    let log: String = (1..=LOG_ENTRIES)
        .map(|entry| format!("log entry {entry}\n"))
        .collect();
    let view = commands
        .spawn((
            UiWidget::new(Paragraph::new(log)),
            ScrollArea {
                content_size: Size::new(LOG_VIEW.width, LOG_ENTRIES),
                scrollbars: ScrollbarVisibility::Never,
            },
            UiArea::Fixed(LOG_VIEW),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        scrollbar(view, ScrollbarOrientation::VerticalRight),
        UiArea::Fixed(LOG_SCROLLBAR),
        ChildOf(parent),
    ));
}

fn spawn_menu(commands: &mut Commands, root: Entity) {
    let anchor = commands
        .spawn((
            menu_button("menu"),
            UiArea::Fixed(MENU_BUTTON),
            ChildOf(root),
        ))
        .insert(TabIndex(MENU_TAB_INDEX))
        .id();
    let popup = commands.spawn((menu_popup(anchor), ChildOf(anchor))).id();
    commands
        .spawn((menu_item("reset"), ChildOf(popup)))
        .observe(reset_demo);
    commands
        .spawn((menu_item(DISABLE_ITEM), ChildOf(popup)))
        .observe(crate::toggle_disabled);
    commands.spawn((menu_item("quit"), ChildOf(popup))).observe(
        |_on: On<Activate>, mut exit: MessageWriter<AppExit>| {
            exit.write(AppExit::Success);
        },
    );
}

#[derive(SystemParam)]
struct ResetTargets<'w, 's> {
    sliders: Query<'w, 's, &'static mut SliderValue>,
    fields: Query<'w, 's, &'static mut TextInput>,
    editors: Query<'w, 's, &'static TextEditor>,
    checked: Query<'w, 's, Entity, With<Checked>>,
}

// Widget components own what is rendered, so a reset must restore them
// alongside the derived DemoState. The editor's view shares its engine by
// handle, so its text is replaced through the lock, not the component.
fn reset_demo(
    _on: On<Activate>,
    mut state: ResMut<DemoState>,
    mut targets: ResetTargets,
    mut commands: Commands,
) {
    *state = DemoState::default();
    for mut value in &mut targets.sliders {
        value.0 = SLIDER_START;
    }
    for mut field in &mut targets.fields {
        *field = TextInput::new(FIELD_TEXT);
    }
    for editor in &targets.editors {
        *editor.lock() = TextArea::from(EDITOR_TEXT.lines());
    }
    for entity in &targets.checked {
        commands.entity(entity).remove::<Checked>();
    }
}
