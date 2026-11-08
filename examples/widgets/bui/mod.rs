//! The right half: bevy's own `bevy_ui_widgets` on a real `bevy_ui` node
//! tree, laid out by taffy at one cell per pixel and picked by
//! `bevy_picking`. Nothing here is styled by the stock stylists - node
//! colors are mutated by the systems below.

mod style;

use bevy_app::{App, Startup, Update};
use bevy_color::Color;
use bevy_ecs::prelude::{ChildOf, Commands, Entity, IntoScheduleConfigs, On, Query, ResMut, With};
use bevy_ui::widget::Text;
use bevy_ui::{
    AngularColorStop, BackgroundColor, BackgroundGradient, BorderColor, BorderRadius, ColorStop,
    ConicGradient, FlexDirection, Gradient, LinearGradient, Node, UiPosition, UiRect,
    UiTargetCamera, Val,
};
use plurimus::bui::bevy_text::EditableText;
use plurimus::bui::bevy_ui_widgets::{
    Button, Checkbox, RadioButton, RadioGroup, Slider, SliderRange, SliderStep, SliderValue,
    TextInput, TrackClick, checkbox_self_update, radio_self_update, slider_self_update,
};
use plurimus::ui::bevy_input_focus::tab_navigation::{TabGroup, TabIndex};
use plurimus::ui::{Activate, Hovered, UiLabel, ValueChange};

use crate::{
    BuiCamera, CHECKBOX_LABEL, DemoState, FIELD_TEXT, RADIO_LABELS, SLIDER_KEY_STEP, SLIDER_START,
    spawn_cameras,
};
use style::{IDLE_BORDER, NORMAL};

const BUTTON_TAB_INDEX: i32 = 10;
const SLIDER_TAB_INDEX: i32 = 11;
const CHECKBOX_TAB_INDEX: i32 = 12;
const RADIO_TAB_INDEX: i32 = 13;
const FIELD_TAB_INDEX: i32 = 16;

pub(crate) fn add_bui_side(app: &mut App) {
    app.add_systems(Startup, spawn_bui_side.after(spawn_cameras));
    app.add_systems(
        Update,
        (
            style::style_fills,
            style::style_button_borders,
            style::sync_toggle_texts,
            style::sync_slider_track,
        ),
    );
}

fn spawn_bui_side(mut commands: Commands, cameras: Query<Entity, With<BuiCamera>>) {
    let Ok(camera) = cameras.single() else {
        return;
    };
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(1.0)),
                row_gap: Val::Px(1.0),
                ..Node::default()
            },
            UiTargetCamera(camera),
            TabGroup::new(1),
        ))
        .id();
    commands.spawn((Text::from("bui - taffy layout"), ChildOf(root)));
    spawn_button(&mut commands, root);
    spawn_slider(&mut commands, root);
    spawn_checkbox(&mut commands, root);
    spawn_radio_group(&mut commands, root);
    spawn_field(&mut commands, root);
    spawn_decor_strip(&mut commands, root);
}

fn spawn_button(commands: &mut Commands, root: Entity) {
    commands
        .spawn((
            Node {
                width: Val::Px(14.0),
                height: Val::Px(3.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(1.0)),
                ..Node::default()
            },
            Button,
            Hovered::default(),
            TabIndex(BUTTON_TAB_INDEX),
            Text::from("press me"),
            BackgroundColor(NORMAL),
            BorderColor::from(IDLE_BORDER),
            ChildOf(root),
        ))
        .observe(|_on: On<Activate>, mut state: ResMut<DemoState>| {
            state.bui.presses += 1;
        });
}

fn spawn_slider(commands: &mut Commands, root: Entity) {
    commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(1.0),
                padding: UiRect::horizontal(Val::Px(1.0)),
                ..Node::default()
            },
            Slider {
                track_click: TrackClick::Snap,
                ..Slider::default()
            },
            SliderRange::new(0.0, 100.0),
            SliderValue(SLIDER_START),
            SliderStep(SLIDER_KEY_STEP),
            Hovered::default(),
            TabIndex(SLIDER_TAB_INDEX),
            Text::default(),
            BackgroundColor(Color::NONE),
            ChildOf(root),
        ))
        .observe(slider_self_update)
        .observe(|on: On<ValueChange<f32>>, mut state: ResMut<DemoState>| {
            state.bui.slider = on.value;
        });
}

fn spawn_checkbox(commands: &mut Commands, root: Entity) {
    commands
        .spawn((
            Node::default(),
            Checkbox,
            Hovered::default(),
            TabIndex(CHECKBOX_TAB_INDEX),
            UiLabel(CHECKBOX_LABEL.into()),
            Text::default(),
            BackgroundColor(Color::NONE),
            ChildOf(root),
        ))
        .observe(checkbox_self_update);
}

fn spawn_radio_group(commands: &mut Commands, root: Entity) {
    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..Node::default()
            },
            RadioGroup,
            ChildOf(root),
        ))
        .observe(radio_self_update)
        .observe(
            |on: On<ValueChange<Entity>>, labels: Query<&UiLabel>, mut state: ResMut<DemoState>| {
                if let Ok(label) = labels.get(on.value) {
                    state.bui.radio = label.0.to_string();
                }
            },
        )
        .id();
    for (index, label) in RADIO_LABELS.iter().enumerate() {
        commands.spawn((
            Node::default(),
            RadioButton,
            Hovered::default(),
            TabIndex(RADIO_TAB_INDEX + index as i32),
            UiLabel((*label).into()),
            Text::default(),
            BackgroundColor(Color::NONE),
            ChildOf(group),
        ));
    }
}

fn spawn_field(commands: &mut Commands, root: Entity) {
    commands.spawn((
        Node {
            width: Val::Px(22.0),
            height: Val::Px(1.0),
            ..Node::default()
        },
        TextInput,
        EditableText::new(FIELD_TEXT),
        TabIndex(FIELD_TAB_INDEX),
        ChildOf(root),
    ));
}

fn spawn_decor_strip(commands: &mut Commands, root: Entity) {
    let strip = commands
        .spawn((
            Node {
                column_gap: Val::Px(2.0),
                ..Node::default()
            },
            ChildOf(root),
        ))
        .id();
    spawn_swatch(
        commands,
        strip,
        6.0,
        Gradient::Conic(ConicGradient::new(
            UiPosition::CENTER,
            vec![
                AngularColorStop::auto(Color::srgb(0.9, 0.9, 0.2)),
                AngularColorStop::auto(Color::srgb(0.1, 0.1, 0.4)),
            ],
        )),
    );
    spawn_swatch(
        commands,
        strip,
        22.0,
        Gradient::Linear(LinearGradient::to_right(vec![
            ColorStop::auto(Color::srgb(0.8, 0.2, 0.2)).with_hint(0.25),
            ColorStop::auto(Color::srgb(0.2, 0.3, 0.8)),
        ])),
    );
}

fn spawn_swatch(commands: &mut Commands, strip: Entity, width: f32, gradient: Gradient) {
    commands.spawn((
        Node {
            width: Val::Px(width),
            height: Val::Px(3.0),
            ..Node::default()
        },
        BackgroundGradient(vec![gradient]),
        ChildOf(strip),
    ));
}
