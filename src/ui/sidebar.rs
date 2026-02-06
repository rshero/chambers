use gpui::{prelude::*, *};
use std::sync::Arc;

use crate::db::{Connection, ConnectionStorage, DatabaseType};
use crate::ui::connection_browser::{
    CollectionContextMenuRequested, CollectionSelected, ConnectionBrowser,
    DatabaseContextMenuRequested, LoadingState,
};
use crate::ui::context_menu::{
    ContextMenu, ContextMenuAction, ContextMenuEvent, ContextMenuItem, ContextMenuItemStyle,
};
use crate::ui::database_menu::{DatabaseMenu, DatabaseSelected};
use crate::ui::database_picker::{DatabasePicker, DatabaseVisibilityChanged};
use crate::ui::filter_menu::{FilterChanged, FilterMenu};
use crate::ui::tooltip::Tooltip;

/// Size of the resize handle in pixels
const RESIZE_HANDLE_SIZE: f32 = 6.0;

/// Default sidebar width
const DEFAULT_SIDEBAR_WIDTH: f32 = 240.0;

/// Minimum sidebar width
const MIN_SIDEBAR_WIDTH: f32 = 150.0;

/// Maximum sidebar width
const MAX_SIDEBAR_WIDTH: f32 = 600.0;

/// Event emitted when user wants to add a new connection
#[derive(Clone)]
pub struct AddConnectionRequested(pub DatabaseType);

impl EventEmitter<AddConnectionRequested> for Sidebar {}

/// Event emitted when user wants to edit an existing connection
#[derive(Clone)]
pub struct EditConnectionRequested(pub Connection);

impl EventEmitter<EditConnectionRequested> for Sidebar {}

/// Event emitted when a collection is selected (database_name, collection_name, connection_string)
#[derive(Clone)]
pub struct OpenCollectionRequested {
    pub database_name: String,
    pub collection_name: String,
    pub connection_string: String,
}

impl EventEmitter<OpenCollectionRequested> for Sidebar {}

/// Drag payload for sidebar resize
#[derive(Clone)]
pub struct DraggedSidebar;

impl Render for DraggedSidebar {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// Identifies what the context menu was opened on
#[derive(Clone)]
#[allow(dead_code)] // Fields used for future context-aware behavior
enum ContextMenuTarget {
    /// Right-clicked on a saved connection row
    Connection(Connection),
    /// Right-clicked on a database row (connection_id, database_name)
    Database(String, String),
    /// Right-clicked on a collection row (connection_id, database_name, collection_name)
    Collection(String, String, String),
}

/// The resizable sidebar component
pub struct Sidebar {
    width: Pixels,
    database_menu: Option<Entity<DatabaseMenu>>,
    filter_menu: Option<Entity<FilterMenu>>,
    database_picker: Option<Entity<DatabasePicker>>,
    database_picker_connection_id: Option<String>,
    /// Timestamp of last picker dismiss (for toggle detection)
    last_picker_dismiss: Option<std::time::Instant>,
    /// Timestamp of last database menu dismiss (for toggle detection)
    last_db_menu_dismiss: Option<std::time::Instant>,
    /// Timestamp of last filter menu dismiss (for toggle detection)
    last_filter_menu_dismiss: Option<std::time::Instant>,
    storage: Arc<ConnectionStorage>,
    connections: Vec<Connection>,
    expanded_connections: std::collections::HashSet<String>,
    connection_browsers: std::collections::HashMap<String, Entity<ConnectionBrowser>>,
    /// Whether we are currently refreshing connections
    is_refreshing: bool,
    /// Filter by database types (empty = show all)
    type_filter: std::collections::HashSet<DatabaseType>,
    /// Context menu state
    context_menu: Option<Entity<ContextMenu>>,
    context_menu_target: Option<ContextMenuTarget>,
    /// Pending database context menu request (conn_id, db_name, position) — processed in render
    pending_db_context_menu: Option<(String, String, Point<Pixels>)>,
    /// Pending collection context menu request (conn_id, db_name, coll_name, position) — processed in render
    pending_coll_context_menu: Option<(String, String, String, Point<Pixels>)>,
}

impl Sidebar {
    pub fn new(storage: Arc<ConnectionStorage>, cx: &mut Context<Self>) -> Self {
        let connections = storage.get_all().unwrap_or_default();

        // Identify connections with saved database preferences to auto-expand
        let mut expanded_connections = std::collections::HashSet::new();
        let mut connection_browsers = std::collections::HashMap::new();

        for conn in &connections {
            let has_saved_dbs = conn
                .visible_databases
                .as_ref()
                .map(|dbs| !dbs.is_empty())
                .unwrap_or(false);
            let has_show_all = conn.show_all_databases.unwrap_or(false);

            if (has_saved_dbs || has_show_all) && conn.db_type == DatabaseType::MongoDB {
                // Auto-expand this connection
                expanded_connections.insert(conn.id.clone());

                // Create browser with saved databases as preview (no connection)
                let browser = cx.new(|_cx| ConnectionBrowser::new(conn.clone()));
                let connection_string = conn.get_connection_string();

                // Subscribe to collection selection events
                cx.subscribe(&browser, {
                    let connection_string = connection_string.clone();
                    move |_this, _, event: &CollectionSelected, cx| {
                        println!("Collection selected: {}.{}", event.0, event.1);
                        cx.emit(OpenCollectionRequested {
                            database_name: event.0.clone(),
                            collection_name: event.1.clone(),
                            connection_string: connection_string.clone(),
                        });
                        cx.notify();
                    }
                })
                .detach();

                // Subscribe to database context menu events
                cx.subscribe(&browser, |this, _, event: &DatabaseContextMenuRequested, cx| {
                    // We need window access but subscribe doesn't provide it
                    // Store pending context menu request
                    this.pending_db_context_menu = Some((event.0.clone(), event.1.clone(), event.2));
                    cx.notify();
                })
                .detach();

                // Subscribe to collection context menu events
                cx.subscribe(&browser, |this, _, event: &CollectionContextMenuRequested, cx| {
                    this.pending_coll_context_menu = Some((event.0.clone(), event.1.clone(), event.2.clone(), event.3));
                    cx.notify();
                })
                .detach();

                // Set initial visible databases and populate preview
                let saved_visible_dbs = conn.visible_databases.clone();
                let saved_show_all = conn.show_all_databases.unwrap_or(false);

                browser.update(cx, |browser, _cx| {
                    browser.set_initial_visible_databases(saved_visible_dbs.clone(), saved_show_all);
                    // Populate database list from saved names as a static preview
                    if let Some(ref dbs) = saved_visible_dbs {
                        if !dbs.is_empty() {
                            browser.set_saved_databases_preview(dbs.clone());
                        }
                    }
                });

                connection_browsers.insert(conn.id.clone(), browser);
            }
        }

        Self {
            width: px(DEFAULT_SIDEBAR_WIDTH),
            database_menu: None,
            filter_menu: None,
            database_picker: None,
            database_picker_connection_id: None,
            last_picker_dismiss: None,
            last_db_menu_dismiss: None,
            last_filter_menu_dismiss: None,
            storage,
            connections,
            expanded_connections,
            connection_browsers,
            is_refreshing: false,
            type_filter: std::collections::HashSet::new(),
            context_menu: None,
            context_menu_target: None,
            pending_db_context_menu: None,
            pending_coll_context_menu: None,
        }
    }

