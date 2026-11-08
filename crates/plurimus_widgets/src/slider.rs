//! The slider widget on `bevy_ui_widgets`' own [`Slider`]: track seek and
//! drag scrubbing at cell resolution, and a stylist.
//!
//! The keys are upstream's - the side arrows step by
//! [`SliderStep`](crate::SliderStep), `Home` and `End` jump - since its key
//! observer needs no layout. Its pointer observers measure a node, so a
//! slider at a cell rect is driven here instead: a press on the track jumps
//! to that position and a drag scrubs, both through the same track-ratio
//! conversion, so a click and a drag that end on the same cell produce the
//! same value. The slider emits
//! [`ValueChange`] and does not move itself unless
//! [`slider_self_update`](crate::slider_self_update) is attached.

use core::cmp::Ordering;

use bevy_ecs::bundle::Bundle;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, On, Query, Res, With, Without};
use bevy_input_focus::InputFocus;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::widgets::Widget;

use super::ValueChange;
use plurimus_core::CellSize;
use plurimus_core::UiWidget;
use plurimus_ui::bevy_picking::events::{PointerCancel, PointerClick, PointerDrag, PointerPress};
use plurimus_ui::bevy_picking::pointer::PointerButton;
use plurimus_ui::bevy_ui_widgets::{Slider, SliderRange, SliderValue};
use plurimus_ui::{ComputedDisabled, ComputedWidgetArea, Hovered, UiTheme, pointer_cell};
use plurimus_ui::{StateQuery, Stylable, StylistCache, hashed_bits, observed};

/// Spawn bundle for a slider over `start..=end` at `value`, stepping by
/// upstream's default [`SliderStep`](crate::SliderStep) of one.
///
/// The bounds are taken in either order, since upstream's range clamps
/// with them as given and a reversed pair would panic on the first step.
/// Upstream's `SliderDragState` stays set after a drag here, the observer
/// that clears it needing a node; nothing at a cell reads it.
#[must_use]
pub fn slider(start: f32, end: f32, value: f32) -> impl Bundle {
    (
        Slider::default(),
        SliderRange::new(start.min(end), start.max(end)),
        SliderValue(value),
        Hovered::default(),
        StylistCache::default(),
        TabIndex(0),
        UiWidget::default(),
    )
}

type SliderItem<'a> = (&'a ComputedWidgetArea, &'a SliderValue, &'a SliderRange);

type SliderQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ComputedWidgetArea,
        &'static SliderValue,
        &'static SliderRange,
    ),
    (With<Slider>, Without<ComputedDisabled>),
>;

pub(crate) fn slider_press(
    mut event: On<PointerPress>,
    cell: Res<CellSize>,
    sliders: SliderQuery,
    mut commands: Commands,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    let Ok(slider) = sliders.get(event.entity) else {
        return;
    };
    event.propagate(false);
    let x = pointer_cell(&event.pointer, *cell).x;
    seek(event.entity, slider, x, false, &mut commands);
}

pub(crate) fn slider_drag(
    mut event: On<PointerDrag>,
    cell: Res<CellSize>,
    sliders: SliderQuery,
    mut commands: Commands,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    let Ok(slider) = sliders.get(event.entity) else {
        return;
    };
    event.propagate(false);
    let x = pointer_cell(&event.pointer, *cell).x;
    seek(event.entity, slider, x, false, &mut commands);
}

// The click rather than the release: bevy sends a release to whatever the
// pointer is over, and a click only to what this pointer pressed.
pub(crate) fn slider_release(
    mut event: On<PointerClick>,
    cell: Res<CellSize>,
    sliders: SliderQuery,
    mut commands: Commands,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    let Ok(slider) = sliders.get(event.entity) else {
        return;
    };
    event.propagate(false);
    let x = pointer_cell(&event.pointer, *cell).x;
    seek(event.entity, slider, x, true, &mut commands);
}

// A cancelled drag commits rather than reverts, as a text field does on
// losing focus: the final change an app waits for still arrives.
pub(crate) fn slider_cancel(
    event: On<PointerCancel>,
    sliders: SliderQuery,
    mut commands: Commands,
) {
    if let Ok((_, value, _)) = sliders.get(event.entity) {
        commands.trigger(ValueChange {
            source: event.entity,
            value: value.0,
            is_final: true,
        });
    }
}

fn seek(
    entity: Entity,
    (area, value, range): SliderItem,
    x: u16,
    is_final: bool,
    commands: &mut Commands,
) {
    let target = track_value(area.0, *range, x);
    if is_final || (target - value.0).abs() > f32::EPSILON {
        commands.trigger(ValueChange {
            source: entity,
            value: target,
            is_final,
        });
    }
}

fn track_value(area: Rect, range: SliderRange, x: u16) -> f32 {
    let ratio = super::track_ratio(area.x, area.width, x);
    range.clamp(range.start() + ratio * range.span())
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
        if !cache.redraws(next, theme.is_changed()) {
            continue;
        }
        let style = next.style(&theme);
        let ratio = range.thumb_position(value.0).clamp(0.0, 1.0);
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
