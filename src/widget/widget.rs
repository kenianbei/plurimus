use crate::draw::DrawFn;
use crate::widget::area::WidgetRect;
use crate::widget::order::WidgetOrder;
use bevy::prelude::{BevyError, Component, Result};
use std::any::{Any, TypeId};
use std::sync::Arc;

/// Component that implements `DrawFn` and renders using `WidgetData` stored state.
///
/// Example usage:
/// ```rust
/// use bevy::prelude::*;
/// use plurimus::Widget;
/// use ratatui::widgets::{List, ListState, Paragraph};
///
/// fn startup(mut commands: Commands) {
///     commands.spawn((
///         Widget::from_widget(Paragraph::new("Hello, world!")),
///     ));
///
///     commands.spawn((
///        Widget::from_stateful(List::new(vec!["Item 1", "Item 2", "Item 3"]), ListState::default()),
///    ));
///
///    commands.spawn((
///       Widget::from_render_fn(|frame, area| {
///         frame.render_widget(Paragraph::new("Rendered from a closure!"), area);
///         Ok(())
///      }),
///   ));
/// }
/// ```
#[derive(Component)]
#[require(WidgetOrder, WidgetRect)]
pub struct Widget {
    enabled: bool,
    widget: WidgetData,
}

impl Clone for Widget {
    fn clone(&self) -> Self {
        Self {
            enabled: self.enabled,
            widget: self.widget.clone(),
        }
    }
}

pub enum WidgetData {
    Widget(Arc<dyn DynWidgetRef>),
    Stateful(Arc<dyn DynStatefulWidgetRef>, Option<ErasedState>),
    RenderFn(Arc<RenderFn>, Option<ErasedState>),
}

impl Clone for WidgetData {
    fn clone(&self) -> Self {
        match self {
            WidgetData::Widget(w) => WidgetData::Widget(Arc::clone(w)),
            WidgetData::Stateful(w, s) => WidgetData::Stateful(Arc::clone(w), s.clone()),
            WidgetData::RenderFn(f, s) => WidgetData::RenderFn(Arc::clone(f), s.clone()),
        }
    }
}

type RenderFn = dyn Fn(
        &mut ratatui::prelude::Frame,
        ratatui::prelude::Rect,
        Option<&mut (dyn Any + Send + Sync)>,
    ) -> Result
    + Send
    + Sync;

impl Widget {
    pub fn from_widget<W>(widget: W) -> Self
    where
        W: Send + Sync + 'static,
        for<'a> &'a W: ratatui::widgets::WidgetRef,
    {
        Self {
            enabled: true,
            widget: WidgetData::Widget(Arc::new(widget)),
        }
    }

    pub fn from_stateful<W, S>(widget: W, state: S) -> Self
    where
        W: Send + Sync + 'static,
        S: Any + Clone + Send + Sync + 'static,
        for<'a> &'a W: ratatui::widgets::StatefulWidgetRef<State = S>,
    {
        Self {
            enabled: true,
            widget: WidgetData::Stateful(
                Arc::new(DynStatefulWidgetRefImpl::<W, S> {
                    widget,
                    _marker: std::marker::PhantomData,
                }),
                Some(ErasedState::new(state)),
            ),
        }
    }

    pub fn from_render_fn<F>(render_fn: F) -> Self
    where
        F: Fn(&mut ratatui::prelude::Frame, ratatui::prelude::Rect) -> Result
            + Send
            + Sync
            + 'static,
    {
        Self {
            enabled: true,
            widget: WidgetData::RenderFn(Self::make_render_fn(render_fn), None),
        }
    }