    pub fn refresh_connections(&mut self, _cx: &mut Context<Self>) {
        self.connections = self.storage.get_all().unwrap_or_default();
    }

    pub fn set_width(&mut self, width: Pixels) {
        self.width = width.max(px(MIN_SIDEBAR_WIDTH)).min(px(MAX_SIDEBAR_WIDTH));
    }

    pub fn resize(&mut self, new_width: Pixels, cx: &mut Context<Self>) {
        self.set_width(new_width);
        cx.notify();
    }

    fn show_database_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Toggle behavior - if already open, close it
        if self.database_menu.is_some() {
            self.database_menu = None;
            self.last_db_menu_dismiss = None;
            cx.notify();
            return;
        }

        // Check if we just dismissed the menu (within 100ms)
        // This handles the case where on_mouse_down_out fires before our click handler
        if let Some(dismiss_time) = self.last_db_menu_dismiss {
            if dismiss_time.elapsed() < std::time::Duration::from_millis(100) {
                self.last_db_menu_dismiss = None;
                cx.notify();
                return;
            }
        }
        self.last_db_menu_dismiss = None;

        let menu = cx.new(DatabaseMenu::new);

        // Subscribe to menu events
        cx.subscribe_in(&menu, window, |this, _, event: &DatabaseSelected, _, cx| {
            cx.emit(AddConnectionRequested(event.0));
            this.database_menu = None;
            cx.notify();
        })
        .detach();

        cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, _, cx| {
            this.database_menu = None;
            this.last_db_menu_dismiss = Some(std::time::Instant::now());
            cx.notify();
        })
        .detach();

        // Focus the menu
        menu.focus_handle(cx).focus(window);

