use gpui::{prelude::*, rems, *};
use std::time::Duration;

use crate::ui::tooltip::Tooltip;

/// Tab close event - emitted by Pane when tab close is requested
#[derive(Clone)]
#[allow(dead_code)] // Event type for future use
pub struct TabCloseRequested(pub SharedString);

/// Tab selection event - emitted by Pane when tab is selected
#[derive(Clone)]
#[allow(dead_code)] // Event type for future use
pub struct TabSelected(pub SharedString);

/// Tab data for rendering (not an Entity - uses RenderOnce)
#[derive(Clone)]
pub struct TabData {
    pub id: SharedString,
    pub title: SharedString,
    pub subtitle: Option<SharedString>,
    pub is_active: bool,
    pub is_loading: bool,
}

impl TabData {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            is_active: false,
            is_loading: false,
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.is_active = active;
        self
    }

    pub fn loading(mut self, loading: bool) -> Self {
        self.is_loading = loading;
        self
    }
}

/// Callback type for tab events
type TabCallback = Box<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>;

/// Renderable tab component
pub struct Tab {
    data: TabData,
    on_select: Option<TabCallback>,
    on_close: Option<TabCallback>,
}

impl Tab {
    pub fn new(data: TabData) -> Self {
        Self {
            data,
            on_select: None,
            on_close: None,
        }
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    pub fn on_close(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Box::new(handler));
        self
    }
}

impl IntoElement for Tab {
    type Element = AnyElement;

    fn into_element(self) -> Self::Element {
        let data = self.data;

        // Colors - consistent with existing theme
        let text_color = if data.is_active {
            rgb(0xe0e0e0)
        } else {
            rgb(0x808080)
        };
        let bg_color = if data.is_active {
            rgb(0x262626)
        } else {
            rgb(0x1e1e1e)
        };
        let hover_bg = rgb(0x252525);
        let close_color = rgb(0x808080);
        let accent_color = rgb(0x0078d4);

        let tab_id = data.id.clone();
        let tab_id_for_close = data.id.clone();
        let is_loading = data.is_loading;
        let is_active = data.is_active;

        let on_select = self.on_select;
        let on_close = self.on_close;

        div()
            .id(SharedString::from(format!("tab-{}", data.id)))
            .flex()
            .flex_row()
            .items_center()
            .flex_none()
            .h(rems(2.0)) // 32px
            .px(rems(0.75)) // 12px
            .gap(rems(0.5)) // 8px
            .bg(bg_color)
            .cursor_pointer()
            .hover(|s| s.bg(hover_bg))
            .when_some(on_select, |el, handler| {
                let tab_id = tab_id.clone();
                el.on_click(move |_, window, cx| {
                    handler(&tab_id, window, cx);
                })
            })
            // Collection icon
            .child(
                svg()
                    .path("icons/collection.svg")
                    .size(rems(0.875)) // 14px
                    .text_color(if is_active { accent_color } else { text_color })
                    .flex_none(),
            )
            // Title and subtitle
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(rems(0.25)) // 4px
                    .overflow_hidden()
                    // Title
                    .child(
                        div()
                            .text_size(rems(0.8125)) // 13px
                            .text_color(text_color)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(data.title.clone()),
                    )
                    // Subtitle (database name in brackets)
                    .when_some(data.subtitle.clone(), |el, subtitle| {
                        el.child(
                            div()
                                .text_size(rems(0.6875)) // 11px
                                .text_color(rgb(0x606060))
                                .whitespace_nowrap()
                                .child(format!("[{}]", subtitle)),
                        )
                    }),
            )
            // Loading spinner or close button
            .child(
                div()
                    .id(SharedString::from(format!("tab-action-{}", data.id)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(rems(1.0)) // 16px
                    .h(rems(1.0)) // 16px
                    .rounded(px(3.0)) // Keep border radius as px
                    .flex_none()
                    .when(is_loading, |el| {
                        // Loading spinner
                        el.child(
                            svg()
                                .path("icons/refresh.svg")
                                .size(rems(0.75)) // 12px
                                .text_color(accent_color)
                                .with_animation(
                                    "tab-loading-spin",
                                    Animation::new(Duration::from_millis(1000)).repeat(),
                                    move |svg_el, delta| {
                                        svg_el.with_transformation(Transformation::rotate(
                                            percentage(delta),
                                        ))
                                    },
                                ),
                        )
                    })
                    .when(!is_loading, |el| {
                        // Close button
                        el.cursor_pointer()
                            .hover(|s| s.bg(rgb(0x3a3a3a)))
                            .when_some(on_close, |el, handler| {
                                el.on_click(move |_event, window, cx| {
                                    handler(&tab_id_for_close, window, cx);
                                })
                            })
                            .tooltip(Tooltip::text("Close tab"))
                            .child(
                                svg()
                                    .path("icons/close.svg")
                                    .size(rems(0.625)) // 10px
                                    .text_color(close_color),
                            )
                    }),
            )
            .into_any_element()
    }
}
