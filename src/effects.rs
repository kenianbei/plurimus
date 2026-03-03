use bevy::prelude::{Commands, Component, Entity, NonSendMut, On, Remove};
use std::collections::HashMap;
use tachyonfx::{Effect, EffectManager};

/// A marker component for entities that have Tachyon effects applied to them.
/// This component is used to identify entities that should be managed by the Tachyon effect system.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct TachyonEffect;

/// A registry for managing Tachyon effects on entities. It maintains a mapping of entities to their corresponding effect managers.
#[derive(Default)]
pub struct TachyonRegistry {
    pub managers: HashMap<Entity, EffectManager<u64>>,
}

impl TachyonRegistry {
    pub fn manager_mut(&mut self, e: Entity) -> &mut EffectManager<u64> {
        self.managers.entry(e).or_default()
    }

    pub fn remove(&mut self, e: Entity) {
        self.managers.remove(&e);
    }
}

/// A system that observes the removal of the `TachyonEffect` component from entities.
/// When an entity has this component removed, it also removes the corresponding entry from the `TachyonRegistry`.
pub fn on_remove_tachyon_fx_target(
    trigger: On<Remove, TachyonEffect>,
    mut reg: NonSendMut<TachyonRegistry>,
) {
    reg.remove(trigger.entity)
}

/// A helper function to enable Tachyon effects on a given entity.
/// It inserts the `TachyonEffect` component and ensures that an effect manager is created for the entity in the registry.
/// See the example usage in the documentation for `add_fx` for how to use this function in practice.
pub fn enable_fx(commands: &mut Commands, reg: &mut TachyonRegistry, e: Entity) {
    commands.entity(e).insert(TachyonEffect);
    let _ = reg.manager_mut(e);
}

/// A helper function to add a Tachyon effect to a given entity.
/// It retrieves the effect manager for the entity from the registry and adds the specified effect to it.
///
/// Example usage:
/// ```rust
/// use bevy::prelude::*;
/// use plurimus::{add_fx, enable_fx, PlurimusPlugin, TachyonRegistry, Widget};
/// use tachyonfx::fx::explode;
///
/// fn main() {
///     App::new()
///         .add_plugins(MinimalPlugins)
///         .add_plugins(PlurimusPlugin)
///         .add_systems(Startup, startup);
/// }
///
/// fn startup(mut commands: Commands, mut reg: NonSendMut<TachyonRegistry>) {
///     use ratatui::widgets::Paragraph;
///     
///     let entity = commands.spawn((
///         Widget::from_widget(Paragraph::new("TachyonFX")),
///     )).id();
///     
///     enable_fx(&mut commands, &mut reg, entity);
///     add_fx(&mut reg, entity, explode(10.0, 3.0, 800));
/// }
/// ```
pub fn add_fx(reg: &mut TachyonRegistry, e: Entity, fx: Effect) {
    reg.manager_mut(e).add_effect(fx);
}
