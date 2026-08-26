//! Placement of a bar's items along its axis.
//!
//! The bar assigns every item its rect each frame, the way a menu popup
//! places its rows: an item's area is where the bar put it, never something
//! the app set. The rects come from one pure function over the look, the
//! bar's rect and the label widths, so a test can check the geometry
//! without an app.

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::hierarchy::Children;
use bevy_ecs::prelude::{Has, Query, With, Without};
use plurimus_core::ratatui_core::layout::Rect;

use super::{TabBar, TabBarLook, TabBarOrientation, TabItem};
use plurimus_core::{CameraViewports, ComputedUiCamera, UiArea, UiHidden, UiOrder, local_area};
use plurimus_ui::{ComputedWidgetArea, UiLabel};

/// The cell a divider takes between two items.
pub(crate) const DIVIDER_CELLS: u16 = 1;

type Bars<'w, 's> = Query<
    'w,
    's,
    (
        &'static ComputedWidgetArea,
        &'static ComputedUiCamera,
        &'static TabBarLook,
        Option<&'static UiOrder>,
        Has<UiHidden>,
        &'static Children,
    ),
    (With<TabBar>, Without<TabItem>),
>;

type Items<'w, 's> = Query<
    'w,
    's,
    (
        &'static UiLabel,
        &'static mut UiArea,
        &'static mut ComputedWidgetArea,
        &'static mut UiOrder,
    ),
    With<TabItem>,
>;

// Items draw one order above the bar, whatever the app gave it, so the
// chrome never covers a label; a hidden bar places its items nowhere, since
// they are separate entities the bar's own `UiHidden` does not reach.
pub(crate) fn place_tab_items(cameras: CameraViewports, bars: Bars, mut items: Items) {
    for (bar_area, camera, look, order, hidden, children) in &bars {
        let viewport = cameras.of(camera.0);
        let above = UiOrder(order.map_or(0, |order| order.0).saturating_add(1));
        let widths: Vec<u16> = children
            .iter()
            .filter_map(|&child| items.get(child).ok())
            .map(|(label, ..)| u16::try_from(label.0.width()).unwrap_or(u16::MAX))
            .collect();
        let bar = if hidden { Rect::ZERO } else { bar_area.0 };
        let mut rects = item_rects(look, bar, &widths).into_iter();
        for &child in children {
            let Ok((_, mut area, mut computed, mut item_order)) = items.get_mut(child) else {
                continue;
            };
            let rect = rects.next().unwrap_or(Rect::ZERO);
            computed.set_if_neq(ComputedWidgetArea(rect));
            let local = viewport.map_or(rect, |viewport| local_area(rect, viewport));
            area.set_if_neq(UiArea::Fixed(local));
            item_order.set_if_neq(above);
        }
    }
}

/// Where each item of a bar drawn at `bar` lands, in `bar`'s coordinates,
/// one rect per label width in order. An item that does not fit whole is
/// [`Rect::ZERO`], and so is every item after it.
pub(crate) fn item_rects(look: &TabBarLook, bar: Rect, widths: &[u16]) -> Vec<Rect> {
    let gap = if look.divider.is_some() {
        DIVIDER_CELLS
    } else {
        0
    };
    let thickness = look.thickness();
    let mut cursor = 0u16;
    widths
        .iter()
        .map(|&width| {
            let (rect, advance) = match look.orientation {
                TabBarOrientation::Horizontal => {
                    let width = width.saturating_add(2 * (look.padding + look.frame()));
                    (Rect::new(cursor, 0, width, thickness), width)
                }
                TabBarOrientation::Vertical => {
                    (Rect::new(0, cursor, bar.width, thickness), thickness)
                }
            };
            cursor = cursor.saturating_add(advance).saturating_add(gap);
            let placed = Rect::new(
                bar.x.saturating_add(rect.x),
                bar.y.saturating_add(rect.y),
                rect.width,
                rect.height,
            );
            if placed.intersection(bar) == placed && !placed.is_empty() {
                placed
            } else {
                Rect::ZERO
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_widgets::borders::BorderType;

    fn look() -> TabBarLook {
        TabBarLook::default()
    }

    #[test]
    fn horizontal_items_follow_each_other_padded() {
        let rects = item_rects(&look(), Rect::new(2, 1, 20, 1), &[3, 4]);
        assert_eq!(rects, [Rect::new(2, 1, 5, 1), Rect::new(7, 1, 6, 1)]);
    }

    #[test]
    fn divider_leaves_a_cell_between_items() {
        let look = look().with_divider(Some("|".into()));
        let rects = item_rects(&look, Rect::new(0, 0, 20, 1), &[3, 3]);
        assert_eq!(rects, [Rect::new(0, 0, 5, 1), Rect::new(6, 0, 5, 1)]);
    }

    #[test]
    fn border_makes_items_three_thick_and_wider() {
        let look = look().with_border(Some(BorderType::Plain));
        let rects = item_rects(&look, Rect::new(0, 0, 20, 3), &[3]);
        assert_eq!(rects, [Rect::new(0, 0, 7, 3)]);
    }

    #[test]
    fn vertical_items_stack_at_the_bar_width() {
        let look = look().with_orientation(TabBarOrientation::Vertical);
        let rects = item_rects(&look, Rect::new(1, 1, 10, 5), &[3, 8]);
        assert_eq!(rects, [Rect::new(1, 1, 10, 1), Rect::new(1, 2, 10, 1)]);
    }

    #[test]
    fn an_item_past_the_bar_is_nowhere_and_so_are_the_rest() {
        let rects = item_rects(&look(), Rect::new(0, 0, 8, 1), &[2, 2, 2]);
        assert_eq!(
            rects,
            [Rect::new(0, 0, 4, 1), Rect::new(4, 0, 4, 1), Rect::ZERO]
        );
    }

    #[test]
    fn a_bar_too_thin_for_boxes_draws_none() {
        let look = look().with_border(Some(BorderType::Plain));
        assert_eq!(
            item_rects(&look, Rect::new(0, 0, 20, 1), &[3]),
            [Rect::ZERO]
        );
    }
}