        self.database_menu = Some(menu);
        cx.notify();
    }

    fn show_filter_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Toggle behavior - if already open, close it
        if self.filter_menu.is_some() {
            self.filter_menu = None;
            self.last_filter_menu_dismiss = None;
            cx.notify();
            return;
        }

        // Check if we just dismissed the menu (within 100ms)
        // This handles the case where on_mouse_down_out fires before our click handler
        if let Some(dismiss_time) = self.last_filter_menu_dismiss {
            if dismiss_time.elapsed() < std::time::Duration::from_millis(100) {
                self.last_filter_menu_dismiss = None;
                cx.notify();
                return;
            }
        }
        self.last_filter_menu_dismiss = None;

        let menu = cx.new(|cx| FilterMenu::new(self.type_filter.clone(), cx));

        // Subscribe to filter change events
        cx.subscribe_in(&menu, window, |this, _, event: &FilterChanged, _, cx| {
            this.type_filter = event.0.clone();
            cx.notify();
        })
        .detach();

        cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, _, cx| {
            this.filter_menu = None;
            this.last_filter_menu_dismiss = Some(std::time::Instant::now());
            cx.notify();
        })
        .detach();

        self.filter_menu = Some(menu);
        cx.notify();
    }

    fn start_refresh(&mut self, cx: &mut Context<Self>) {
        if self.is_refreshing {
            return;
        }

        self.is_refreshing = true;
        cx.notify();

        // Refresh connections from storage
        self.connections = self.storage.get_all().unwrap_or_default();

        // Also refresh all expanded connection browsers
        let expanded_ids: Vec<String> = self.expanded_connections.iter().cloned().collect();
        for conn_id in expanded_ids {
            if let Some(browser) = self.connection_browsers.get(&conn_id) {
                browser.update(cx, |browser, cx| {
                    browser.load_databases(cx);
                });
            }
        }

        // Stop the spinning animation after a short delay
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(800))
                .await;
            this.update(cx, |this, cx| {
                this.is_refreshing = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn retry_connection(&mut self, conn_id: &str, cx: &mut Context<Self>) {
        if let Some(browser) = self.connection_browsers.get(conn_id) {
            browser.update(cx, |browser, cx| {
                browser.load_databases(cx);
            });
        }
    }

    /// Refresh a specific connection by ID
    fn refresh_single_connection(&mut self, conn_id: &str, cx: &mut Context<Self>) {
        if let Some(browser) = self.connection_browsers.get(conn_id) {
            browser.update(cx, |browser, cx| {
                browser.load_databases(cx);
            });
        }
    }

    /// Remove a connection from storage and UI state
    fn remove_connection(&mut self, conn_id: &str, cx: &mut Context<Self>) {
        // Delete from SQLite storage
        if let Err(e) = self.storage.delete(conn_id) {
            eprintln!("Failed to delete connection: {}", e);
            return;
        }

        // Remove from in-memory state
        self.connections.retain(|c| c.id != conn_id);
        self.expanded_connections.remove(conn_id);
        self.connection_browsers.remove(conn_id);

        // Close picker if open for this connection
        if self.database_picker_connection_id.as_deref() == Some(conn_id) {
            self.database_picker = None;
            self.database_picker_connection_id = None;
        }

        cx.notify();
    }

    fn show_database_picker(
        &mut self,
        conn_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Check if picker is already open for this connection - if so, close it (toggle behavior)
        if self.database_picker_connection_id.as_ref() == Some(&conn_id) {
            self.database_picker = None;
            self.database_picker_connection_id = None;
            self.last_picker_dismiss = None;
            cx.notify();
            return;
        }

        // Check if we just dismissed the picker for this connection (within 100ms)
        // This handles the case where on_mouse_down_out fires before our click handler
        if let Some(dismiss_time) = self.last_picker_dismiss {
            if dismiss_time.elapsed() < std::time::Duration::from_millis(100) {
                // This is a toggle click - don't reopen
                self.database_picker_connection_id = None;
                self.last_picker_dismiss = None;
                cx.notify();
                return;
            }
        }
        self.last_picker_dismiss = None;

        // Get database info from the browser
        let (all_databases, visible_databases, show_all) =
            if let Some(browser) = self.connection_browsers.get(&conn_id) {
                let browser = browser.read(cx);
                (
                    browser.database_names(),
                    browser.visible_databases(),
                    browser.is_showing_all(),
                )
            } else {
                (Vec::new(), Vec::new(), false)
            };

        if all_databases.is_empty() {
            return;
        }

        let picker =
            cx.new(|cx| DatabasePicker::new(all_databases, visible_databases, show_all, cx));

        // Subscribe to picker events
        cx.subscribe_in(&picker, window, {
            let conn_id = conn_id.clone();
            let storage = self.storage.clone();
            move |this, _, event: &DatabaseVisibilityChanged, _, cx| {
                // Update browser state
                if let Some(browser) = this.connection_browsers.get(&conn_id) {
                    browser.update(cx, |browser, cx| {
                        // Always update show_all state first
                        browser.set_show_all(event.show_all, cx);
                        // Then update visible databases if not showing all
                        if !event.show_all {
                            browser.set_visible_databases(event.visible_databases.clone(), cx);
                        }
                    });
                }

                // Save to storage
                if let Err(e) = storage.update_visible_databases(
                    &conn_id,
                    &event.visible_databases,
                    event.show_all,
                ) {
                    eprintln!("Failed to save visible databases: {}", e);
                }

                // Update in-memory connection
                if let Some(conn) = this.connections.iter_mut().find(|c| c.id == conn_id) {
                    conn.visible_databases = Some(event.visible_databases.clone());
                    conn.show_all_databases = Some(event.show_all);
                }
            }
        })
        .detach();

        cx.subscribe_in(&picker, window, |this, _, _: &DismissEvent, _, cx| {
            this.database_picker = None;
            this.last_picker_dismiss = Some(std::time::Instant::now());
            // Keep connection_id for a moment to detect toggle clicks
            cx.notify();
        })
        .detach();

        // Focus the search input inside the picker for immediate typing
        picker.update(cx, |picker, cx| {
            picker.focus_search(window, cx);
        });

        self.database_picker = Some(picker);
        self.database_picker_connection_id = Some(conn_id);
        cx.notify();
    }

    /// Show a context menu for a connection row
    fn show_connection_context_menu(
        &mut self,
        conn: Connection,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Dismiss any existing context menu
        self.context_menu = None;
        self.context_menu_target = None;

        let conn_name = conn.name.clone();

        let mut items = vec![
            // Properties - opens connection modal with this connection
            ContextMenuItem {
                label: "Properties".into(),
                icon: Some("icons/settings.svg"),
                style: ContextMenuItemStyle::Normal,
                action: ContextMenuAction::Custom("properties".into()),
            },
            // Query Console
            ContextMenuItem {
                label: "Query Console".into(),
                icon: Some("icons/terminal.svg"),
                style: ContextMenuItemStyle::Normal,
                action: ContextMenuAction::Custom("query_console".into()),
            },
            // Copy connection name
            ContextMenuItem {
                label: "Copy".into(),
                icon: Some("icons/copy.svg"),
                style: ContextMenuItemStyle::Normal,
                action: ContextMenuAction::Copy(conn_name),
            },
            // Refresh
            ContextMenuItem {
                label: "Refresh".into(),
                icon: Some("icons/refresh.svg"),
                style: ContextMenuItemStyle::Normal,
                action: ContextMenuAction::Custom("refresh".into()),
            },
            // Remove Connection
            ContextMenuItem {
                label: "Remove Connection".into(),
                icon: Some("icons/trash.svg"),
                style: ContextMenuItemStyle::Danger,
                action: ContextMenuAction::Custom("remove_connection".into()),
            },
        ];

        // Drop database - only for connection-level (this drops the whole connection's databases)
        // Actually "Drop" only appears on database/collection items, not connection-level
        // But per spec, no "Drop" on connections. We'll skip it here.
        let _ = &mut items; // suppress unused warning

        let window_size = window.viewport_size();
        let menu = cx.new(|cx| ContextMenu::new(items, position, window_size, cx));

        // Subscribe to context menu events
        let conn_clone = conn.clone();
        cx.subscribe_in(&menu, window, move |this, _, event: &ContextMenuEvent, _, cx| {
            match &event.action {
                ContextMenuAction::Custom(key) => match key.as_ref() {
                    "properties" => {
                        cx.emit(EditConnectionRequested(conn_clone.clone()));
                    }
                    "query_console" => {
                        // Not implemented yet
                    }
                    "refresh" => {
                        this.refresh_single_connection(&conn_clone.id, cx);
                    }
                    "remove_connection" => {
                        let id = conn_clone.id.clone();
                        this.remove_connection(&id, cx);
                    }
                    _ => {}
                },
                ContextMenuAction::Copy(_) => {
                    // Already handled by context menu itself (writes to clipboard)
                }
            }
            this.context_menu = None;
            this.context_menu_target = None;
            cx.notify();
        })
        .detach();

        cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, _, cx| {
            this.context_menu = None;
            this.context_menu_target = None;
            cx.notify();
        })
        .detach();

        menu.focus_handle(cx).focus(window);

        self.context_menu = Some(menu);
        self.context_menu_target = Some(ContextMenuTarget::Connection(conn));
        cx.notify();
    }

    /// Show a context menu for a database row
    pub fn show_database_context_menu(
        &mut self,
        conn_id: String,
        db_name: String,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Dismiss any existing context menu
        self.context_menu = None;
        self.context_menu_target = None;

        let copy_text = db_name.clone();

        let items = vec![
            // Query Console
            ContextMenuItem {
                label: "Query Console".into(),
                icon: Some("icons/terminal.svg"),
                style: ContextMenuItemStyle::Normal,
                action: ContextMenuAction::Custom("query_console".into()),
            },
            // Copy database name
            ContextMenuItem {
                label: "Copy".into(),
                icon: Some("icons/copy.svg"),
                style: ContextMenuItemStyle::Normal,
                action: ContextMenuAction::Copy(copy_text),
            },
            // Refresh
            ContextMenuItem {
                label: "Refresh".into(),
                icon: Some("icons/refresh.svg"),
                style: ContextMenuItemStyle::Normal,
                action: ContextMenuAction::Custom("refresh".into()),
            },
            // Drop database
            ContextMenuItem {
                label: "Drop".into(),
                icon: Some("icons/warning.svg"),
                style: ContextMenuItemStyle::Danger,
                action: ContextMenuAction::Custom("drop".into()),
            },
        ];

        let window_size = window.viewport_size();
        let menu = cx.new(|cx| ContextMenu::new(items, position, window_size, cx));

        let conn_id_clone = conn_id.clone();
        let db_name_clone = db_name.clone();
        cx.subscribe_in(&menu, window, move |this, _, event: &ContextMenuEvent, _, cx| {
            match &event.action {
                ContextMenuAction::Custom(key) => match key.as_ref() {
                    "query_console" => {
                        // Not implemented yet
                    }
                    "refresh" => {
                        this.refresh_single_connection(&conn_id_clone, cx);
                    }
                    "drop" => {
                        if let Some(conn) = this.connections.iter().find(|c| c.id == conn_id_clone) {
                            let conn_string = conn.get_connection_string();
                            let db_type = conn.db_type;
                            let db_name = db_name_clone.clone();
                            let conn_id = conn_id_clone.clone();
                            cx.spawn(async move |this, cx| {
                                let config = crate::db::driver::ConnectionConfig::new(
                                    db_type,
                                    conn_string,
                                );
                                let result = async {
                                    let driver = crate::db::driver::create_connection(config)?;
                                    driver.drop_database(&db_name).await
                                }.await;
                                match result {
                                    Ok(()) => {
                                        this.update(cx, |this, cx| {
                                            this.refresh_single_connection(&conn_id, cx);
                                        }).ok();
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to drop database: {}", e);
                                    }
                                }
                            }).detach();
                        }
                    }
                    _ => {}
                },
                ContextMenuAction::Copy(_) => {}
            }
            this.context_menu = None;
            this.context_menu_target = None;
            cx.notify();
        })
        .detach();

        cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, _, cx| {
            this.context_menu = None;
            this.context_menu_target = None;
            cx.notify();
        })
        .detach();

        menu.focus_handle(cx).focus(window);

        self.context_menu = Some(menu);
        self.context_menu_target = Some(ContextMenuTarget::Database(conn_id, db_name));
        cx.notify();
    }

    /// Show a context menu for a collection row
    pub fn show_collection_context_menu(
        &mut self,
        conn_id: String,
        db_name: String,
        coll_name: String,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Dismiss any existing context menu
        self.context_menu = None;
        self.context_menu_target = None;

        // Copy format: dbname.collection
        let copy_text = format!("{}.{}", db_name, coll_name);

        let items = vec![
            // Query Console
            ContextMenuItem {
                label: "Query Console".into(),
                icon: Some("icons/terminal.svg"),
                style: ContextMenuItemStyle::Normal,
                action: ContextMenuAction::Custom("query_console".into()),
            },
            // Copy full name (dbname.collection)
            ContextMenuItem {
                label: "Copy".into(),
                icon: Some("icons/copy.svg"),
                style: ContextMenuItemStyle::Normal,
                action: ContextMenuAction::Copy(copy_text),
            },
            // Refresh parent connection
            ContextMenuItem {
                label: "Refresh".into(),
                icon: Some("icons/refresh.svg"),
                style: ContextMenuItemStyle::Normal,
                action: ContextMenuAction::Custom("refresh".into()),
            },
            // Drop collection
            ContextMenuItem {
                label: "Drop".into(),
                icon: Some("icons/warning.svg"),
                style: ContextMenuItemStyle::Danger,
                action: ContextMenuAction::Custom("drop".into()),
            },
        ];

        let window_size = window.viewport_size();
        let menu = cx.new(|cx| ContextMenu::new(items, position, window_size, cx));

        let conn_id_clone = conn_id.clone();
        let db_name_clone = db_name.clone();
        let coll_name_clone = coll_name.clone();
        cx.subscribe_in(&menu, window, move |this, _, event: &ContextMenuEvent, _, cx| {
            match &event.action {
                ContextMenuAction::Custom(key) => match key.as_ref() {
                    "query_console" => {
                        // Not implemented yet
                    }
                    "refresh" => {
                        this.refresh_single_connection(&conn_id_clone, cx);
                    }
                    "drop" => {
                        if let Some(conn) = this.connections.iter().find(|c| c.id == conn_id_clone) {
                            let conn_string = conn.get_connection_string();
                            let db_type = conn.db_type;
                            let db_name = db_name_clone.clone();
                            let coll_name = coll_name_clone.clone();
                            let conn_id = conn_id_clone.clone();
                            cx.spawn(async move |this, cx| {
                                let config = crate::db::driver::ConnectionConfig::new(
                                    db_type,
                                    conn_string,
                                );
                                let result = async {
                                    let driver = crate::db::driver::create_connection(config)?;
                                    driver.drop_collection(&db_name, &coll_name).await
                                }.await;
                                match result {
                                    Ok(()) => {
                                        this.update(cx, |this, cx| {
                                            this.refresh_single_connection(&conn_id, cx);
                                        }).ok();
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to drop collection: {}", e);
                                    }
                                }
                            }).detach();
                        }
                    }
                    _ => {}
                },
                ContextMenuAction::Copy(_) => {}
            }
            this.context_menu = None;
            this.context_menu_target = None;
            cx.notify();
        })
        .detach();

        cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, _, cx| {
            this.context_menu = None;
            this.context_menu_target = None;
            cx.notify();
        })
        .detach();

        menu.focus_handle(cx).focus(window);

        self.context_menu = Some(menu);
        self.context_menu_target =
            Some(ContextMenuTarget::Collection(conn_id, db_name, coll_name));
        cx.notify();
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border_color = rgb(0x2a2a2a);
        let icon_color = rgb(0x808080);
        let icon_hover_color = rgb(0xe0e0e0);
        let bg_hover = rgba(0xffffff0f);
        let menu = self.database_menu.clone();
        let filter_menu = self.filter_menu.clone();
        let is_refreshing = self.is_refreshing;

        div()
            .id("sidebar-toolbar")
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(34.0))
            .px(px(8.0))
            .gap(px(2.0))
            .border_b_1()
            .border_color(border_color)
            // Add connection button with popover - use relative positioning
            .child(
                div()
                    .id("add-connection-wrapper")
                    .relative()
                    .child(
                        div()
                            .id("add-connection")
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(26.0))
                            .h(px(22.0))
                            .rounded_md()
                            .hover(|style| style.bg(bg_hover))
                            .active(|style| style.bg(rgba(0xffffff08)))
                            .child(
                                svg()
                                    .path("icons/plus.svg")
                                    .size(px(16.0))
                                    .text_color(icon_color)
                                    .hover(|style| style.text_color(icon_hover_color)),
                            )
                            .tooltip(Tooltip::text("Add new connection"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_database_menu(window, cx);
                            })),
                    )
                    // Menu positioned below the button
                    .when_some(menu, |el, menu| {
                        el.child(
                            deferred(div().absolute().top(px(26.0)).left_0().child(menu))
                                .with_priority(1),
                        )
                    }),
            )
            // Refresh button with spinning animation
            .child(
                div()
                    .id("refresh-connections")
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(26.0))
                    .h(px(22.0))
                    .rounded_md()
                    .hover(|style| style.bg(bg_hover))
                    .active(|style| style.bg(rgba(0xffffff08)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.start_refresh(cx);
                    }))
                    .tooltip(Tooltip::text("Refresh connections"))
                    .child(
                        svg()
                            .path("icons/refresh.svg")
                            .size(px(16.0))
                            .text_color(if is_refreshing {
                                rgb(0x0078d4)
                            } else {
                                icon_color
                            })
                            .hover(|style| style.text_color(icon_hover_color))
                            .with_animation(
                                "refresh-spin",
                                Animation::new(std::time::Duration::from_millis(1000)).repeat(),
                                move |el, delta| {
                                    if is_refreshing {
                                        el.with_transformation(Transformation::rotate(percentage(delta)))
                                    } else {
                                        el
                                    }
                                },
                            ),
                    ),
            )
            // Filter button with menu
            .child(
                div()
                    .id("filter-connections-wrapper")
                    .relative()
                    .child(
                        div()
                            .id("filter-connections")
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(26.0))
                            .h(px(22.0))
                            .rounded_md()
                            .hover(|style| style.bg(bg_hover))
                            .active(|style| style.bg(rgba(0xffffff08)))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_filter_menu(window, cx);
                            }))
                            .tooltip(Tooltip::text("Filter by database type"))
                            .child(
                                svg()
                                    .path("icons/filter.svg")
                                    .size(px(16.0))
                                    .text_color(if self.type_filter.is_empty() {
                                        icon_color
                                    } else {
                                        rgb(0x0078d4) // Highlight when filter is active
                                    })
                                    .hover(|style| style.text_color(icon_hover_color)),
                            ),
                    )
                    // Filter menu positioned below the button
                    .when_some(filter_menu, |el, menu| {
                        el.child(
                            deferred(div().absolute().top(px(26.0)).left_0().child(menu))
                                .with_priority(1),
                        )
                    }),
            )
    }

    fn toggle_connection(&mut self, conn: &Connection, cx: &mut Context<Self>) {
        let id = conn.id.clone();
        if self.expanded_connections.contains(&id) {
            self.expanded_connections.remove(&id);
        } else {
            self.expanded_connections.insert(id.clone());
            // Create or get the connection browser for this connection
            if !self.connection_browsers.contains_key(&id) {
                let browser = cx.new(|_cx| ConnectionBrowser::new(conn.clone()));
                let connection_string = conn.get_connection_string();

                // Get saved visible databases and show_all state
                let saved_visible_dbs = conn.visible_databases.clone();
                let saved_show_all = conn.show_all_databases.unwrap_or(false);

                cx.subscribe(&browser, {
                    let connection_string = connection_string.clone();
                    move |_this, _, event: &CollectionSelected, cx| {
                        println!("Collection selected: {}.{}", event.0, event.1);
                        // Emit event to workspace
                        cx.emit(OpenCollectionRequested {
                            database_name: event.0.clone(),
                            collection_name: event.1.clone(),
                            connection_string: connection_string.clone(),
                        });
                        cx.notify();
                    }
                })
                .detach();

                // Subscribe to database context menu events
                cx.subscribe(&browser, |this, _, event: &DatabaseContextMenuRequested, cx| {
                    this.pending_db_context_menu = Some((event.0.clone(), event.1.clone(), event.2));
                    cx.notify();
                })
                .detach();

                // Subscribe to collection context menu events
                cx.subscribe(&browser, |this, _, event: &CollectionContextMenuRequested, cx| {
                    this.pending_coll_context_menu = Some((event.0.clone(), event.1.clone(), event.2.clone(), event.3));
                    cx.notify();
                })
                .detach();

                self.connection_browsers.insert(id.clone(), browser.clone());

                // Load databases for MongoDB connections
                if conn.db_type == DatabaseType::MongoDB {
                    // Set the initial visible databases before loading (will be applied after load completes)
                    browser.update(cx, |browser, _cx| {
                        browser.set_initial_visible_databases(saved_visible_dbs, saved_show_all);
                    });

                    browser.update(cx, |browser, cx| {
                        browser.load_databases(cx);
                    });
                }
            } else if conn.db_type == DatabaseType::MongoDB {
                // Browser already exists - if it's in NotConnected state (preview only),
                // trigger the actual connection
                let browser = self.connection_browsers.get(&id).unwrap().clone();
                let needs_connect = browser.read(cx).is_not_connected();
                if needs_connect {
                    browser.update(cx, |browser, cx| {
                        browser.load_databases(cx);
                    });
                }
            }
        }
        cx.notify();
    }

    fn render_connections(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let hover_bg = rgb(0x252525);
        let text_color = rgb(0xe0e0e0);
        let text_muted = rgb(0x808080);
        let accent_color = rgb(0x0078d4);
        let error_color = rgb(0xf44336);

        // Filter connections by type if filter is active
        let filtered_connections: Vec<&Connection> = if self.type_filter.is_empty() {
            self.connections.iter().collect()
        } else {
            self.connections
                .iter()
                .filter(|c| self.type_filter.contains(&c.db_type))
                .collect()
        };

        div()
            .id("connections-list")
            .flex_1()
            .w_full()
            .overflow_y_scroll()
            .py(px(4.0))
            .children(filtered_connections.iter().map(|conn| {
                let conn_id = conn.id.clone();
                let conn_name = conn.name.clone();
                let db_type = conn.db_type;
                let is_expanded = self.expanded_connections.contains(&conn.id);
                let conn_clone = (*conn).clone();

                // Get database count and error state if browser exists
                let (db_count, visible_count, is_loading, has_error, is_not_connected) =
                    if let Some(browser) = self.connection_browsers.get(&conn_id) {
                        let browser = browser.read(cx);
                        (
                            browser.database_count(),
                            browser.visible_count(),
                            matches!(browser.loading_state, LoadingState::LoadingDatabases),
                            matches!(browser.loading_state, LoadingState::Error(_)),
                            browser.is_not_connected(),
                        )
                    } else {
                        (0, 0, false, false, false)
                    };

                div()
                    .id(SharedString::from(format!("conn-{}", conn_id)))
                    .flex()
                    .flex_col()
                    .w_full()
                    // Connection row
                    .child(
                        div()
                            .id(SharedString::from(format!("conn-row-{}", conn_id)))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .w_full()
                            .px(px(8.0))
                            .py(px(6.0))
                            .cursor_pointer()
                            .rounded(px(4.0))
                            .hover(|s| s.bg(hover_bg))
                            .on_click(cx.listener({
                                let conn_clone = conn_clone.clone();
                                move |this, _, _, cx| {
                                    this.toggle_connection(&conn_clone, cx);
                                }
                            }))
                            // Right-click context menu
                            .on_mouse_down(MouseButton::Right, cx.listener({
                                let conn_clone = conn_clone.clone();
                                move |this, event: &MouseDownEvent, window, cx| {
                                    this.show_connection_context_menu(
                                        conn_clone.clone(),
                                        event.position,
                                        window,
                                        cx,
                                    );
                                    cx.stop_propagation();
                                }
                            }))
                            // Collapse/expand chevron
                            .child(
                                svg()
                                    .path(if is_expanded {
                                        "icons/chevron-down.svg"
                                    } else {
                                        "icons/chevron-right.svg"
                                    })
                                    .size(px(12.0))
                                    .text_color(text_muted)
                                    .flex_none(),
                            )
                            // Database icon
                            .child(img(db_type.icon_path()).size(px(16.0)).flex_none())
                            // Connection name with count badge inline
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_size(px(13.0))
                                            .text_color(text_color)
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(conn_name),
                                    )
                                    // Database count badge (clickable, only when expanded, has databases, and connected)
                                    // Wrapped in relative container so picker can be positioned absolutely below it
                                    .when(is_expanded && db_count > 0 && !is_not_connected, |el| {
                                        let is_picker_open =
                                            self.database_picker_connection_id.as_ref()
                                                == Some(&conn_id);
                                        let picker = self.database_picker.clone();

                                        el.child(
                                            div()
                                                .id(SharedString::from(format!(
                                                    "db-count-wrapper-{}",
                                                    conn_id
                                                )))
                                                .relative() // Create positioning context for picker
                                                .child(
                                                    div()
                                                        .id(SharedString::from(format!(
                                                            "db-count-{}",
                                                            conn_id
                                                        )))
                                                        .cursor_pointer()
                                                        .px(px(6.0))
                                                        .py(px(2.0))
                                                        .rounded(px(4.0))
                                                        .hover(|s| s.bg(rgb(0x333333)))
                                                        .on_click(cx.listener({
                                                            let conn_id = conn_id.clone();
                                                            move |this, _, window, cx| {
                                                                this.show_database_picker(
                                                                    conn_id.clone(),
                                                                    window,
                                                                    cx,
                                                                );
                                                                cx.stop_propagation();
                                                            }
                                                        }))
                                                        .child(
                                                            div()
                                                                .text_size(px(10.0))
                                                                .text_color(text_muted)
                                                                .child(format!(
                                                                    "{} of {}",
                                                                    visible_count, db_count
                                                                )),
                                                        ),
                                                )
                                                // Database picker dropdown positioned below the badge
                                                .when(is_picker_open, |el| {
                                                    if let Some(picker) = picker {
                                                        return el.child(
                                                            deferred(
                                                                div()
                                                                    .absolute()
                                                                    .top(px(26.0))
                                                                    .left_0()
                                                                    .w(px(280.0))
                                                                    .child(picker)
                                                                    .occlude(),
                                                            )
                                                            .with_priority(1),
                                                        );
                                                    }
                                                    el
                                                }),
                                        )
                                    }),
                            )
                            // Loading indicator
                            .when(is_expanded && is_loading, |el| {
                                el.child(
                                    div().px(px(6.0)).py(px(2.0)).child(
                                        div()
                                            .text_size(px(10.0))
                                            .text_color(accent_color)
                                            .child("Loading..."),
                                    ),
                                )
                            })
                            // Error indicator with retry button
                            .when(is_expanded && has_error && !is_loading, |el| {
                                el.child(
                                    div()
                                        .id(SharedString::from(format!("retry-{}", conn_id)))
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(4.0))
                                        .px(px(6.0))
                                        .py(px(2.0))
                                        .rounded(px(4.0))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(0x3a2020)))
                                        .on_click(cx.listener({
                                            let conn_id = conn_id.clone();
                                            move |this, _, _, cx| {
                                                this.retry_connection(&conn_id, cx);
                                                cx.stop_propagation();
                                            }
                                        }))
                                        .tooltip(Tooltip::text("Click to retry"))
                                        .child(
                                            svg()
                                                .path("icons/refresh.svg")
                                                .size(px(10.0))
                                                .text_color(error_color),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.0))
                                                .text_color(error_color)
                                                .child("Error"),
                                        ),
                                )
                            }),
                    )
                    // Expanded content - show connection browser for MongoDB
                    .when(is_expanded, |el| {
                        if db_type == DatabaseType::MongoDB {
                            if let Some(browser) = self.connection_browsers.get(&conn_id) {
                                el.child(
                                    div()
                                        .pl(px(28.0))
                                        .pr(px(4.0))
                                        .py(px(4.0))
                                        .child(browser.clone()),
                                )
                            } else {
                                el.child(
                                    div()
                                        .pl(px(36.0))
                                        .pr(px(8.0))
                                        .py(px(4.0))
                                        .text_size(px(12.0))
                                        .text_color(text_muted)
                                        .child("Loading..."),
                                )
                            }
                        } else {
                            el.child(
                                div()
                                    .pl(px(36.0))
                                    .pr(px(8.0))
                                    .py(px(4.0))
                                    .text_size(px(12.0))
                                    .text_color(text_muted)
                                    .child("Connect to browse..."),
                            )
                        }
                    })
            }))
            // Empty state
            .when(filtered_connections.is_empty(), |el| {
                if self.connections.is_empty() {
                    el.child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .py(px(32.0))
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(text_muted)
                                    .child("No connections"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(rgb(0x606060))
                                    .child("Click + to add one"),
                            ),
                    )
                } else {
                    // Filter is hiding all connections
                    el.child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .py(px(32.0))
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(text_muted)
                                    .child("No matching connections"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(rgb(0x606060))
                                    .child("Adjust filter to see more"),
                            ),
                    )
                }
            })
    }
}

