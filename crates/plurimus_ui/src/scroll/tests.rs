use super::{ScrollArea, ScrollOffset, content_cell, max_offset, screen_cell, stepped_offset};
use plurimus_core::ratatui_core::buffer::{Buffer, Cell};
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::ratatui_core::widgets::StatefulWidget;
use tui_scrollview::{ScrollView, ScrollViewState, ScrollbarVisibility};

const AREA: Rect = Rect::new(4, 2, 10, 5);

#[test]
fn a_step_moves_the_offset_and_stops_at_either_bound() {
    assert_eq!(stepped_offset(10, 5, 100), 15);
    assert_eq!(stepped_offset(10, -5, 100), 5);
    assert_eq!(stepped_offset(10, -50, 100), 0);
    assert_eq!(stepped_offset(10, 500, 100), 100);
}

// A jump to an extreme is a step at the full range of its type, which
// must saturate at the bound rather than overflow reaching it.
#[test]
fn a_step_past_the_offsets_own_range_still_lands_on_the_bound() {
    assert_eq!(stepped_offset(u16::MAX - 1, i32::MAX, u16::MAX), u16::MAX);
    assert_eq!(stepped_offset(u16::MAX - 1, i32::MIN, u16::MAX), 0);
    assert_eq!(stepped_offset(0, i32::from(i16::MAX) + 1, u16::MAX), 32768);
}

#[test]
fn a_cell_inside_the_area_is_its_offset_from_the_origin() {
    assert_eq!(
        content_cell(Position::new(6, 3), AREA, None),
        Some(Position::new(2, 1))
    );
}

#[test]
fn the_scroll_offset_is_added_to_what_the_area_resolves() {
    assert_eq!(
        content_cell(
            Position::new(6, 3),
            AREA,
            Some(&ScrollOffset(Position::new(7, 20)))
        ),
        Some(Position::new(9, 21))
    );
}

// A captured drag reports cells outside the widget it began on; the
// nearest one is what keeps it selecting rather than collapsing.
#[test]
fn a_cell_outside_the_area_clamps_to_the_nearest_edge() {
    assert_eq!(
        content_cell(Position::new(0, 0), AREA, None),
        Some(Position::new(0, 0)),
        "above and left of the area"
    );
    assert_eq!(
        content_cell(Position::new(99, 99), AREA, None),
        Some(Position::new(9, 4)),
        "the last cell, not one past it"
    );
}

#[test]
fn a_content_cell_maps_back_to_the_screen_cell_it_came_from() {
    for screen in [
        Position::new(4, 2),
        Position::new(9, 5),
        Position::new(6, 3),
    ] {
        for offset in [None, Some(&ScrollOffset(Position::new(7, 20)))] {
            let content = content_cell(screen, AREA, offset).expect("inside the area");
            assert_eq!(screen_cell(content, AREA, offset), Some(screen));
        }
    }
}

// A caret whose character is scrolled off has no screen cell; answering
// with the nearest edge would draw it beside the wrong character.
#[test]
fn a_content_cell_outside_the_window_is_on_no_screen_cell() {
    let offset = Some(&ScrollOffset(Position::new(3, 4)));
    assert_eq!(screen_cell(Position::new(2, 5), AREA, offset), None, "left");
    assert_eq!(
        screen_cell(Position::new(5, 3), AREA, offset),
        None,
        "above"
    );
    assert_eq!(
        screen_cell(Position::new(13, 5), AREA, offset),
        None,
        "past the right edge"
    );
    assert_eq!(
        screen_cell(Position::new(5, 9), AREA, offset),
        None,
        "past the bottom edge"
    );
}

#[test]
fn an_empty_area_addresses_no_cell() {
    assert_eq!(content_cell(Position::new(4, 2), Rect::ZERO, None), None);
    assert_eq!(
        content_cell(Position::new(4, 2), Rect::new(4, 2, 0, 5), None),
        None,
        "zero width alone is enough"
    );
    assert_eq!(screen_cell(Position::new(0, 0), Rect::ZERO, None), None);
}

// tui-scrollview keeps this rule private, so the viewport is checked
// against the real thing: where it clamps and where the content shows.
#[test]
fn the_viewport_is_where_tui_scrollview_draws_and_clamps() {
    let area = Rect::new(2, 1, 5, 5);
    let visibilities = [
        ScrollbarVisibility::Automatic,
        ScrollbarVisibility::Always,
        ScrollbarVisibility::Never,
    ];
    let fits_exactly_and_overflows = 3..=7;
    for scrollbars in visibilities {
        for width in fits_exactly_and_overflows.clone() {
            for height in fits_exactly_and_overflows.clone() {
                let content = Size::new(width, height);
                let case = format!("{content:?} {scrollbars:?}");
                let viewport = ScrollArea::new(content)
                    .with_scrollbars(scrollbars)
                    .viewport(area);

                let mut state = ScrollViewState::with_offset(Position::new(u16::MAX, u16::MAX));
                let mut buffer = Buffer::empty(area);
                filled_view(content, scrollbars).render(area, &mut buffer, &mut state);

                assert_eq!(state.offset(), max_offset(content, viewport), "{case}");
                let shown = Rect {
                    width: viewport.width.min(width),
                    height: viewport.height.min(height),
                    ..viewport
                };
                for position in area.positions() {
                    let is_content = buffer[position].symbol() == CONTENT;
                    assert_eq!(
                        is_content,
                        shown.contains(position),
                        "{case} at {position:?}"
                    );
                }
            }
        }
    }
}

const CONTENT: &str = "x";

fn filled_view(content: Size, scrollbars: ScrollbarVisibility) -> ScrollView {
    let mut view = ScrollView::new(content).scrollbars_visibility(scrollbars);
    *view.buf_mut() = Buffer::filled(view.area(), Cell::new(CONTENT));
    view
}
