//! Per-camera visibility masks for 2d entities.
//!
//! Several cameras usually view one world, so an entity needs a way to say
//! which of them should see it - a minimap camera drawing markers the main
//! view omits, for instance. A camera draws an entity when their masks
//! intersect, and since both default to layer 0, an app that never touches
//! layers gets the obvious behaviour of everything seeing everything.

use bevy_ecs::prelude::Component;

const LAYER_COUNT: u8 = 64;

/// Camera-to-entity visibility mask: a camera draws an entity iff their
/// masks intersect. Entities and cameras without the component are on
/// layer 0; an explicit [`RenderLayers::none`] entity is invisible to
/// every camera. 64 layers.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderLayers(u64);

impl Default for RenderLayers {
    fn default() -> Self {
        Self::layer(0)
    }
}

impl RenderLayers {
    /// The mask containing only layer `n`.
    ///
    /// # Panics
    ///
    /// Panics when `n` is 64 or above.
    #[must_use]
    pub const fn layer(n: u8) -> Self {
        assert!(n < LAYER_COUNT, "RenderLayers supports layers 0..=63");
        Self(1 << n)
    }

    /// The empty mask; entities carrying it are drawn by no camera.
    #[must_use]
    pub const fn none() -> Self {
        Self(0)
    }

    /// This mask with layer `n` added.
    ///
    /// # Panics
    ///
    /// Panics when `n` is 64 or above.
    #[must_use]
    pub const fn with(self, n: u8) -> Self {
        Self(self.0 | Self::layer(n).0)
    }

    /// This mask with layer `n` removed.
    ///
    /// # Panics
    ///
    /// Panics when `n` is 64 or above.
    #[must_use]
    pub const fn without(self, n: u8) -> Self {
        Self(self.0 & !Self::layer(n).0)
    }

    /// Whether any layer is in both masks.
    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

#[cfg(test)]
mod tests {
    use super::RenderLayers;

    #[test]
    fn absent_component_semantics_default_to_layer_zero() {
        assert_eq!(RenderLayers::default(), RenderLayers::layer(0));
        assert!(RenderLayers::default().intersects(RenderLayers::default()));
    }

    #[test]
    fn distinct_layers_do_not_intersect() {
        assert!(!RenderLayers::layer(0).intersects(RenderLayers::layer(1)));
        assert!(RenderLayers::layer(63).intersects(RenderLayers::layer(63)));
    }

    #[test]
    fn with_and_without_edit_single_layers() {
        let mask = RenderLayers::layer(0).with(5).without(0);
        assert!(mask.intersects(RenderLayers::layer(5)));
        assert!(!mask.intersects(RenderLayers::layer(0)));
    }

    #[test]
    fn none_intersects_nothing() {
        assert!(!RenderLayers::none().intersects(RenderLayers::default()));
        assert!(!RenderLayers::none().intersects(RenderLayers::none()));
    }

    #[test]
    #[should_panic(expected = "0..=63")]
    fn layer_64_panics() {
        let _ = RenderLayers::layer(64);
    }
}
