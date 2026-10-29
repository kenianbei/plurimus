//! The windowed target: bevy's own window, renderer and picking, assembled
//! from the granular crates, and a skin that shares nothing with the
//! terminal's but the tree it dresses. The field is the skin's own, typed
//! through `FocusedInput`, since upstream's `TextInput` edits through a
//! text-buffer stack this example does not run.

use bevy_a11y::AccessibilityPlugin;
use bevy_app::{
    App, AppExit, PanicHandlerPlugin, Plugin, PostUpdate, Startup, TaskPoolPlugin, Update,
};
use bevy_asset::AssetPlugin;
use bevy_camera::{Camera2d, CameraPlugin, ClearColor};
use bevy_color::Color;
use bevy_core_pipeline::CorePipelinePlugin;
use bevy_diagnostic::FrameCountPlugin;
use bevy_ecs::prelude::{
    Add, ChildOf, Children, Commands, Component, DetectChangesMut, Entity, Has,
    IntoScheduleConfigs, MessageWriter, On, Query, Res, ResMut, Resource, With, Without,
};
use bevy_ecs::system::EntityCommands;
use bevy_image::{ImagePlugin, TextureAtlasPlugin};
use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy_input::{ButtonInput, ButtonState, InputPlugin};
use bevy_log::LogPlugin;
use bevy_mesh::MeshPlugin;
use bevy_render::RenderPlugin;
use bevy_sprite_render::SpriteRenderPlugin;
use bevy_text::{FontSize, Strikethrough, TextColor, TextFont, TextPlugin, TextSpan};
use bevy_time::TimePlugin;
use bevy_transform::TransformPlugin;
use bevy_ui::widget::Text;
use bevy_ui::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, BoxShadow, Checked, GridTrack,
    JustifyContent, Node, Outline, Pressed, UiPlugin, UiRect, UiSystems, Val,
};
use bevy_ui_render::UiRenderPlugin;
use bevy_window::{Window, WindowPlugin};
use bevy_winit::WinitPlugin;
use plurimus::bui::bevy_picking::DefaultPickingPlugins;
use plurimus::bui::bevy_picking::hover::Hovered;
use plurimus::bui::bevy_ui_widgets::{Activate, UiWidgetsPlugins};
use plurimus::ui::bevy_input_focus::tab_navigation::{TabIndex, TabNavigationPlugin};
use plurimus::ui::bevy_input_focus::{
    FocusedInput, InputDispatchPlugin, InputFocus, InputFocusPlugin,
};

use crate::model::{Done, Part, Todos};
use crate::{add_shared, write};

const BACKDROP: Color = Color::srgb(0.051, 0.059, 0.078);
const CARD: Color = Color::srgb(0.090, 0.102, 0.137);
const EDGE: Color = Color::srgb(0.137, 0.153, 0.212);
const WELL: Color = Color::srgb(0.059, 0.067, 0.094);
const WELL_EDGE: Color = Color::srgb(0.165, 0.184, 0.243);
const PILL: Color = Color::srgb(0.149, 0.169, 0.227);
const ACCENT: Color = Color::srgb(0.486, 0.424, 1.0);
const ACCENT_HOVER: Color = Color::srgb(0.557, 0.502, 1.0);
const ACCENT_PRESSED: Color = Color::srgb(0.416, 0.353, 0.941);
const INK: Color = Color::srgb(0.902, 0.910, 0.937);
const INK_STRONG: Color = Color::srgb(0.961, 0.965, 0.980);
const INK_SOFT: Color = Color::srgb(0.545, 0.576, 0.655);
const INK_FAINT: Color = Color::srgb(0.373, 0.400, 0.471);
const DANGER: Color = Color::srgb(1.0, 0.420, 0.420);
const DANGER_WASH: Color = Color::srgba(1.0, 0.420, 0.420, 0.13);
const CHECK_EDGE: Color = Color::srgb(0.231, 0.259, 0.337);
const SHADOW: Color = Color::srgba(0.0, 0.0, 0.0, 0.55);

const WINDOW: (u32, u32) = (720, 640);
const CARD_WIDTH: f32 = 560.0;
const ID_COLUMN: f32 = 48.0;
const CONTROL_PAD: f32 = 12.0;
const CHECK_SIZE: f32 = 22.0;
const BODY: f32 = 16.0;
const SMALL: f32 = 14.0;
const TITLE: f32 = 34.0;
const CARET: &str = "|";
/// The default font is an ASCII subset, so the tree's `×` is replaced.
const DELETE_GLYPH: &str = "x";
const PLACEHOLDER: &str = "What needs to be done?";

