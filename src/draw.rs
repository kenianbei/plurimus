use bevy::prelude::{BevyError, Entity, Result};
use bevy_ecs::prelude::{Query, ResMut};
use bevy_ratatui::RatatuiContext;
use ratatui::Frame;
use ratatui::prelude::Rect;

#[cfg(feature = "tachyonfx")]
use {
    crate::effects::{TachyonEffect, TachyonRegistry},
    bevy::prelude::{NonSendMut, Res, Time},
};

/// Trait for defining the area an entity should be drawn in. If not implemented, the entire frame will be used.
/// This allows for flexible drawing of entities in specific regions of the terminal.
///
/// Example:
/// ```rust
/// use bevy::prelude::*;
/// use plurimus::DrawArea;
/// use ratatui::prelude::Rect;
///
/// #[derive(Component)]
/// struct MyComponent(pub Rect);
/// impl DrawArea for MyComponent {
///     fn area(&self) -> Rect {
///       self.0
///     }
/// }
#[bevy_trait_query::queryable]
pub trait DrawArea {
    fn area(&self) -> Rect;
}

/// Trait for defining the draw order of an entity. Entities with lower order values will be drawn first. If not implemented, the default order is 0.
/// This allows for layering of entities, where those with higher order values will be drawn on top of those with lower values.
///
/// Example:
/// ```rust
/// use bevy::prelude::*;
/// use plurimus::DrawOrder;
///
/// #[derive(Component)]
/// struct MyComponent(pub i32);
/// impl DrawOrder for MyComponent {
///     fn order(&self) -> i32 {
///        self.0
///     }
/// }
#[bevy_trait_query::queryable]
pub trait DrawOrder {
    fn order(&self) -> i32;
}

/// Trait for defining the drawing logic of an entity. The `draw` method will be called with a mutable reference to the frame and the area it should be drawn in.
/// Implementing this trait allows an entity to be rendered on the terminal using Ratatui.
///
/// Example:
/// ```rust
/// use bevy::prelude::*;
/// use plurimus::DrawFn;
/// use ratatui::prelude::{Frame, Rect};
///
/// #[derive(Component)]
/// struct MyComponent;
/// impl DrawFn for MyComponent {
///     fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result {
///         // Drawing logic here
///         Ok(())
///     }
/// }
#[bevy_trait_query::queryable]
pub trait DrawFn {
    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result;
}

/// System that handles drawing all entities that implement the `DrawFn` trait.
/// It sorts entities based on their `DrawOrder` and draws them in the correct order.
pub fn draw(
    mut ratatui_context: ResMut<RatatuiContext>,
    q_entities: Query<Entity>,
    q_order: Query<&dyn DrawOrder>,
    q_area: Query<&dyn DrawArea>,
    mut q_render: Query<&mut dyn DrawFn>,
    #[cfg(feature = "tachyonfx")] q_fx_target: Query<&TachyonEffect>,
    #[cfg(feature = "tachyonfx")] time: Res<Time>,
    #[cfg(feature = "tachyonfx")] mut tachyon: Option<NonSendMut<TachyonRegistry>>,
) -> Result {
    let mut items: Vec<(i32, Entity)> = Vec::new();
    for e in q_entities.iter() {
        let order = match q_order.get(e) {
            Ok(read_traits) => read_traits.iter().map(|t| t.order()).min().unwrap_or(0),
            Err(_) => 0,
        };
        items.push((order, e));
    }
    items.sort_by_key(|(o, _)| *o);

    let mut errors: Vec<BevyError> = Vec::new();

    ratatui_context.draw(|frame| {
        #[cfg(feature = "tachyonfx")]
        let elapsed = time.delta();

        for (_, entity) in items.iter().copied() {
            let area = match q_area.get(entity) {
                Ok(read_traits) => read_traits
                    .iter()
                    .next()
                    .map(|t| t.area())
                    .unwrap_or_else(|| frame.area()),
                Err(_) => frame.area(),
            };

            if let Ok(mut write_traits) = q_render.get_mut(entity) {
                for mut draw_fn in write_traits.iter_mut() {
                    if let Err(e) = draw_fn.draw(frame, area) {
                        errors.push(BevyError::from(format!(
                            "Error drawing entity {:?}: {}",
                            entity, e
                        )));
                    }
                }
            }

            #[cfg(feature = "tachyonfx")]
            if q_fx_target.get(entity).is_ok()
                && let Some(reg) = tachyon.as_mut()
            {
                reg.manager_mut(entity)
                    .process_effects(elapsed.into(), frame.buffer_mut(), area);
            }
        }
    })?;

    if !errors.is_empty() {
        return Err(BevyError::from(format!(
            "Errors during drawing: {:?}",
            errors
        )));
    }

    Ok(())
}
