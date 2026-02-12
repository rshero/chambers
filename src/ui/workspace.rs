use gpui::{prelude::*, *};
use std::sync::Arc;

use crate::db::{Connection, ConnectionStorage, DatabaseType};
use crate::ui::connection_modal::ConnectionModal;
use crate::ui::pane::Pane;
use crate::ui::sidebar::{
    AddConnectionRequested, DraggedSidebar, EditConnectionRequested, OpenCollectionRequested,
    Sidebar,
};
use crate::ui::title_bar::TitleBar;

// UI Scaling constants
const DEFAULT_REM_SIZE: f32 = 16.0;
const MIN_REM_SIZE: f32 = 12.0;
const MAX_REM_SIZE: f32 = 24.0;
const REM_SIZE_STEP: f32 = 1.0;

// Define actions for UI scaling
actions!(workspace, [ZoomIn, ZoomOut, ZoomReset]);

/// Register workspace key bindings (zoom in/out/reset)
pub fn register_workspace_bindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-=", ZoomIn, None),
        KeyBinding::new("ctrl-+", ZoomIn, None),
        KeyBinding::new("ctrl--", ZoomOut, None),
        KeyBinding::new("ctrl-0", ZoomReset, None),
    ]);
}

/// Pending collection to open (deferred to render)
struct PendingCollection {
    collection_name: String,
    database_name: String,
    connection_string: String,
}

pub struct ChambersWorkspace {
    focus_handle: FocusHandle,
    title_bar: Entity<TitleBar>,
    sidebar: Entity<Sidebar>,
    pane: Entity<Pane>,
    bounds: Bounds<Pixels>,
    storage: Arc<ConnectionStorage>,
    pending_db_type: Option<DatabaseType>,
    pending_collection: Option<PendingCollection>,
    pending_edit_connection: Option<Connection>,
    needs_initial_focus: bool,
}

impl ChambersWorkspace {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let storage =
            Arc::new(ConnectionStorage::new().expect("Failed to initialize connection storage"));

        let title_bar = cx.new(|_| TitleBar::new());
        let sidebar = cx.new(|cx| Sidebar::new(storage.clone(), cx));
        let pane = cx.new(|_| Pane::new());

        // Subscribe to sidebar events - add connection
        cx.subscribe(
            &sidebar,
            |this, _sidebar, event: &AddConnectionRequested, cx| {
                // Store the db_type and notify - we'll open modal in render
                this.pending_db_type = Some(event.0);
                cx.notify();
            },
        )
        .detach();

        // Subscribe to sidebar events - open collection
        cx.subscribe(
            &sidebar,
            |this, _sidebar, event: &OpenCollectionRequested, cx| {
                // Store the collection info - we'll open in render where we have window
                this.pending_collection = Some(PendingCollection {
                    collection_name: event.collection_name.clone(),
                    database_name: event.database_name.clone(),
                    connection_string: event.connection_string.clone(),
                });
                cx.notify();
            },
        )
        .detach();

        // Subscribe to sidebar events - edit connection (Properties context menu)
        cx.subscribe(
            &sidebar,
            |this, _sidebar, event: &EditConnectionRequested, cx| {
                this.pending_edit_connection = Some(event.0.clone());
                cx.notify();
            },
        )
        .detach();

        Self {
            focus_handle: cx.focus_handle(),
            title_bar,
            sidebar,
            pane,
            bounds: Bounds::default(),
            storage,
            pending_db_type: None,
            pending_collection: None,
            pending_edit_connection: None,
            needs_initial_focus: true,
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

    fn open_edit_connection_window(
        connection: Connection,
        storage: Arc<ConnectionStorage>,
        cx: &mut App,
    ) {
        let bounds = Bounds::centered(None, size(px(900.0), px(600.0)), cx);
        let conn_id = connection.id.clone();
        let db_type = connection.db_type;

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_decorations: Some(WindowDecorations::Client),
                titlebar: Some(TitlebarOptions {
                    title: Some(SharedString::from("Edit Connection")),
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
            |_, cx| {
                cx.new(|cx| {
                    let mut modal = ConnectionModal::new(db_type, storage, cx);
                    modal.select_connection_by_id(&conn_id, cx);
                    modal
                })
            },
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

impl Focusable for ChambersWorkspace {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ChambersWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Focus the workspace on first render
        if self.needs_initial_focus {
            self.needs_initial_focus = false;
            cx.defer_in(window, |this, window, _cx| {
                window.focus(&this.focus_handle);
            });
        }

        // Handle pending modal open - open in new window
        if let Some(db_type) = self.pending_db_type.take() {
            let storage = self.storage.clone();
            cx.defer(move |cx| {
                Self::open_connection_window(db_type, storage, cx);
            });
        }

        // Handle pending edit connection - open modal with connection pre-selected
        if let Some(connection) = self.pending_edit_connection.take() {
            let storage = self.storage.clone();
            cx.defer(move |cx| {
                Self::open_edit_connection_window(connection, storage, cx);
            });
        }

        // Handle pending collection open - we now have window access
        if let Some(pending) = self.pending_collection.take() {
            self.pane.update(cx, |pane, cx| {
                pane.open_collection(
                    pending.collection_name,
                    pending.database_name,
                    pending.connection_string,
                    window,
                    cx,
                );
            });
        }

        // Refresh sidebar connections when window is active (catches modal close)
        if window.is_window_active() {
            self.sidebar.update(cx, |sidebar, cx| {
                sidebar.refresh_connections(cx);
            });
        }

        let this = cx.entity();

        div()
            .id("workspace")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1a1a1a))
            .font_family("Fira Code")
            // Key context for zoom actions
            .key_context("Workspace")
            // Zoom actions
            .on_action(cx.listener(|_this, _: &ZoomIn, window, cx| {
                let current: f32 = window.rem_size().into();
                let new_size = (current + REM_SIZE_STEP).min(MAX_REM_SIZE);
                window.set_rem_size(px(new_size));
                // Also update gpui-component theme font_size for consistency
                gpui_component::theme::Theme::global_mut(cx).font_size = px(new_size);
                cx.refresh_windows();
            }))
            .on_action(cx.listener(|_this, _: &ZoomOut, window, cx| {
                let current: f32 = window.rem_size().into();
                let new_size = (current - REM_SIZE_STEP).max(MIN_REM_SIZE);
                window.set_rem_size(px(new_size));
                // Also update gpui-component theme font_size for consistency
                gpui_component::theme::Theme::global_mut(cx).font_size = px(new_size);
                cx.refresh_windows();
            }))
            .on_action(cx.listener(|_this, _: &ZoomReset, window, cx| {
                window.set_rem_size(px(DEFAULT_REM_SIZE));
                // Also update gpui-component theme font_size for consistency
                gpui_component::theme::Theme::global_mut(cx).font_size = px(DEFAULT_REM_SIZE);
                cx.refresh_windows();
            }))
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
                    .min_w_0() // Allow shrinking below content width
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
                    // Pane (tabs + content) on the right
                    .child(
                        div()
                            .id("editor-area")
                            .flex_1()
                            .min_w_0() // Allow shrinking below content width
                            .h_full()
                            .child(self.pane.clone()),
                    ),
            )
    }
}