pub(crate) fn run() -> AppExit {
    let mut app = App::new();
    app.add_plugins((
        PanicHandlerPlugin,
        LogPlugin::default(),
        TaskPoolPlugin::default(),
        FrameCountPlugin,
        TimePlugin,
        TransformPlugin,
        InputPlugin,
        InputFocusPlugin,
        InputDispatchPlugin,
        TabNavigationPlugin,
        WindowPlugin {
            primary_window: Some(Window {
                title: "todos".into(),
                resolution: WINDOW.into(),
                ..Window::default()
            }),
            ..WindowPlugin::default()
        },
        AccessibilityPlugin,
        AssetPlugin::default(),
        WinitPlugin::default(),
    ));
    app.add_plugins((
        RenderPlugin::default(),
        ImagePlugin::default(),
        TextureAtlasPlugin,
        MeshPlugin,
        CameraPlugin,
        CorePipelinePlugin,
        SpriteRenderPlugin,
        TextPlugin,
        UiPlugin,
        UiRenderPlugin,
        DefaultPickingPlugins,
        UiWidgetsPlugins,
        DesktopSkin,
    ));
    add_shared(&mut app);
    app.add_systems(Update, quit_on_escape);
    app.run()
}

/// The desktop look over the shared tree, and the field it supplies.
pub(crate) struct DesktopSkin;

impl Plugin for DesktopSkin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(BACKDROP))
            .init_resource::<Draft>()
            .add_observer(shape)
            .add_systems(Startup, |mut commands: Commands| {
                commands.spawn(Camera2d);
            })
            .add_systems(PostUpdate, (paint, write_field).before(UiSystems::Prepare));
    }
}

/// The field's text so far, until Enter or the Add button commits it.
#[derive(Resource, Debug, Default)]
struct Draft(String);

/// The checkmark a checkbox shows; the terminal skin draws `[x]` instead.
#[derive(Component)]
struct Mark;

/// The field's text and caret, as two spans so the caret can sit either
/// side of the text.
#[derive(Component)]
struct FieldSpan(usize);

const fn px(count: f32) -> Val {
    Val::Px(count)
}

fn font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..TextFont::default()
    }
}

fn shape(add: On<Add<Part>>, mut nodes: Query<(&Part, &mut Node)>, mut commands: Commands) {
    let Ok((part, mut node)) = nodes.get_mut(add.entity) else {
        return;
    };
    let mut entity = commands.entity(add.entity);
    if !shape_frame(*part, &mut node, &mut entity) {
        shape_control(*part, &mut node, &mut entity);
    }
    if is_interactive(*part) {
        entity.insert(Hovered::default());
    }
}

/// The containers and text that frame the controls; false for anything else.
fn shape_frame(part: Part, node: &mut Node, entity: &mut EntityCommands) -> bool {
    match part {
        Part::Root => node.padding = UiRect::top(px(72.0)),
        Part::Card => {
            node.width = px(CARD_WIDTH);
            node.padding = UiRect::all(px(28.0));
            node.border = UiRect::all(px(1.0));
            node.border_radius = BorderRadius::all(px(18.0));
            entity.insert((
                BackgroundColor(CARD),
                BorderColor::all(EDGE),
                BoxShadow::new(SHADOW, px(0.0), px(18.0), px(0.0), px(48.0)),
            ));
        }
        Part::Title => {
            node.margin = UiRect::bottom(px(20.0));
            entity.insert((font(TITLE), TextColor(INK_STRONG)));
        }
        Part::InputRow => {
            node.column_gap = px(10.0);
            node.margin = UiRect::bottom(px(18.0));
        }
        Part::List => {
            node.grid_template_columns[2] = GridTrack::px(ID_COLUMN);
            node.row_gap = px(2.0);
            node.column_gap = px(14.0);
            node.align_items = AlignItems::Center;
        }
        Part::Empty => {
            node.padding = UiRect::vertical(px(28.0));
            entity.insert(font(15.0));
        }
        Part::ItemText => {
            entity.insert(font(BODY));
        }
        Part::ItemId | Part::Count => {
            entity.insert(font(SMALL));
        }
        Part::Footer => {
            node.margin = UiRect::top(px(16.0));
            node.padding = UiRect::top(px(16.0));
            node.border = UiRect::top(px(1.0));
            entity.insert(BorderColor::all(EDGE));
        }
        Part::Filters => node.column_gap = px(4.0),
        _ => return false,
    }
    true
}

