//! Node styling driven by change detection: the stock stylists never see
//! these widgets, so interaction state has to reach the node colors and
//! text by hand.

use bevy_color::Color;
use bevy_ecs::prelude::{Changed, Entity, Has, Or, Query, Res, With};
use bevy_input_focus::InputFocus;
use bevy_ui::{BackgroundColor, BorderColor};
use plurimus::bui::{ComputedNodeRect, Text};
use plurimus::ui::{Checked, Hovered, InteractionDisabled, Pressed, UiLabel};
use plurimus::widgets::{Button, Checkbox, RadioButton, SliderRange, SliderValue};

pub(super) const NORMAL: Color = Color::srgb(0.15, 0.15, 0.15);
const HOVERED: Color = Color::srgb(0.25, 0.25, 0.25);
const PRESSED: Color = Color::srgb(0.35, 0.75, 0.35);
const DISABLED: Color = Color::srgb(0.1, 0.1, 0.1);
pub(super) const IDLE_BORDER: Color = Color::srgb(0.6, 0.6, 0.6);
const FOCUS_BORDER: Color = Color::srgb(1.0, 0.8, 0.0);
/// The themed side's focus lift, in `bevy_color` terms.
const FOCUS_FILL: Color = Color::srgb(0.19, 0.19, 0.25);

const TRACK_BEFORE: char = '━';
const TRACK_THUMB: char = '█';
const TRACK_AFTER: char = '─';

type FillQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Hovered,
        Has<Pressed>,
        Has<InteractionDisabled>,
        Has<Button>,
        &'static mut BackgroundColor,
    ),
>;

// Only the button has a border to recolor, so focus reaches the rest as a
// background lift - the same cue the themed side's inputs get. Widgets
// with no background of their own idle fully transparent, which the
// rasterizer skips.
pub(super) fn style_fills(focus: Res<InputFocus>, mut widgets: FillQuery) {
    for (entity, hovered, pressed, disabled, is_button, mut background) in &mut widgets {
        let idle = if is_button { NORMAL } else { Color::NONE };
        let fill = if disabled {
            idle
        } else if pressed {
            PRESSED
        } else if hovered.0 {
            HOVERED
        } else if focus.get() == Some(entity) {
            FOCUS_FILL
        } else {
            idle
        };
        if background.0 != fill {
            background.0 = fill;
        }
    }
}

pub(super) fn style_button_borders(
    focus: Res<InputFocus>,
    mut buttons: Query<(Entity, Has<InteractionDisabled>, &mut BorderColor), With<Button>>,
) {
    for (entity, disabled, mut border) in &mut buttons {
        let edge = match (disabled, focus.get() == Some(entity)) {
            (true, _) => DISABLED,
            (false, true) => FOCUS_BORDER,
            (false, false) => IDLE_BORDER,
        };
        if border.top != edge {
            *border = BorderColor::from(edge);
        }
    }
}

pub(super) fn sync_toggle_texts(
    mut toggles: Query<
        (&UiLabel, Has<Checked>, Has<Checkbox>, &mut Text),
        Or<(With<Checkbox>, With<RadioButton>)>,
    >,
) {
    for (label, checked, is_checkbox, mut text) in &mut toggles {
        let mark = match (is_checkbox, checked) {
            (true, true) => "[x]",
            (true, false) => "[ ]",
            (false, true) => "(•)",
            (false, false) => "( )",
        };
        let rendered = format!("{mark} {}", label.0);
        let current = text.0.first().map(|span| span.content.as_str());
        if current != Some(rendered.as_str()) {
            *text = Text::from(rendered);
        }
    }
}

pub(super) fn sync_slider_track(
    mut sliders: Query<
        (&SliderValue, &SliderRange, &ComputedNodeRect, &mut Text),
        Or<(Changed<SliderValue>, Changed<ComputedNodeRect>)>,
    >,
) {
    for (value, range, node, mut text) in &mut sliders {
        let width = usize::from(node.content.width);
        if width == 0 {
            continue;
        }
        *text = Text::from(track(value.0, *range, width));
    }
}

fn track(value: f32, range: SliderRange, width: usize) -> String {
    let span = (range.end() - range.start()).max(f32::EPSILON);
    let ratio = ((value - range.start()) / span).clamp(0.0, 1.0);
    let thumb = (ratio * (width - 1) as f32).round() as usize;
    (0..width)
        .map(|cell| match cell.cmp(&thumb) {
            core::cmp::Ordering::Less => TRACK_BEFORE,
            core::cmp::Ordering::Equal => TRACK_THUMB,
            core::cmp::Ordering::Greater => TRACK_AFTER,
        })
        .collect()
}