    pub fn from_render_fn_with_state<F, S>(render_fn: F, state: S) -> Self
    where
        F: Fn(&mut ratatui::prelude::Frame, ratatui::prelude::Rect, &mut S) -> Result
            + Send
            + Sync
            + 'static,
        S: Any + Clone + Send + Sync + 'static,
    {
        Self {
            enabled: true,
            widget: WidgetData::RenderFn(
                Self::make_typed_render_fn::<F, S>(render_fn),
                Some(ErasedState::new(state)),
            ),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn get_state<S>(&self) -> Result<&S>
    where
        S: Any + Send + Sync + 'static,
    {
        let any = self.get_state_any().ok_or_else(|| {
            BevyError::from("Widget::get_state: no state found (non-stateful widget)")
        })?;

        any.downcast_ref::<S>().ok_or_else(|| {
            BevyError::from(format!(
                "Widget::get_state: state type mismatch, expected {}",
                std::any::type_name::<S>()
            ))
        })
    }

    pub fn get_state_mut<S>(&mut self) -> Result<&mut S>
    where
        S: Any + Send + Sync + 'static,
    {
        let any = self.get_state_any_mut().ok_or_else(|| {
            BevyError::from("Widget::get_state_mut: no state found (non-stateful widget)")
        })?;

        any.downcast_mut::<S>().ok_or_else(|| {
            BevyError::from(format!(
                "Widget::get_state_mut: state type mismatch, expected {}",
                std::any::type_name::<S>()
            ))
        })
    }

    pub fn set_state<S>(&mut self, state: S) -> Result
    where
        S: Any + Clone + Send + Sync + 'static,
    {
        match &mut self.widget {
            WidgetData::Stateful(_, s_opt) => {
                *s_opt = Some(ErasedState::new(state));
                Ok(())
            }
            WidgetData::RenderFn(_, s_opt) => {
                *s_opt = Some(ErasedState::new(state));
                Ok(())
            }
            WidgetData::Widget(_) => Err(BevyError::from(
                "Widget::set_state: cannot set state on a non-stateful widget",
            )),
        }
    }

    pub fn widget<W>(&self) -> Result<&W>
    where
        W: Any + Send + Sync + 'static,
    {
        match &self.widget {
            WidgetData::Widget(w_arc) => w_arc.as_any().downcast_ref::<W>().ok_or_else(|| {
                BevyError::from(format!(
                    "Widget::widget_ref: type mismatch, expected {}",
                    std::any::type_name::<W>()
                ))
            }),
            _ => Err(BevyError::from(
                "Widget::widget_ref: not a Widget variant (expected WidgetData::Widget)",
            )),
        }
    }

    pub fn widget_mut<W>(&mut self) -> Result<&mut W>
    where
        W: Any + Send + Sync + 'static,
    {
        match &mut self.widget {
            WidgetData::Widget(w_arc) => {
                let w_dyn = Arc::get_mut(w_arc).ok_or_else(|| {
                    BevyError::from("Widget::widget_mut: widget is shared (Arc strong_count > 1); cannot get &mut")
                })?;

                w_dyn.as_any_mut().downcast_mut::<W>().ok_or_else(|| {
                    BevyError::from(format!(
                        "Widget::widget_mut: type mismatch, expected {}",
                        std::any::type_name::<W>()
                    ))
                })
            }
            _ => Err(BevyError::from(
                "Widget::widget_mut: not a Widget variant (expected WidgetData::Widget)",
            )),
        }
    }

    pub fn stateful_widget_ref<W, S>(&self) -> Result<&W>
    where
        W: Any + Send + Sync + 'static,
        S: Any + Send + Sync + 'static,
    {
        match &self.widget {
            WidgetData::Stateful(w_arc, s_opt) => {
                let s = s_opt.as_ref().ok_or_else(|| {
                    BevyError::from("Widget::stateful_widget_ref: missing state (expected Some)")
                })?;

                if !s.is::<S>() {
                    return Err(BevyError::from(format!(
                        "Widget::stateful_widget_ref: state type mismatch, expected {}",
                        std::any::type_name::<S>()
                    )));
                }

                let wrapper = w_arc
                    .as_any()
                    .downcast_ref::<DynStatefulWidgetRefImpl<W, S>>()
                    .ok_or_else(|| {
                        BevyError::from(format!(
                            "Widget::stateful_widget_ref: widget type mismatch, expected {}",
                            std::any::type_name::<W>()
                        ))
                    })?;

                Ok(&wrapper.widget)
            }
            _ => Err(BevyError::from(
                "Widget::stateful_widget_ref: not a Stateful variant (expected WidgetData::Stateful)",
            )),
        }
    }

    pub fn stateful_widget_mut<W, S>(&mut self) -> Result<&mut W>
    where
        W: Any + Send + Sync + 'static,
        S: Any + Send + Sync + 'static,
    {
        match &mut self.widget {
            WidgetData::Stateful(w_arc, s_opt) => {
                let _s = s_opt.as_ref().ok_or_else(|| {
                    BevyError::from("Widget::stateful_widget_mut: missing state (expected Some)")
                })?;

                if !_s.is::<S>() {
                    return Err(BevyError::from(format!(
                        "Widget::stateful_widget_mut: state type mismatch, expected {}",
                        std::any::type_name::<S>(),
                    )));
                }

                let w_dyn = Arc::get_mut(w_arc).ok_or_else(|| {
                    BevyError::from("Widget::stateful_widget_mut: widget is shared (Arc strong_count > 1); cannot get &mut")
                })?;

                let wrapper = w_dyn
                    .as_any_mut()
                    .downcast_mut::<DynStatefulWidgetRefImpl<W, S>>()
                    .ok_or_else(|| {
                        BevyError::from(format!(
                            "Widget::stateful_widget_mut: widget type mismatch, expected {}",
                            std::any::type_name::<W>()
                        ))
                    })?;

                Ok(&mut wrapper.widget)
            }
            _ => Err(BevyError::from(
                "Widget::stateful_widget_mut: not a Stateful variant (expected WidgetData::Stateful)",
            )),
        }
    }

    pub fn set_widget<W>(&mut self, widget: W)
    where
        W: Send + Sync + 'static,
        for<'a> &'a W: ratatui::widgets::WidgetRef,
    {
        self.widget = WidgetData::Widget(Arc::new(widget));
    }

    pub fn set_stateful<W, S>(&mut self, widget: W) -> Result
    where
        W: Send + Sync + 'static,
        S: Any + Send + Sync + 'static,
        for<'a> &'a W: ratatui::widgets::StatefulWidgetRef<State = S>,
    {
        let state = self.take_any_state().ok_or_else(|| {
            BevyError::from(
                "Widget::set_stateful: no existing state to reuse; call set_stateful_with_state",
            )
        })?;

        if !state.is::<S>() {
            return Err(BevyError::from(format!(
                "Widget::set_stateful: state type mismatch, expected {}",
                std::any::type_name::<S>(),
            )));
        }

        self.widget = WidgetData::Stateful(
            Arc::new(DynStatefulWidgetRefImpl::<W, S> {
                widget,
                _marker: std::marker::PhantomData,
            }),
            Some(state),
        );

        Ok(())
    }

    pub fn set_stateful_with_state<W, S>(&mut self, widget: W, state: S)
    where
        W: Send + Sync + 'static,
        S: Any + Clone + Send + Sync + 'static,
        for<'a> &'a W: ratatui::widgets::StatefulWidgetRef<State = S>,
    {
        self.widget = WidgetData::Stateful(
            Arc::new(DynStatefulWidgetRefImpl::<W, S> {
                widget,
                _marker: std::marker::PhantomData,
            }),
            Some(ErasedState::new(state)),
        );
    }

    pub fn set_render_fn<F>(&mut self, render_fn: F)
    where
        F: Fn(&mut ratatui::prelude::Frame, ratatui::prelude::Rect) -> Result
            + Send
            + Sync
            + 'static,
    {
        let state = self.take_any_state();
        self.widget = WidgetData::RenderFn(Self::make_render_fn(render_fn), state);
    }

    pub fn set_render_fn_typed<F, S>(&mut self, render_fn: F)
    where
        F: Fn(&mut ratatui::prelude::Frame, ratatui::prelude::Rect, &mut S) -> Result
            + Send
            + Sync
            + 'static,
        S: Any + Send + Sync + 'static,
    {
        let state = self.take_any_state();
        let state = match state {
            Some(s) if s.is::<S>() => Some(s),
            _ => None,
        };

        self.widget = WidgetData::RenderFn(Self::make_typed_render_fn::<F, S>(render_fn), state);
    }

    pub fn set_render_fn_with_state<F, S>(&mut self, render_fn: F, state: S)
    where
        F: Fn(&mut ratatui::prelude::Frame, ratatui::prelude::Rect, &mut S) -> Result
            + Send
            + Sync
            + 'static,
        S: Any + Clone + Send + Sync + 'static,
    {
        self.widget = WidgetData::RenderFn(
            Self::make_typed_render_fn::<F, S>(render_fn),
            Some(ErasedState::new(state)),
        );
    }

    fn make_render_fn<F>(render_fn: F) -> Arc<RenderFn>
    where
        F: Fn(&mut ratatui::prelude::Frame, ratatui::prelude::Rect) -> Result
            + Send
            + Sync
            + 'static,
    {
        Arc::new(move |frame, area, _state| render_fn(frame, area))
    }

    fn make_typed_render_fn<F, S>(render_fn: F) -> Arc<RenderFn>
    where
        F: Fn(&mut ratatui::prelude::Frame, ratatui::prelude::Rect, &mut S) -> Result
            + Send
            + Sync
            + 'static,
        S: Any + Send + Sync + 'static,
    {
        let expected = std::any::type_name::<S>();

        Arc::new(move |frame, area, state_any| {
            let Some(state_any) = state_any else {
                return Err(BevyError::from(
                    "Widget::RenderFn: missing state (expected state, found None)",
                ));
            };

            let s = state_any.downcast_mut::<S>().ok_or_else(|| {
                BevyError::from(format!(
                    "Widget::RenderFn: state type mismatch, expected {expected}"
                ))
            })?;

            render_fn(frame, area, s)
        })
    }

    fn take_any_state(&mut self) -> Option<ErasedState> {
        match &mut self.widget {
            WidgetData::Stateful(_, s_opt) => s_opt.take(),
            WidgetData::RenderFn(_, s_opt) => s_opt.take(),
            WidgetData::Widget(_) => None,
        }
    }

    fn get_state_any(&self) -> Option<&(dyn Any + Send + Sync)> {
        match &self.widget {
            WidgetData::Stateful(_, Some(s)) => Some(s.as_any()),
            WidgetData::RenderFn(_, Some(s)) => Some(s.as_any()),
            _ => None,
        }
    }

    fn get_state_any_mut(&mut self) -> Option<&mut (dyn Any + Send + Sync)> {
        match &mut self.widget {
            WidgetData::Stateful(_, Some(s)) => Some(s.as_any_mut()),
            WidgetData::RenderFn(_, Some(s)) => Some(s.as_any_mut()),
            _ => None,
        }
    }
}

pub trait CloneAny: Any + Send + Sync + 'static {
    fn clone_box(&self) -> Box<dyn CloneAny>;
}

impl<T> CloneAny for T
where
    T: Any + Send + Sync + Clone,
{
    fn clone_box(&self) -> Box<dyn CloneAny> {
        Box::new(self.clone())
    }
}

pub struct ErasedState {
    type_id: TypeId,
    value: Box<dyn CloneAny>,
}

impl Clone for ErasedState {
    fn clone(&self) -> Self {
        Self {
            type_id: self.type_id,
            value: self.value.clone_box(),
        }
    }
}

impl ErasedState {
    pub fn new<S>(s: S) -> Self
    where
        S: Any + Send + Sync + Clone + 'static,
    {
        Self {
            type_id: TypeId::of::<S>(),
            value: Box::new(s),
        }
    }

