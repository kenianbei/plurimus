//! The slider widget: keyboard steps, track seek, and drag scrubbing.
//!
//! Three input routes converge on one value: arrow keys step by the slider's
//! step size, a press on the track jumps to that position, and a drag scrubs
//! continuously. All three go through the same track-ratio conversion, so a
//! click and a drag that end on the same cell produce the same value. The
//! slider emits [`ValueChange`] and does not move itself unless the stock
//! observer is attached.

use core::cmp::Ordering;

use bevy_ecs::bundle::Bundle;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Component, On, Query, Res, With, Without};
use bevy_input::ButtonState;
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::FocusedInput;
use bevy_input_focus::InputFocus;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::widgets::Widget;

use super::ValueChange;
use crate::placeholder;
use crate::stylist::{StateQuery, Stylable, StylistCache, hashed_bits, observed};
use crate::theme::UiTheme;
use plurimus_core::UiWidget;
use plurimus_ui::{
    ComputedWidgetArea, Hovered, InteractionDisabled, PointerDrag, PointerPress, PointerRelease,
};

/// A horizontal slider. Emits [`ValueChange<f32>`]; attach
/// [`super::slider_self_update`] for uncontrolled behavior.
#[derive(Component, Debug, Clone, Copy)]
#[require(
    SliderValue,
    SliderRange,
    SliderStep,
    Hovered,
    crate::stylist::StylistCache
)]
pub struct Slider;

/// The slider's current value.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct SliderValue(pub f32);

/// The slider's value range.
#[derive(Component, Debug, Clone, Copy)]
pub struct SliderRange {
    start: f32,
    end: f32,
}

impl SliderRange {
    /// A range from `start` to `end`.
    #[must_use]
    pub const fn new(start: f32, end: f32) -> Self {
        Self { start, end }
    }

    /// Lower bound.
    #[must_use]
    pub const fn start(&self) -> f32 {
        self.start
    }

    /// Upper bound.
    #[must_use]
    pub const fn end(&self) -> f32 {
        self.end
    }

    const fn clamp(self, value: f32) -> f32 {
        value.clamp(self.start, self.end)
    }
}

impl Default for SliderRange {
    fn default() -> Self {
        Self::new(0.0, 1.0)
    }
}

/// Keyboard adjustment increment.
#[derive(Component, Debug, Clone, Copy)]
pub struct SliderStep(pub f32);

impl Default for SliderStep {
    fn default() -> Self {
        Self(0.1)
    }
}

/// Spawn bundle for a slider over `start..=end` at `value`.
#[must_use]
pub fn slider(start: f32, end: f32, value: f32) -> impl Bundle {
    (
        Slider,
        SliderRange::new(start, end),
        SliderValue(value),
        TabIndex(0),
        placeholder(),
    )
}

type SliderQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ComputedWidgetArea,
        &'static SliderValue,
        &'static SliderRange,
    ),
    With<Slider>,
>;

pub(crate) fn slider_press(event: On<PointerPress>, sliders: SliderQuery, mut commands: Commands) {
    seek(
        (event.entity, event.position.x),
        false,
        &sliders,
        &mut commands,
    );
}

pub(crate) fn slider_drag(event: On<PointerDrag>, sliders: SliderQuery, mut commands: Commands) {
    seek(
        (event.entity, event.position.x),
        false,
        &sliders,
        &mut commands,
    );
}

pub(crate) fn slider_release(
    event: On<PointerRelease>,
    sliders: SliderQuery,
    mut commands: Commands,
) {
    seek(
        (event.entity, event.position.x),
        true,
        &sliders,
        &mut commands,
    );
}

fn seek(
    (entity, x): (Entity, u16),
    is_final: bool,
    sliders: &SliderQuery,
    commands: &mut Commands,
) {
    let Ok((area, value, range)) = sliders.get(entity) else {
        return;
    };
    let target = track_value(area.0, *range, x);
    emit(entity, value.0, target, is_final, commands);
}

pub(crate) fn slider_key(
    mut input: On<FocusedInput<KeyboardInput>>,
    sliders: Query<
        (&SliderValue, &SliderRange, &SliderStep),
        (With<Slider>, Without<InteractionDisabled>),
    >,
    mut commands: Commands,
) {
    let Ok((value, range, step)) = sliders.get(input.focused_entity) else {
        return;
    };
    if input.input.state != ButtonState::Pressed {
        return;
    }
    let delta = match input.input.logical_key {
        Key::ArrowLeft => -step.0,
        Key::ArrowRight => step.0,
        _ => return,
    };
    input.propagate(false);
    let target = range.clamp(value.0 + delta);
    emit(input.focused_entity, value.0, target, true, &mut commands);
}

fn track_value(area: Rect, range: SliderRange, x: u16) -> f32 {
    let ratio = super::track_ratio(area.x, area.width, x);
    range.clamp(range.start + ratio * (range.end - range.start))
}

fn emit(entity: Entity, current: f32, value: f32, is_final: bool, commands: &mut Commands) {
    if is_final || (value - current).abs() > f32::EPSILON {
        commands.trigger(ValueChange {
            source: entity,
            value,
            is_final,
        });
    }
}

pub(crate) fn style_sliders(
    theme: Res<UiTheme>,
    focus: Res<InputFocus>,
    mut sliders: Query<
        (
            StateQuery,
            &SliderValue,
            &SliderRange,
            &mut StylistCache,
            &mut UiWidget,
        ),
        Stylable<Slider>,
    >,
) {
    for (state, value, range, mut cache, mut widget) in &mut sliders {
        let next = observed(state, &focus, hashed_bits(value.0.to_bits()));
        if !theme.is_changed() && next == *cache {
            continue;
        }
        *cache = next;
        let style = next.style(&theme);
        let span = (range.end() - range.start()).max(f32::EPSILON);
        let ratio = ((value.0 - range.start()) / span).clamp(0.0, 1.0);
        *widget = UiWidget::new(SliderTrack { ratio, style });
    }
}

/// Track visual: `━━━█────`, thumb positioned by ratio.
struct SliderTrack {
    ratio: f32,
    style: Style,
}

impl Widget for &SliderTrack {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let y = area.top() + area.height / 2;
        let last = area.width.saturating_sub(1);
        let thumb_offset = (self.ratio * f32::from(last)).round();
        let thumb = area.x + thumb_offset as u16;
        for x in area.left()..area.right() {
            let symbol = match x.cmp(&thumb) {
                Ordering::Equal => "█",
                Ordering::Less => "━",
                Ordering::Greater => "─",
            };
            if let Some(cell) = buffer.cell_mut((x, y)) {
                cell.set_symbol(symbol);
                cell.set_style(self.style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use plurimus_core::ratatui_core::layout::Rect;

    use super::{SliderRange, track_value};

    #[test]
    fn track_value_maps_cells_to_range() {
        let area = Rect::new(2, 0, 11, 1);
        let range = SliderRange::new(0.0, 100.0);
        assert!((track_value(area, range, 2) - 0.0).abs() < f32::EPSILON);
        assert!((track_value(area, range, 7) - 50.0).abs() < f32::EPSILON);
        assert!((track_value(area, range, 12) - 100.0).abs() < f32::EPSILON);
        assert!((track_value(area, range, 40) - 100.0).abs() < f32::EPSILON);
    }
}
