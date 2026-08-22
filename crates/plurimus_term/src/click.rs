//! How long a run of presses stays one gesture.

use std::time::Duration;

use bevy_ecs::prelude::Resource;

/// How soon after a press another has to land to run with it.
///
/// One resource for the whole app, beside
/// [`ReleaseTimeout`](crate::ReleaseTimeout) and read against the same clock:
/// two widgets disagreeing about what a double click is would be a difference
/// the user feels rather than one anybody chose. [`Duration::ZERO`] turns runs
/// off, the test being strictly inside the window and two presses in one frame
/// being no time apart at all.
///
/// The run this bounds is kept by whoever routes presses, since what a press
/// reached is half of what keys it, and no terminal reports a count for this
/// crate to relay.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiClickWindow(pub Duration);

impl Default for MultiClickWindow {
    fn default() -> Self {
        Self(Duration::from_millis(500))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::MultiClickWindow;

    #[test]
    fn the_default_window_is_half_a_second() {
        assert_eq!(MultiClickWindow::default().0, Duration::from_millis(500));
    }
}
