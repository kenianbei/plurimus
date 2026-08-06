//! Averaging colors correctly: accumulate in linear space, not in bytes.
//!
//! Several samples collapse into one color in a few places - braille dots
//! sharing a cell's single foreground, and the 3d pipeline folding a pair of
//! subcell samples into one. Averaging gamma-encoded sRGB bytes directly
//! drags mixed mid-tones darker than they belong, so [`LinearColorSum`]
//! converts each sample into linear space on the way in and back out again
//! when it resolves.

use bevy_color::{ColorToPacked, LinearRgba, Srgba};

/// Incremental color average over sRGB byte samples, accumulated in
/// linear space - averaging gamma-encoded bytes directly darkens mixed
/// mid-tones.
#[derive(Clone, Copy, Default)]
pub struct LinearColorSum {
    channels: [f32; 3],
    count: u32,
}

impl LinearColorSum {
    /// Accumulates one sRGB sample.
    pub fn add(&mut self, rgb: [u8; 3]) {
        for (sum, byte) in self.channels.iter_mut().zip(rgb) {
            *sum += Srgba::gamma_function(f32::from(byte) / 255.0);
        }
        self.count += 1;
    }

    /// The linear-space average, or `None` when nothing was added.
    #[must_use]
    pub fn resolve_linear(&self) -> Option<LinearRgba> {
        if self.count == 0 {
            return None;
        }
        let [red, green, blue] = self.channels.map(|sum| sum / self.count as f32);
        Some(LinearRgba::rgb(red, green, blue))
    }

    /// The average re-encoded to sRGB bytes, or `None` when nothing was
    /// added.
    #[must_use]
    pub fn resolve_srgb(&self) -> Option<[u8; 3]> {
        self.resolve_linear()
            .map(|linear| Srgba::from(linear).to_u8_array_no_alpha())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_sum_resolves_to_none() {
        let sum = LinearColorSum::default();

        assert!(sum.resolve_linear().is_none());
        assert!(sum.resolve_srgb().is_none());
    }

    #[test]
    fn single_sample_round_trips() {
        let mut sum = LinearColorSum::default();
        sum.add([255, 100, 0]);

        assert_eq!(sum.resolve_srgb(), Some([255, 100, 0]));
    }

    #[test]
    fn mixed_extremes_average_brighter_than_srgb_space() {
        let mut sum = LinearColorSum::default();
        sum.add([255, 255, 255]);
        sum.add([0, 0, 0]);

        assert_eq!(sum.resolve_srgb(), Some([188, 188, 188]));
    }
}
