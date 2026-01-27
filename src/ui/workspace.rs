use gpui::{prelude::*, *};

use crate::ui::sidebar::{DraggedSidebar, Sidebar};
use crate::ui::title_bar::TitleBar;

pub struct ChambersWorkspace {
    title_bar: Entity<TitleBar>,
    sidebar: Entity<Sidebar>,
    bounds: Bounds<Pixels>,
}

impl ChambersWorkspace {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let title_bar = cx.new(|_| TitleBar::new());
        let sidebar = cx.new(|_| Sidebar::new());
        Self {
            title_bar,
            sidebar,
            bounds: Bounds::default(),
        }
    }

    fn resize_sidebar(&mut self, position_x: Pixels, cx: &mut Context<Self>) {
        // Calculate new width based on mouse position relative to workspace left edge
        let new_width = position_x - self.bounds.origin.x;
        self.sidebar.update(cx, |sidebar, cx| {
            sidebar.resize(new_width, cx);
        });
    }
}

impl Render for ChambersWorkspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();

        div()
            .id("workspace")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e1e))
            // Handle sidebar drag-to-resize
            .on_drag_move(cx.listener(
                |this, event: &DragMoveEvent<DraggedSidebar>, _, cx| {
                    this.resize_sidebar(event.event.position.x, cx);
                },
            ))
            // Title bar at top
            .child(self.title_bar.clone())
            // Main content area with sidebar
            .child(
                div()
                    .id("main-content")
                    .flex_1()
                    .w_full()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    // Canvas to track bounds
                    .child(canvas(
                        move |bounds, _, cx| {
                            this.update(cx, |this, _cx| {
                                this.bounds = bounds;
                            });
                        },
                        |_, _, _, _| {},
                    ))
                    // Sidebar on the left
                    .child(self.sidebar.clone())
                    // Editor/content area
                    .child(
                        div()
                            .id("editor-area")
                            .flex_1()
                            .h_full()
                            .bg(rgb(0x1e1e1e)),
                    ),
            )
    }
}
