//! The click count no terminal reports.
//!
//! A terminal reports a press as a press: no mouse encoding says that two of
//! them ran together, so the count a double-click gesture needs is made up
//! here, against the same real clock
//! [`ReleaseTimeout`](plurimus_term::ReleaseTimeout) expires held keys
//! against. Unlike a release no terminal tier reports one either, so there is
//! no capability for this to turn itself off on; a
//! [`MultiClickWindow`](plurimus_term::MultiClickWindow) of zero is how an app
//! turns it off instead.
//!
//! The run lives beside the pointer router because every input to its key is
//! that router's: which widget the press reached, whether an overlay swallowed
//! it, whether a disabled widget absorbed it. It is stepped inline rather than
//! by a system of its own, because two presses can arrive in one drained
//! batch, which state derived once a frame would report one count for.

use std::time::Duration;

use bevy_ecs::prelude::{Entity, Resource};
use plurimus_core::ratatui_core::layout::Position;

/// The run of presses the pointer is on.
///
/// One run for one pointer, counting whichever button the router routes
/// gestures for: stepped once per press that reaches a widget, and ended by
/// one that reaches none, so a press that dismissed an overlay and the press
/// after it on what was underneath are two lone presses rather than a double
/// click on a widget that has seen one.
#[derive(Resource, Debug, Default)]
pub(crate) struct ClickRun(Option<CountedPress>);

/// The press a run currently stands on: what it reached, where and when, and
/// which of the run it was.
#[derive(Debug)]
struct CountedPress {
    target: Entity,
    cell: Position,
    when: Duration,
    count: u8,
}

impl ClickRun {
    /// Counts a press on `target` at `cell` at `now`, answering how many
    /// presses have run together there.
    ///
    /// One more than the last press when that one reached the same target at
    /// the same cell strictly inside `window`, and one otherwise. Saturates at
    /// [`u8::MAX`]: what a run that long means is the widget's to decide, and
    /// wrapping back to zero would answer it with a lie.
    #[must_use]
    pub(crate) fn step(
        &mut self,
        target: Entity,
        cell: Position,
        now: Duration,
        window: Duration,
    ) -> u8 {
        let count = self
            .0
            .as_ref()
            .filter(|last| {
                last.target == target && last.cell == cell && now.saturating_sub(last.when) < window
            })
            .map_or(1, |last| last.count.saturating_add(1));
        self.0 = Some(CountedPress {
            target,
            cell,
            when: now,
            count,
        });
        count
    }

    /// Ends the run, so the next press counts from one.
    pub(crate) const fn reset(&mut self) {
        self.0 = None;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy_ecs::prelude::Entity;
    use plurimus_core::ratatui_core::layout::Position;

    use super::ClickRun;

    const WINDOW: Duration = Duration::from_millis(500);
    const CELL: Position = Position::new(4, 2);
    const OTHER: Position = Position::new(5, 2);

    fn target(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("a raw index is a valid entity")
    }

    fn at(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    #[test]
    fn presses_running_together_count_up() {
        let mut run = ClickRun::default();

        assert_eq!(run.step(target(1), CELL, at(0), WINDOW), 1);
        assert_eq!(run.step(target(1), CELL, at(100), WINDOW), 2);
        assert_eq!(run.step(target(1), CELL, at(200), WINDOW), 3);
    }

    // The window is measured from the last press rather than the first, so a
    // slow run keeps counting as long as no gap in it is too long.
    #[test]
    fn each_press_is_timed_against_the_last() {
        let mut run = ClickRun::default();

        assert_eq!(run.step(target(1), CELL, at(0), WINDOW), 1);
        assert_eq!(run.step(target(1), CELL, at(400), WINDOW), 2);
        assert_eq!(run.step(target(1), CELL, at(800), WINDOW), 3);
    }

    #[test]
    fn a_press_a_whole_window_later_starts_over() {
        let mut run = ClickRun::default();

        assert_eq!(run.step(target(1), CELL, at(0), WINDOW), 1);
        assert_eq!(run.step(target(1), CELL, at(500), WINDOW), 1);
    }

    #[test]
    fn a_zero_window_never_joins_a_run() {
        let mut run = ClickRun::default();

        assert_eq!(run.step(target(1), CELL, at(0), Duration::ZERO), 1);
        assert_eq!(run.step(target(1), CELL, at(0), Duration::ZERO), 1);
    }

    #[test]
    fn a_press_on_another_cell_starts_over() {
        let mut run = ClickRun::default();

        assert_eq!(run.step(target(1), CELL, at(0), WINDOW), 1);
        assert_eq!(run.step(target(1), OTHER, at(100), WINDOW), 1);
    }

    #[test]
    fn a_press_on_another_target_starts_over() {
        let mut run = ClickRun::default();

        assert_eq!(run.step(target(1), CELL, at(0), WINDOW), 1);
        assert_eq!(run.step(target(2), CELL, at(100), WINDOW), 1);
    }

    #[test]
    fn a_reset_run_counts_from_one() {
        let mut run = ClickRun::default();

        assert_eq!(run.step(target(1), CELL, at(0), WINDOW), 1);
        run.reset();
        assert_eq!(run.step(target(1), CELL, at(100), WINDOW), 1);
    }

    #[test]
    fn a_long_run_saturates() {
        let mut run = ClickRun::default();
        let mut counted = 0;
        for press in 0..u16::from(u8::MAX) {
            counted = run.step(target(1), CELL, at(u64::from(press)), WINDOW);
        }

        assert_eq!(counted, u8::MAX, "255 presses reach the ceiling");
        assert_eq!(
            run.step(target(1), CELL, at(u64::from(u8::MAX)), WINDOW),
            u8::MAX,
            "and the next stays there rather than wrapping to nothing"
        );
    }
}
