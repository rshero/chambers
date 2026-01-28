use gpui::{prelude::*, *};
use std::sync::Arc;

use crate::db::{ConnectionStorage, DatabaseType};
use crate::ui::connection_modal::ConnectionModal;
use crate::ui::sidebar::{AddConnectionRequested, DraggedSidebar, Sidebar};
use crate::ui::title_bar::TitleBar;

pub struct ChambersWorkspace {
    title_bar: Entity<TitleBar>,
    sidebar: Entity<Sidebar>,
    bounds: Bounds<Pixels>,
    storage: Arc<ConnectionStorage>,
    pending_db_type: Option<DatabaseType>,
}

impl ChambersWorkspace {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let storage =
            Arc::new(ConnectionStorage::new().expect("Failed to initialize connection storage"));

        let title_bar = cx.new(|_| TitleBar::new());
        let sidebar = cx.new(|_| Sidebar::new());

        // Subscribe to sidebar events
        cx.subscribe(
            &sidebar,
            |this, _sidebar, event: &AddConnectionRequested, cx| {
                // Store the db_type and notify - we'll open modal in render
                this.pending_db_type = Some(event.0);
                cx.notify();
            },
        )
        .detach();

        Self {
            title_bar,
            sidebar,
            bounds: Bounds::default(),
            storage,
            pending_db_type: None,
        }
    }

    fn open_connection_window(
        db_type: DatabaseType,
        storage: Arc<ConnectionStorage>,
        cx: &mut App,
    ) {
        let bounds = Bounds::centered(None, size(px(900.0), px(600.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_decorations: Some(WindowDecorations::Client),
                titlebar: Some(TitlebarOptions {
                    title: Some(SharedString::from("New Connection")),
                    appears_transparent: true,
                    ..Default::default()
                }),
                focus: true,
                show: true,
                is_movable: true,
                is_resizable: true,
                window_min_size: Some(size(px(600.0), px(400.0))),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| ConnectionModal::new(db_type, storage, cx)),
        )
        .ok();
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
        // Handle pending modal open - open in new window
        if let Some(db_type) = self.pending_db_type.take() {
            let storage = self.storage.clone();
            cx.defer(move |cx| {
                Self::open_connection_window(db_type, storage, cx);
            });
        }

        let this = cx.entity();

        div()
            .id("workspace")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1a1a1a))
            .font_family("Fira Code")
            // Handle sidebar drag-to-resize
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<DraggedSidebar>, _, cx| {
                    this.resize_sidebar(event.event.position.x, cx);
                }),
            )
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
                    .child(div().id("editor-area").flex_1().h_full().bg(rgb(0x1a1a1a))),
            )
    }
}
