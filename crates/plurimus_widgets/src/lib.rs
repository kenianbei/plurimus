//! Standard widgets mirroring `bevy_ui_widgets` - the settled strategy:
//! upstream's component vocabulary and event contract over terminal-native
//! engines. Most widgets are stateless controllers emitting entity events,
//! applied by the app or the stock `*_self_update` observers; the text
//! widgets own their editing state and mutate-then-notify.
//!
//! A widget is therefore three separable things: components describing what
//! it is, systems turning input into events, and a stylist that rebuilds its
//! [`UiWidget`](plurimus_core::UiWidget) from [`UiTheme`](plurimus_ui::UiTheme) when the state it last
//! drew changes. An app that wants different looks replaces the stylist and
//! keeps the behaviour; one that wants different behaviour handles the events
//! itself and keeps the look.

mod activate;
mod button;
mod checkbox;
mod listbox;
mod listbox_style;
mod menu;
mod menu_layout;
mod pane;
mod popover;
mod radio;
mod rows;
mod scrollbar;
mod self_update;
mod slider;
mod table;
mod text;

pub use bevy_input::keyboard::Key;
pub use plurimus_ui::ValueChange;
pub use ratatui_textarea;
pub use ratatui_widgets;

pub use activate::Activate;
pub use button::{Button, button};
pub use checkbox::{Checkbox, checkbox};
pub use listbox::{
    ListBox, ListBoxAction, ListBoxCursor, ListBoxKeys, ListBoxMultiSelect, ListBoxSelectionMarker,
    ListItem, list_item, listbox,
};
pub use menu::{MenuButton, MenuItem, MenuOpen, MenuPopup, menu_button, menu_item, menu_popup};
pub use pane::{Pane, pane};
pub use popover::{Popover, PopoverAlign, PopoverSide};
pub use radio::{RadioButton, RadioGroup, radio};
pub use rows::{ActiveDescendant, ListItemText, ListItemTrailing, Marked};
pub use scrollbar::{Scrollbar, scrollbar};
pub use self_update::{
    checkbox_self_update, listbox_self_update, radio_self_update, slider_self_update,
    table_self_update,
};
pub use slider::{Slider, SliderRange, SliderStep, SliderValue, slider};
pub use table::{
    ActiveColumn, Table, TableAction, TableCheckedStyle, TableColumns, TableCursor, TableFooter,
    TableHeader, TableHeaderClick, TableKeys, TableLayout, TableMultiSelect, TablePosition,
    TableRow, TableSelection, TableStripe, table, table_footer, table_header, table_row,
};
pub use text::{
    EditableText, Submit, TextChanged, TextEditor, TextInput, editable_text, text_editor,
};

pub(crate) use activate::{is_activate_key, widget_click, widget_key};
pub(crate) use button::style_buttons;
pub(crate) use checkbox::style_checkboxes;
pub(crate) use listbox::{
    listbox_click, listbox_drag, listbox_key, listbox_press, reveal_listbox_cursor,
};
pub(crate) use listbox_style::{ListRowsChanged, ListSelfChanged, style_listboxes};
pub(crate) use menu::{
    menu_button_activate, menu_dismiss, menu_item_click, menu_key, style_menu_items,
    style_menu_popups,
};
pub(crate) use menu_layout::{place_menu_items, size_menu_popups};
pub(crate) use pane::style_panes;
pub(crate) use popover::{adopt_anchor_cameras, place_popovers};
pub(crate) use radio::style_radios;
pub(crate) use rows::{mark_dirty_content, repair_active_descendants, sync_row_scroll};
pub(crate) use scrollbar::{scrollbar_drag, scrollbar_press, scrollbar_release, style_scrollbars};
pub(crate) use slider::{slider_drag, slider_key, slider_press, slider_release, style_sliders};
pub(crate) use table::{
    TableBodyRow, TableRowsChanged, TableSelfChanged, reveal_table_cursor, style_tables,
    table_click, table_key,
};
pub(crate) use text::{
    install_editor_views, style_text_inputs, text_editor_key, text_editor_paste,
    text_editor_scrolled, text_input_blur, text_input_key, text_input_paste,
};

use bevy_app::{App, Plugin, PreUpdate, Update};
use bevy_ecs::prelude::{IntoScheduleConfigs, With};
use bevy_ecs::schedule::SystemSet;
use bevy_input_focus::InputFocusSystems;

use plurimus_core::CameraSystems;
use plurimus_ui::{UiPlugin, UiSystems};

