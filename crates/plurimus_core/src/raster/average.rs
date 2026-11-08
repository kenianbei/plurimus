//! Averaging colors correctly: in linear space, not in bytes.
//!
//! Several samples collapse into one color in a few places - braille dots
//! sharing a cell's single foreground, and the 3d pipeline folding a block of
//! pixels into one. Averaging gamma-encoded sRGB bytes directly drags mixed
//! mid-tones darker than they belong, so [`linear_average`] converts each
//! sample into linear space before summing.

use bevy_color::{ColorToPacked, LinearRgba, Srgba};

/// The linear-space mean of sRGB byte samples, or `None` for no samples.
///
/// Bound for a cell, it is [`linear_cell_color`](super::linear_cell_color)'s
/// input; bound for bytes, `Srgba::from(..)`'s.
#[must_use]
pub fn linear_average(samples: impl IntoIterator<Item = [u8; 3]>) -> Option<LinearRgba> {
    let (sum, count) = samples
        .into_iter()
        .fold((LinearRgba::NONE, 0u32), |(sum, count), rgb| {
            (sum + Srgba::from_u8_array_no_alpha(rgb).into(), count + 1)
        });
    (count > 0).then(|| sum / count as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn srgb_average(samples: impl IntoIterator<Item = [u8; 3]>) -> Option<[u8; 3]> {
        linear_average(samples).map(|linear| Srgba::from(linear).to_u8_array_no_alpha())
    }

    #[test]
    fn no_samples_average_to_none() {
        assert!(linear_average([]).is_none());
    }

    #[test]
    fn single_sample_round_trips() {
        assert_eq!(srgb_average([[255, 100, 0]]), Some([255, 100, 0]));
    }

    #[test]
    fn mixed_extremes_average_brighter_than_srgb_space() {
        assert_eq!(
            srgb_average([[255, 255, 255], [0, 0, 0]]),
            Some([188, 188, 188])
        );
    }
}
