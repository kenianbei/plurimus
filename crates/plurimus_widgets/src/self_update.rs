//! Stock observers for uncontrolled widgets, mirroring `bevy_ui_widgets`.
//!
//! Widgets emit [`ValueChange`] and change nothing themselves, which leaves
//! an app free to validate, reject or transform an edit before it lands. That
//! costs a handler for every widget an app does not care to control, so these
//! observers supply the obvious behaviour - apply the value - and an app
//! attaches them per widget. Controlled and uncontrolled widgets can sit side
//! by side in one tree.

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Children, Commands, Has, On, Query, With};

use super::{Table, TableMultiSelect, TablePosition, TableRow, ValueChange};
use plurimus_ui::Checked;

/// Applies table selection to [`Checked`]: single-select moves it among the
/// rows, multi-select toggles the selected one. A selection that tracks only
/// a column carries no row, and changes nothing.
pub fn table_self_update(
    change: On<ValueChange<TablePosition>>,
    tables: Query<(&Children, Has<TableMultiSelect>), With<Table>>,
    rows: Query<Has<Checked>, With<TableRow>>,
    mut commands: Commands,
) {
    let Ok((children, multi_select)) = tables.get(change.source) else {
        return;
    };
    let Some(selected) = change.value.row else {
        return;
    };
    if multi_select {
        if let Ok(checked) = rows.get(selected) {
            set_checked(selected, !checked, &mut commands);
        }
        return;
    }
    for &child in children {
        if rows.contains(child) {
            set_checked(child, child == selected, &mut commands);
        }
    }
}

fn set_checked(entity: Entity, checked: bool, commands: &mut Commands) {
    if checked {
        commands.entity(entity).insert(Checked);
    } else {
        commands.entity(entity).remove::<Checked>();
    }
}
