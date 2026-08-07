//! Every stock plurimus widget, split-screen: themed widgets placed by
//! fixed cell rects on the left, the same widget logic driven by `bevy_ui`
//! layout on the right. Tab/Shift-Tab and arrows move focus (the pane
//! holding focus highlights its border), Enter/Space activates, arrows
//! adjust the focused slider and walk open menus, the mouse
//! hovers/clicks/drags, the menu resets the demo or disables every
//! widget, Esc unfocuses, `q` with nothing focused or ctrl-c quits.
//!
//! Laid out for a terminal of roughly 80x30 or larger.

mod bui;
mod themed;

#[cfg(test)]
mod tests;

use std::time::Duration;

use bevy_app::{App, AppExit, PreUpdate, ScheduleRunnerPlugin, Startup, Update};
use bevy_ecs::change_detection::{DetectChanges, DetectChangesMut};
use bevy_ecs::prelude::{
    Commands, Component, Entity, Has, IntoScheduleConfigs, MessageReader, MessageWriter, On, Or,
    Query, Res, ResMut, Resource, With, Without,
};
use bevy_input_focus::InputFocus;
use plurimus::bui::BuiPlugin;
use plurimus::core::ratatui_core::layout::Rect;
use plurimus::core::{
    CameraSystems, CorePlugin, ResolvedViewport, TerminalCamera, TerminalSize, UiArea, UiWidget,
    Viewport,
};
use plurimus::crossterm::CrosstermPlugin;
use plurimus::input::{KeyCode, KeyKind, KeyMessage};
use plurimus::ui::InteractionDisabled;
use plurimus::widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus::widgets::{
    Activate, Button, Checkbox, EditableText, ListBox, MenuButton, RadioButton, Slider, TextEditor,
    WidgetsPlugin,
};

const STATUS_ROWS: u16 = 2;

/// Demo content both sides present, so the two placement paths can be
/// compared on identical widgets.
const SLIDER_START: f32 = 50.0;
const SLIDER_KEY_STEP: f32 = 5.0;
const RADIO_LABELS: [&str; 3] = ["snap", "crackle", "pop"];
const CHECKBOX_LABEL: &str = "enable tachyons";
const FIELD_TEXT: &str = "edit me";

/// Marks the camera hosting the fixed-rect themed widgets.
#[derive(Component)]
struct ThemedCamera;

/// Marks the camera hosting the `bevy_ui` node tree.
#[derive(Component)]
struct BuiCamera;

#[derive(Component)]
struct StatusLine;

/// What one side's widgets have most recently reported.
struct SideState {
    presses: u32,
    slider: f32,
    radio: String,
    choice: String,
}

impl Default for SideState {
    fn default() -> Self {
        Self {
            presses: 0,
            slider: SLIDER_START,
            radio: String::new(),
            choice: String::new(),
        }
    }
}

#[derive(Resource, Default)]
struct DemoState {
    themed: SideState,
    bui: SideState,
}

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins((
        ScheduleRunnerPlugin::run_loop(Duration::from_millis(16)),
        CorePlugin,
        CrosstermPlugin::default(),
    ));
    add_demo(&mut app);
    app.run()
}

// Owns every plugin above core, so a headless test and the terminal
// binary cannot disagree about what the demo is made of.
fn add_demo(app: &mut App) {
    app.add_plugins((WidgetsPlugin, BuiPlugin));
    app.init_resource::<DemoState>();
    app.add_systems(Startup, (spawn_cameras, spawn_status));
    app.add_systems(
        PreUpdate,
        resize_cameras.before(CameraSystems::ResolveViewports),
    );
    app.add_systems(Update, (anchor_status, update_status, quit_on_key));
    themed::add_themed_side(app);
    bui::add_bui_side(app);
}

fn spawn_cameras(mut commands: Commands) {
    commands.spawn((
        TerminalCamera {
            viewport: Viewport::Fixed(Rect::ZERO),
            ..TerminalCamera::default()
        },
        ThemedCamera,
    ));
    commands.spawn((
        TerminalCamera {
            order: 1,
            viewport: Viewport::Fixed(Rect::ZERO),
            ..TerminalCamera::default()
        },
        BuiCamera,
    ));
}

