use crate::draw::{DrawArea, DrawFn, DrawOrder};
use crate::plugin::PlurimusFixedSet;
use crate::widget::area::WidgetRect;
use crate::widget::order::WidgetOrder;
use crate::widget::size::{TerminalSize, on_add_widget_rect, update_rects, update_size};
use crate::widget::widget::Widget;
use bevy::prelude::{App, FixedUpdate, IntoScheduleConfigs, Plugin};
use bevy_ecs::prelude::resource_exists;
use bevy_ratatui::RatatuiContext;

pub mod area;
pub mod order;
pub mod size;
pub mod widget;

#[derive(Default)]
pub struct PlurimusWidgetPlugin;

impl Plugin for PlurimusWidgetPlugin {
    fn build(&self, app: &mut App) {
        use bevy_trait_query::RegisterExt;
        app.register_component_as::<dyn DrawArea, WidgetRect>();
        app.register_component_as::<dyn DrawFn, Widget>();
        app.register_component_as::<dyn DrawOrder, WidgetOrder>();

        app.init_resource::<TerminalSize>();
        app.add_observer(on_add_widget_rect);

        app.add_systems(
            FixedUpdate,
            (update_size, update_rects)
                .chain()
                .in_set(PlurimusFixedSet::Layout)
                .run_if(resource_exists::<RatatuiContext>),
        );
    }
}