impl Render for Sidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Process pending database context menu request (deferred from subscribe which lacks window)
        if let Some((conn_id, db_name, position)) = self.pending_db_context_menu.take() {
            self.show_database_context_menu(conn_id, db_name, position, window, cx);
        }

        // Process pending collection context menu request
        if let Some((conn_id, db_name, coll_name, position)) = self.pending_coll_context_menu.take() {
            self.show_collection_context_menu(conn_id, db_name, coll_name, position, window, cx);
        }

        let sidebar_bg = rgb(0x1a1a1a);
        let border_color = rgb(0x2a2a2a);
        let handle_hover_color = rgb(0x0078d4);
        let context_menu = self.context_menu.clone();

        div()
            .id("sidebar-container")
            .flex()
            .flex_row()
            .h_full()
            // Sidebar content area
            .child(
                div()
                    .id("sidebar")
                    .flex()
                    .flex_col()
                    .h_full()
                    .w(self.width)
                    .bg(sidebar_bg)
                    .border_r_1()
                    .border_color(border_color)
                    // Toolbar row at top
                    .child(self.render_toolbar(cx))
                    // Connections list
                    .child(self.render_connections(cx)),
            )
            // Resize handle
            .child(
                div()
                    .id("sidebar-resize-handle")
                    .h_full()
                    .w(px(RESIZE_HANDLE_SIZE))
                    .cursor_col_resize()
                    .bg(transparent_black())
                    .hover(|style| style.bg(handle_hover_color))
                    .active(|style| style.bg(handle_hover_color))
                    // Drag handling
                    .on_drag(DraggedSidebar, |_, _, _, cx| {
                        cx.stop_propagation();
                        cx.new(|_| DraggedSidebar)
                    })
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    // Double-click to reset to default width
                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                        if event.click_count() == 2 {
                            this.width = px(DEFAULT_SIDEBAR_WIDTH);
                            cx.notify();
                        }
                    })),
            )
            // Context menu rendered as deferred overlay at absolute position
            .when_some(context_menu, |el, menu| {
                el.child(deferred(menu).with_priority(2))
            })
    }
}