fn spawn_status(mut commands: Commands) {
    commands.spawn((
        UiWidget::new(Paragraph::new("")),
        UiArea::Fixed(Rect::ZERO),
        StatusLine,
    ));
}

// The bui side takes the odd column, so the halves always cover the
// terminal exactly.
fn resize_cameras(
    size: Res<TerminalSize>,
    mut cameras: Query<(&mut TerminalCamera, Has<BuiCamera>)>,
) {
    let split = size.cols / 2;
    for (mut camera, is_bui) in &mut cameras {
        let viewport = Viewport::Fixed(if is_bui {
            Rect::new(split, 0, size.cols.saturating_sub(split), size.rows)
        } else {
            Rect::new(0, 0, split, size.rows)
        });
        if camera.viewport != viewport {
            camera.viewport = viewport;
        }
    }
}

// Only the status block follows the terminal; every other themed widget
// keeps the fixed rect it was spawned with.
fn anchor_status(
    cameras: Query<&ResolvedViewport, With<ThemedCamera>>,
    mut status: Query<&mut UiArea, With<StatusLine>>,
) {
    let Ok(viewport) = cameras.single().map(|resolved| resolved.0) else {
        return;
    };
    for mut area in &mut status {
        area.set_if_neq(UiArea::Fixed(Rect::new(
            0,
            viewport.height.saturating_sub(STATUS_ROWS),
            viewport.width,
            STATUS_ROWS,
        )));
    }
}

fn update_status(state: Res<DemoState>, mut lines: Query<&mut UiWidget, With<StatusLine>>) {
    if !state.is_changed() {
        return;
    }
    let text = format!(
        "themed  presses {}  slider {:.0}  {}\nbui     presses {}  slider {:.0}  {}",
        state.themed.presses,
        state.themed.slider,
        choice_summary(&state.themed),
        state.bui.presses,
        state.bui.slider,
        choice_summary(&state.bui),
    );
    for mut widget in &mut lines {
        *widget = UiWidget::new(Paragraph::new(text.clone()));
    }
}

fn choice_summary(side: &SideState) -> String {
    match (side.radio.as_str(), side.choice.as_str()) {
        ("", "") => String::new(),
        (radio, "") => radio.to_owned(),
        ("", choice) => choice.to_owned(),
        (radio, choice) => format!("{radio}/{choice}"),
    }
}

/// Every widget kind the menu's disable item covers, on both sides. The
/// menu button is exempt: disabling it would shut the only way back.
type Interactive = (
    Or<(
        With<Button>,
        With<Slider>,
        With<Checkbox>,
        With<RadioButton>,
        With<ListBox>,
        With<EditableText>,
        With<TextEditor>,
    )>,
    Without<MenuButton>,
);

// A menu item rather than a hotkey: the text widgets swallow plain keys,
// so any letter bound globally fires while the user is typing it.
pub(crate) fn toggle_disabled(
    _on: On<Activate>,
    widgets: Query<(Entity, Has<InteractionDisabled>), Interactive>,
    mut commands: Commands,
) {
    for (entity, disabled) in &widgets {
        if disabled {
            commands.entity(entity).remove::<InteractionDisabled>();
        } else {
            commands.entity(entity).insert(InteractionDisabled);
        }
    }
}

// The text widgets consume plain keys, so `q` only quits while nothing
// holds focus.
fn quit_on_key(
    mut keys: MessageReader<KeyMessage>,
    mut focus: ResMut<InputFocus>,
    mut exit: MessageWriter<AppExit>,
) {
    for key in keys.read() {
        if key.kind != KeyKind::Press {
            continue;
        }
        if key.modifiers.ctrl && key.code == KeyCode::Char('c') {
            exit.write(AppExit::Success);
            continue;
        }
        match key.code {
            KeyCode::Esc => focus.clear(),
            KeyCode::Char('q') if focus.get().is_none() => {
                exit.write(AppExit::Success);
            }
            _ => {}
        }
    }
}