fn shape_control(part: Part, node: &mut Node, entity: &mut EntityCommands) {
    match part {
        Part::Field => shape_field(node, entity),
        Part::AddButton => {
            node.padding = UiRect::axes(px(22.0), px(CONTROL_PAD));
            node.border_radius = BorderRadius::all(px(10.0));
            entity.insert(font(15.0)).observe(add_from_draft);
        }
        Part::Check => {
            node.width = px(CHECK_SIZE);
            node.height = px(CHECK_SIZE);
            node.border = UiRect::all(px(2.0));
            node.border_radius = BorderRadius::all(px(7.0));
            node.align_items = AlignItems::Center;
            node.justify_content = JustifyContent::Center;
            entity.with_child((Mark, Text::new("x"), font(SMALL), TextColor(Color::NONE)));
        }
        Part::Delete => {
            node.padding = UiRect::axes(px(8.0), px(0.0));
            node.border_radius = BorderRadius::all(px(8.0));
            entity.insert((font(22.0), Text::new(DELETE_GLYPH)));
        }
        Part::Filter(_) | Part::Clear => {
            node.padding = UiRect::axes(px(12.0), px(6.0));
            node.border_radius = BorderRadius::all(px(999.0));
            entity.insert(font(SMALL));
        }
        _ => {}
    }
}

fn shape_field(node: &mut Node, entity: &mut EntityCommands) {
    node.padding = UiRect::axes(px(14.0), px(CONTROL_PAD));
    node.border = UiRect::all(px(1.0));
    node.border_radius = BorderRadius::all(px(10.0));
    node.align_items = AlignItems::Center;
    entity
        .insert((
            TabIndex(0),
            BackgroundColor(WELL),
            Text::default(),
            font(BODY),
        ))
        .with_children(|field| {
            field.spawn((TextSpan::default(), font(BODY), FieldSpan(0)));
            field.spawn((TextSpan::default(), font(BODY), FieldSpan(1)));
        })
        .observe(type_into_field);
}

const fn is_interactive(part: Part) -> bool {
    matches!(
        part,
        Part::Field | Part::AddButton | Part::Check | Part::Delete | Part::Filter(_) | Part::Clear
    )
}

fn type_into_field(
    mut input: On<FocusedInput<KeyboardInput>>,
    mut draft: ResMut<Draft>,
    mut todos: ResMut<Todos>,
) {
    let key = &input.event().input;
    if key.state != ButtonState::Pressed {
        return;
    }
    match &key.logical_key {
        Key::Enter => commit(&mut draft, &mut todos),
        Key::Backspace => {
            draft.0.pop();
        }
        _ => match &key.text {
            Some(text) if text.chars().all(|letter| !letter.is_control()) => {
                draft.0.push_str(text);
            }
            _ => return,
        },
    }
    input.propagate(false);
}

fn add_from_draft(_: On<Activate>, mut draft: ResMut<Draft>, mut todos: ResMut<Todos>) {
    commit(&mut draft, &mut todos);
}

fn commit(draft: &mut Draft, todos: &mut Todos) {
    todos.add(&draft.0);
    draft.0.clear();
}

