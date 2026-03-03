use crate::widget::area::{WidgetLayout, WidgetRect};
use bevy::prelude::{
    Add, BevyError, Changed, DetectChanges, On, ParamSet, Query, Res, ResMut, Resource, Result,
};
use bevy_ratatui::RatatuiContext;
use ratatui::prelude::Rect;

#[derive(Default, Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct TerminalSize {
    pub width: u16,
    pub height: u16,
}

pub fn update_size(
    r_ratatui_context: Res<RatatuiContext>,
    mut r_terminal_size: ResMut<TerminalSize>,
) -> Result {
    if !r_ratatui_context.is_changed() {
        return Ok(());
    }

    let Ok(size) = r_ratatui_context.size() else {
        return Err(BevyError::from("RatatuiContext size not available"));
    };

    if size.width == r_terminal_size.width && size.height == r_terminal_size.height {
        return Ok(());
    }

    r_terminal_size.width = size.width;
    r_terminal_size.height = size.height;

    Ok(())
}

pub fn on_add_widget_rect(
    trigger: On<Add, WidgetRect>,
    r_terminal_size: Res<TerminalSize>,
    mut query: Query<(&WidgetLayout, &mut WidgetRect)>,
) -> Result {
    let Ok((layout, mut rect)) = query.get_mut(trigger.entity) else {
        return Ok(());
    };

    rect.0 = layout.0(&Rect::new(
        0,
        0,
        r_terminal_size.width,
        r_terminal_size.height,
    ));

    Ok(())
}

pub fn update_rects(
    r_terminal_size: Res<TerminalSize>,
    mut params: ParamSet<(
        Query<(&WidgetLayout, &mut WidgetRect), Changed<WidgetLayout>>,
        Query<(&WidgetLayout, &mut WidgetRect)>,
    )>,
) -> Result {
    if !r_terminal_size.is_changed() && params.p0().is_empty() {
        return Ok(());
    }

    let root = Rect::new(0, 0, r_terminal_size.width, r_terminal_size.height);

    if r_terminal_size.is_changed() {
        for (layout, mut rect) in params.p1().iter_mut() {
            rect.0 = layout.0(&root);
        }
        return Ok(());
    }

    for (layout, mut rect) in params.p0().iter_mut() {
        rect.0 = layout.0(&root);
    }

    Ok(())
}