// Shared track convention: first cell is 0.0, last is 1.0, outside clamps.
pub(crate) fn track_ratio(start: u16, length: u16, pointer: u16) -> f32 {
    let last = length.saturating_sub(1).max(1);
    let cell = pointer.saturating_sub(start).min(last);
    f32::from(cell) / f32::from(last)
}

/// Phases of widget maintenance, in different schedules. Apps interleave
/// their own systems against these - a stylist replacing a stock one runs
/// [`after`](IntoScheduleConfigs::after) [`WidgetSystems::Style`].
#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[non_exhaustive]
pub enum WidgetSystems {
    /// `PreUpdate`: cursor repair, scroll extent and row-change forwarding
    /// for the containers drawn from row children, the reveal that keeps a
    /// cursor visible, popup sizing, and popover and menu row placement.
    /// Runs after [`UiSystems::Areas`] and focus dispatch, before
    /// [`UiSystems::Hover`] - so a cursor written later in the frame is
    /// revealed on the next one.
    Layout,
    /// `Update`: the stock stylists rebuilding each widget's [`UiWidget`](plurimus_core::UiWidget).
    Style,
}

/// Installs the widget library: stylists, widget observers, and the
/// layout systems for menus and popovers.
///
/// Requires [`plurimus_core::CorePlugin`] first; adds [`UiPlugin`] itself
/// when absent. Its systems run in the [`WidgetSystems`] phases.
pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<UiPlugin>() {
            app.add_plugins(UiPlugin);
        }
        add_layout_systems(app);
        add_stylists(app);
        add_observers(app);
    }
}

// Placement reads areas and feeds hover; the row-change forwarding also
// has to see what a key handler did, which happens in focus dispatch.
fn add_layout_systems(app: &mut App) {
    app.configure_sets(
        PreUpdate,
        WidgetSystems::Layout
            .after(UiSystems::Areas)
            .after(InputFocusSystems::Dispatch)
            .before(UiSystems::Hover),
    );
    app.add_systems(
        PreUpdate,
        (
            install_editor_views.before(UiSystems::Areas),
            // Before anything reads which camera a widget draws on: the
            // area resolution in Areas, and extraction after the frame.
            adopt_anchor_cameras
                .after(CameraSystems::PropagateCameras)
                .before(UiSystems::Areas),
            (size_menu_popups, place_popovers, place_menu_items)
                .chain()
                .in_set(WidgetSystems::Layout),
            // The scroll extent reads the dirty mark, so a row's edit
            // resizes the content in the frame it happens; the reveal reads
            // the extent, so it comes after both.
            (
                (
                    repair_active_descendants::<ListBox, With<ListItem>>,
                    repair_active_descendants::<Table, TableBodyRow>,
                ),
                (
                    mark_dirty_content::<ListBox, ListItem, ListRowsChanged, ListSelfChanged>,
                    mark_dirty_content::<Table, TableRow, TableRowsChanged, TableSelfChanged>,
                ),
                (
                    sync_row_scroll::<ListBox, ListItem>,
                    sync_row_scroll::<Table, TableRow>,
                ),
                (reveal_listbox_cursor, reveal_table_cursor),
            )
                .chain()
                .in_set(WidgetSystems::Layout),
        ),
    );
}

fn add_stylists(app: &mut App) {
    app.add_systems(
        Update,
        (
            style_buttons,
            style_checkboxes,
            style_radios,
            style_sliders,
            style_scrollbars,
            style_listboxes,
            style_tables,
            style_panes,
            style_text_inputs,
            style_menu_items,
            style_menu_popups,
        )
            .in_set(WidgetSystems::Style),
    );
}

fn add_observers(app: &mut App) {
    app.add_observer(widget_click);
    app.add_observer(widget_key);
    app.add_observer(slider_press);
    app.add_observer(slider_drag);
    app.add_observer(slider_release);
    app.add_observer(slider_key);
    app.add_observer(scrollbar_press);
    app.add_observer(scrollbar_drag);
    app.add_observer(scrollbar_release);
    app.add_observer(listbox_press);
    app.add_observer(listbox_drag);
    app.add_observer(listbox_click);
    app.add_observer(listbox_key);
    app.add_observer(table_click);
    app.add_observer(table_key);
    app.add_observer(text_input_key);
    app.add_observer(text_input_paste);
    app.add_observer(text_input_blur);
    app.add_observer(text_editor_key);
    app.add_observer(text_editor_paste);
    app.add_observer(text_editor_scrolled);
    app.add_observer(menu_button_activate);
    app.add_observer(menu_item_click);
    app.add_observer(menu_key);
    app.add_observer(menu_dismiss);
}
