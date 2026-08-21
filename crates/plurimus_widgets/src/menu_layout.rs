//! Sizing and row placement for menu popups.
//!
//! A menu popup has to know its size before `popover` can decide where it
//! fits, so the width is measured from the widest item label - by display
//! width, not char count - and the height from the row count, both padded by
//! the frame. Once the popup has a placed rect, each item is given the row
//! inside it that the item's own hit-testing then uses.

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::Children;
use bevy_ecs::prelude::{Query, With, Without};
use plurimus_core::ratatui_core::layout::{Rect, Size};

use crate::menu::{MenuAccess, MenuItem, MenuPopup};
use crate::popover::Popover;
use plurimus_core::{ComputedUiCamera, UiArea};
use plurimus_ui::ComputedWidgetArea;
use plurimus_ui::UiLabel;

const POPUP_FRAME: u16 = 1;
const ITEM_PADDING: u16 = 1;

pub(crate) fn size_menu_popups(
    labels: Query<&UiLabel>,
    mut popups: Query<(Entity, &mut Popover), With<MenuPopup>>,
    menus: MenuAccess,
) {
    for (popup, mut popover) in &mut popups {
        let rows = menus.item_rows(popup);
        let widest = rows
            .iter()
            .filter_map(|&item| labels.get(item).ok())
            .map(|label| label.0.width() as u16)
            .max()
            .unwrap_or(0);
        let size = Size::new(
            widest + 2 * (POPUP_FRAME + ITEM_PADDING),
            rows.len() as u16 + 2 * POPUP_FRAME,
        );
        if popover.size != size {
            popover.size = size;
        }
    }
}

pub(crate) fn place_menu_items(
    popups: Query<
        (&UiArea, &ComputedWidgetArea, &ComputedUiCamera, &Children),
        (With<MenuPopup>, Without<MenuItem>),
    >,
    mut items: Query<(&mut UiArea, &mut ComputedWidgetArea, &mut ComputedUiCamera), With<MenuItem>>,
) {
    for (popup_area, popup_computed, camera, children) in &popups {
        let UiArea::Fixed(local) = *popup_area else {
            continue;
        };
        let mut index = 0;
        for &child in children {
            let Ok((mut area, mut computed, mut target)) = items.get_mut(child) else {
                continue;
            };
            area.set_if_neq(UiArea::Fixed(inner_row(local, index)));
            computed.set_if_neq(ComputedWidgetArea(inner_row(popup_computed.0, index)));
            // The popup itself may have taken its anchor's camera over the
            // hierarchy's; items follow the popup either way.
            target.set_if_neq(*camera);
            index += 1;
        }
    }
}

fn inner_row(popup: Rect, index: usize) -> Rect {
    Rect::new(
        popup.x.saturating_add(POPUP_FRAME),
        popup
            .y
            .saturating_add(POPUP_FRAME)
            .saturating_add(index as u16),
        popup.width.saturating_sub(2 * POPUP_FRAME),
        1,
    )
    .intersection(popup)
}
