use gpui::{prelude::*, *};

use crate::ui::title_bar::TitleBar;

pub struct ChambersWorkspace {
    title_bar: Entity<TitleBar>,
}

impl ChambersWorkspace {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let title_bar = cx.new(|_| TitleBar::new());
        Self { title_bar }
    }
}

impl Render for ChambersWorkspace {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e1e))
            // Title bar at top
            .child(self.title_bar.clone())
            // Main content area
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .bg(rgb(0x1e1e1e)),
            )
    }
}