    #[inline]
    pub fn is<S>(&self) -> bool
    where
        S: Any + 'static,
    {
        self.type_id == TypeId::of::<S>()
    }

    #[inline]
    pub fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self.value.as_ref()
    }

    #[inline]
    pub fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) {
        self.value.as_mut()
    }
}

pub trait DynWidgetRef: Send + Sync + 'static {
    fn render(&self, frame: &mut ratatui::prelude::Frame, area: ratatui::prelude::Rect) -> Result;
    fn as_any(&self) -> &(dyn Any + Send + Sync);
    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync);
}

impl<W> DynWidgetRef for W
where
    W: Send + Sync + 'static,
    for<'a> &'a W: ratatui::widgets::WidgetRef,
{
    fn render(&self, frame: &mut ratatui::prelude::Frame, area: ratatui::prelude::Rect) -> Result {
        use ratatui::widgets::FrameExt;
        frame.render_widget_ref(self, area);
        Ok(())
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self
    }

    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) {
        self
    }
}

pub trait DynStatefulWidgetRef: Send + Sync + 'static {
    fn render(
        &self,
        frame: &mut ratatui::prelude::Frame,
        area: ratatui::prelude::Rect,
        state: &mut (dyn Any + Send + Sync),
    ) -> Result;
    fn as_any(&self) -> &(dyn Any + Send + Sync);
    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync);
}