fn quit_on_escape(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

// The caret sits after typed text and before the placeholder, so the two
// spans swap roles with the draft.
fn write_field(
    fields: Query<(Entity, &Part)>,
    mut spans: Query<(&FieldSpan, &ChildOf, &mut TextSpan, &mut TextColor)>,
    draft: Res<Draft>,
    focus: Res<InputFocus>,
) {
    for (field, part) in &fields {
        if *part != Part::Field {
            continue;
        }
        let caret = if focus.get() == Some(field) {
            CARET
        } else {
            ""
        };
        let (first, second) = if draft.0.is_empty() {
            ((caret, ACCENT), (PLACEHOLDER, INK_FAINT))
        } else {
            ((draft.0.as_str(), INK), (caret, ACCENT))
        };
        for (span, parent, mut text, mut color) in &mut spans {
            if parent.parent() != field {
                continue;
            }
            let (content, ink) = if span.0 == 0 { first } else { second };
            if text.0 != content {
                content.clone_into(&mut text.0);
            }
            color.set_if_neq(TextColor(ink));
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the five facts a control is drawn from, read together"
)]
struct State {
    is_hovered: bool,
    is_pressed: bool,
    is_focused: bool,
    is_checked: bool,
    is_done: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Look {
    background: Color,
    border: Color,
    ink: Color,
}

const PLAIN: Look = Look {
    background: Color::NONE,
    border: Color::NONE,
    ink: INK,
};

const fn look(part: Part, state: State) -> Option<Look> {
    Some(match part {
        Part::Field => Look {
            background: WELL,
            border: if state.is_focused {
                ACCENT
            } else if state.is_hovered {
                CHECK_EDGE
            } else {
                WELL_EDGE
            },
            ..PLAIN
        },
        Part::Check => Look {
            background: if state.is_checked {
                ACCENT
            } else {
                Color::NONE
            },
            border: if state.is_checked || state.is_hovered || state.is_focused {
                ACCENT
            } else {
                CHECK_EDGE
            },
            ..PLAIN
        },
        Part::ItemText => Look {
            ink: if state.is_done { INK_FAINT } else { INK },
            ..PLAIN
        },
        Part::ItemId | Part::Count | Part::Empty => Look {
            ink: INK_SOFT,
            ..PLAIN
        },
        _ => return button_look(part, state),
    })
}

const fn button_look(part: Part, state: State) -> Option<Look> {
    Some(match part {
        Part::AddButton => Look {
            background: if state.is_pressed {
                ACCENT_PRESSED
            } else if state.is_hovered {
                ACCENT_HOVER
            } else {
                ACCENT
            },
            ink: Color::WHITE,
            ..PLAIN
        },
        Part::Delete => Look {
            background: if state.is_hovered {
                DANGER_WASH
            } else {
                Color::NONE
            },
            ink: if state.is_hovered { DANGER } else { INK_SOFT },
            ..PLAIN
        },
        Part::Filter(_) => Look {
            background: if state.is_checked { PILL } else { Color::NONE },
            ink: if state.is_checked {
                INK_STRONG
            } else if state.is_hovered {
                INK
            } else {
                INK_SOFT
            },
            ..PLAIN
        },
        Part::Clear => Look {
            ink: if state.is_hovered { DANGER } else { INK_SOFT },
            ..PLAIN
        },
        _ => return None,
    })
}

type Painted<'a> = (
    Entity,
    &'a Part,
    Option<&'a Hovered>,
    Has<Pressed>,
    Has<Checked>,
    Has<Done>,
    Has<Outline>,
    Has<Strikethrough>,
    Option<&'a Children>,
    Drawn<'a>,
);

type Drawn<'a> = (
    Option<&'a mut BackgroundColor>,
    Option<&'a mut BorderColor>,
    Option<&'a mut TextColor>,
);

/// Maps behaviour state to looks, every frame, writing only what differs.
fn paint(
    mut nodes: Query<Painted, Without<Mark>>,
    mut marks: Query<&mut TextColor, With<Mark>>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    for (
        entity,
        part,
        hovered,
        is_pressed,
        is_checked,
        is_done,
        has_ring,
        is_struck,
        children,
        drawn,
    ) in &mut nodes
    {
        let state = State {
            is_hovered: hovered.is_some_and(|hovered| hovered.0),
            is_pressed,
            is_focused: focus.get() == Some(entity),
            is_checked,
            is_done,
        };
        let Some(look) = look(*part, state) else {
            continue;
        };
        let (background, border, ink) = drawn;
        write(
            entity,
            background,
            BackgroundColor(look.background),
            &mut commands,
        );
        write(entity, border, BorderColor::all(look.border), &mut commands);
        write(entity, ink, TextColor(look.ink), &mut commands);
        let wants_ring = state.is_focused && *part != Part::Field;
        let ring = wants_ring.then(|| Outline::new(px(2.0), px(2.0), ACCENT));
        toggle(entity, ring, has_ring, &mut commands);
        let strike = (*part == Part::ItemText && state.is_done).then_some(Strikethrough);
        toggle(entity, strike, is_struck, &mut commands);
        paint_marks(children, is_checked, &mut marks);
    }
}

fn paint_marks(
    children: Option<&Children>,
    is_checked: bool,
    marks: &mut Query<&mut TextColor, With<Mark>>,
) {
    let ink = if is_checked {
        Color::WHITE
    } else {
        Color::NONE
    };
    for child in children.into_iter().flatten() {
        if let Ok(mut mark) = marks.get_mut(*child) {
            mark.set_if_neq(TextColor(ink));
        }
    }
}

/// Inserts or removes a marker so the entity carries it exactly when wanted.
fn toggle<T: Component>(entity: Entity, wanted: Option<T>, has: bool, commands: &mut Commands) {
    match wanted {
        Some(value) if !has => {
            commands.entity(entity).insert(value);
        }
        None if has => {
            commands.entity(entity).remove::<T>();
        }
        _ => {}
    }
}
