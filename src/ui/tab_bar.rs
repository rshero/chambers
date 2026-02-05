use gpui::{prelude::*, *};

use crate::ui::tab::{Tab, TabData};

/// Horizontal tab bar container
/// Holds multiple tabs and handles horizontal scrolling
pub struct TabBar {
    scroll_handle: ScrollHandle,
}

impl TabBar {
    pub fn new() -> Self {
        Self {
            scroll_handle: ScrollHandle::new(),
        }
    }
}

impl Default for TabBar {
    fn default() -> Self {
        Self::new()
    }
}

/// TabBar is rendered by the Pane, not as its own entity
/// This provides the render function to be used by Pane
impl TabBar {
    pub fn render_bar<F, G>(
        &self,
        tabs: Vec<TabData>,
        on_select: F,
        on_close: G,
    ) -> impl IntoElement
    where
        F: Fn(&SharedString, &mut Window, &mut App) + Clone + 'static,
        G: Fn(&SharedString, &mut Window, &mut App) + Clone + 'static,
    {
        let bg_color = rgb(0x1e1e1e);
        let border_color = rgb(0x2a2a2a);

        div()
            .id("tab-bar")
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(32.0))
            .bg(bg_color)
            .border_b_1()
            .border_color(border_color)
            .overflow_x_scroll()
            .track_scroll(&self.scroll_handle)
            .children(tabs.into_iter().map(|tab_data| {
                let on_select = on_select.clone();
                let on_close = on_close.clone();
                Tab::new(tab_data).on_select(on_select).on_close(on_close)
            }))
            // Empty space fills the rest when fewer tabs
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .h_full()
                    .bg(bg_color)
                    .border_b_1()
                    .border_color(border_color),
            )
    }
}