pub struct DynStatefulWidgetRefImpl<W, S> {
    widget: W,
    _marker: std::marker::PhantomData<S>,
}

impl<W, S> DynStatefulWidgetRef for DynStatefulWidgetRefImpl<W, S>
where
    W: Send + Sync + 'static,
    S: Any + Send + Sync + 'static,
    for<'a> &'a W: ratatui::widgets::StatefulWidgetRef<State = S>,
{
    fn render(
        &self,
        frame: &mut ratatui::prelude::Frame,
        area: ratatui::prelude::Rect,
        state: &mut (dyn Any + Send + Sync),
    ) -> Result {
        use ratatui::widgets::FrameExt;

        let expected = std::any::type_name::<S>();
        let s = state.downcast_mut::<S>().ok_or_else(|| {
            BevyError::from(format!(
                "Widget::Stateful: state type mismatch, expected {expected}"
            ))
        })?;

        frame.render_stateful_widget_ref(&self.widget, area, s);
        Ok(())
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self
    }

    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) {
        self
    }
}

impl DrawFn for Widget {
    fn draw(
        &mut self,
        frame: &mut ratatui::prelude::Frame,
        area: ratatui::prelude::Rect,
    ) -> Result {
        if !self.is_enabled() {
            return Ok(());
        }

        match &mut self.widget {
            WidgetData::Widget(widget) => widget.render(frame, area),
            WidgetData::Stateful(widget, Some(state)) => {
                widget.render(frame, area, state.as_any_mut())
            }
            WidgetData::Stateful(_, None) => Err(BevyError::from(
                "Widget::draw: Stateful widget has no state (expected Some)",
            )),
            WidgetData::RenderFn(render_fn, state_opt) => {
                if let Some(state) = state_opt.as_mut() {
                    render_fn(frame, area, Some(state.as_any_mut()))
                } else {
                    render_fn(frame, area, None)
                }
            }
        }
    }
}
