//! Browser pointer and wheel events in cells.

use plurimus_core::ratatui_core::layout::Position;
use plurimus_term::{MouseButton, MouseKind};

/// Pixels a `WheelEvent` in pixel mode scrolls per wheel notch: what Chrome
/// and Edge report for one notch of a mouse wheel.
const PIXELS_PER_NOTCH: f64 = 100.0;

/// Lines a `WheelEvent` in line mode scrolls per wheel notch: Firefox's
/// default.
const LINES_PER_NOTCH: f64 = 3.0;

/// The most notches one wheel event turns into, so a flung trackpad does not
/// flood the scroll routers with hundreds of messages in one frame.
const MAX_NOTCHES_PER_EVENT: u32 = 10;

/// `WheelEvent.deltaMode` values.
const DELTA_LINE: u32 = 1;
const DELTA_PAGE: u32 = 2;

/// The cell under a pointer at `offset` CSS pixels from the canvas's
/// top-left, for cells `cell` CSS pixels in size on a `grid` of columns and
/// rows. Clamped into the grid, since a captured drag keeps reporting past
/// the canvas edge; `None` while there is no grid to address.
pub(crate) fn cell_at(offset: (f64, f64), cell: (f64, f64), grid: (u16, u16)) -> Option<Position> {
    let (columns, rows) = grid;
    if columns == 0 || rows == 0 || cell.0 <= 0.0 || cell.1 <= 0.0 {
        return None;
    }
    let column = (offset.0 / cell.0)
        .floor()
        .clamp(0.0, f64::from(columns - 1)) as u16;
    let row = (offset.1 / cell.1).floor().clamp(0.0, f64::from(rows - 1)) as u16;
    Some(Position::new(column, row))
}

/// `PointerEvent.button` to the button it names.
pub(crate) const fn button(index: i16) -> Option<MouseButton> {
    match index {
        0 => Some(MouseButton::Left),
        1 => Some(MouseButton::Middle),
        2 => Some(MouseButton::Right),
        3 => Some(MouseButton::Back),
        4 => Some(MouseButton::Forward),
        _ => None,
    }
}

/// The first button held in a `PointerEvent.buttons` mask, which is what
/// turns a move into a drag.
pub(crate) fn held_button(buttons: u16) -> Option<MouseButton> {
    const HELD: [(u16, MouseButton); 5] = [
        (1, MouseButton::Left),
        (2, MouseButton::Right),
        (4, MouseButton::Middle),
        (8, MouseButton::Back),
        (16, MouseButton::Forward),
    ];
    HELD.iter()
        .find(|&&(bit, _)| buttons & bit != 0)
        .map(|&(_, held)| held)
}

/// Turns wheel deltas into whole notches, carrying the remainder so a
/// trackpad's many small deltas still add up to scrolling.
#[derive(Debug, Default)]
pub(crate) struct WheelResidue {
    horizontal: f64,
    vertical: f64,
}

impl WheelResidue {
    /// The scroll messages one `WheelEvent` amounts to, in the order a
    /// terminal would report them: vertical first.
    pub(crate) fn notches(&mut self, delta: (f64, f64), mode: u32) -> Vec<MouseKind> {
        let per_notch = match mode {
            DELTA_LINE => LINES_PER_NOTCH,
            DELTA_PAGE => 1.0,
            _ => PIXELS_PER_NOTCH,
        };
        let vertical = take_notches(&mut self.vertical, delta.1 / per_notch);
        let horizontal = take_notches(&mut self.horizontal, delta.0 / per_notch);
        let mut kinds = Vec::new();
        push_notches(
            &mut kinds,
            vertical,
            MouseKind::ScrollDown,
            MouseKind::ScrollUp,
        );
        push_notches(
            &mut kinds,
            horizontal,
            MouseKind::ScrollRight,
            MouseKind::ScrollLeft,
        );
        kinds
    }
}

fn take_notches(residue: &mut f64, notches: f64) -> i64 {
    *residue += notches;
    let whole = residue.trunc();
    *residue -= whole;
    let cap = f64::from(MAX_NOTCHES_PER_EVENT);
    whole.clamp(-cap, cap) as i64
}

fn push_notches(
    kinds: &mut Vec<MouseKind>,
    notches: i64,
    positive: MouseKind,
    negative: MouseKind,
) {
    let kind = if notches > 0 { positive } else { negative };
    kinds.extend(std::iter::repeat_n(kind, notches.unsigned_abs() as usize));
}

#[cfg(test)]
mod tests {
    use super::*;

    const DELTA_PIXEL: u32 = 0;

    #[test]
    fn a_pointer_lands_in_the_cell_it_is_over() {
        let cell = (8.0, 16.0);
        assert_eq!(
            cell_at((0.0, 0.0), cell, (10, 5)),
            Some(Position::new(0, 0))
        );
        assert_eq!(
            cell_at((17.0, 33.0), cell, (10, 5)),
            Some(Position::new(2, 2))
        );
    }

    #[test]
    fn a_pointer_past_the_edge_clamps_into_the_grid() {
        let cell = (8.0, 16.0);
        assert_eq!(
            cell_at((-5.0, -1.0), cell, (10, 5)),
            Some(Position::new(0, 0))
        );
        assert_eq!(
            cell_at((999.0, 999.0), cell, (10, 5)),
            Some(Position::new(9, 4))
        );
    }

    #[test]
    fn no_grid_addresses_no_cell() {
        assert_eq!(cell_at((1.0, 1.0), (8.0, 16.0), (0, 5)), None);
        assert_eq!(cell_at((1.0, 1.0), (0.0, 16.0), (10, 5)), None);
    }

    #[test]
    fn buttons_map_by_dom_index_and_mask() {
        assert_eq!(button(0), Some(MouseButton::Left));
        assert_eq!(button(2), Some(MouseButton::Right));
        assert_eq!(button(9), None);
        assert_eq!(held_button(0), None);
        assert_eq!(held_button(4), Some(MouseButton::Middle));
        assert_eq!(held_button(1 | 2), Some(MouseButton::Left));
    }

    #[test]
    fn a_mouse_wheel_notch_is_one_scroll() {
        let mut residue = WheelResidue::default();
        assert_eq!(
            residue.notches((0.0, 100.0), DELTA_PIXEL),
            vec![MouseKind::ScrollDown]
        );
        assert_eq!(
            residue.notches((0.0, -3.0), DELTA_LINE),
            vec![MouseKind::ScrollUp]
        );
        assert_eq!(
            residue.notches((-100.0, 0.0), DELTA_PIXEL),
            vec![MouseKind::ScrollLeft]
        );
    }

    #[test]
    fn trackpad_deltas_accumulate_into_notches() {
        let mut residue = WheelResidue::default();
        for _ in 0..3 {
            assert!(residue.notches((0.0, 30.0), DELTA_PIXEL).is_empty());
        }
        assert_eq!(
            residue.notches((0.0, 30.0), DELTA_PIXEL),
            vec![MouseKind::ScrollDown]
        );
    }

    #[test]
    fn a_flung_wheel_is_capped() {
        let mut residue = WheelResidue::default();
        let kinds = residue.notches((0.0, 100_000.0), DELTA_PIXEL);
        assert_eq!(kinds.len(), MAX_NOTCHES_PER_EVENT as usize);
    }
}
