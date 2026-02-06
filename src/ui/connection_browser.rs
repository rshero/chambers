use gpui::{prelude::*, *};
use std::collections::{HashMap, HashSet};

use crate::db::driver::{CollectionInfo, DatabaseInfo};
use crate::db::{Connection, ConnectionConfig, DatabaseType};
use crate::db::driver::create_connection;
use crate::ui::tooltip::Tooltip;

/// Maximum number of databases to show initially
const MAX_DATABASES_SHOWN: usize = 10;

/// Uniform item height for virtual list (both databases and collections)
const ITEM_HEIGHT: f32 = 26.0;

/// Pre-computed flat tree item for efficient rendering
/// All strings are pre-computed SharedStrings to avoid allocations during render
#[derive(Clone)]
enum FlatTreeItem {
    Database {
        /// Stable unique key for element ID
        stable_key: SharedString,
        /// Database name (pre-computed SharedString)
        name: SharedString,
        /// Pre-formatted size string (e.g., "1.2 GB")
        formatted_size: Option<SharedString>,
        /// Whether this database is expanded
        is_expanded: bool,
        /// Whether collections are currently loading
        is_loading: bool,
    },
    Collection {
        /// Stable unique key for element ID
        stable_key: SharedString,
        /// Parent database name (for click handler)
        db_name: SharedString,
        /// Collection name (pre-computed SharedString)
        name: SharedString,
        /// Pre-formatted document count
        doc_count: Option<SharedString>,
        /// Whether this collection is selected
        is_selected: bool,
    },
    /// Loading placeholder shown when collections are loading
    Loading {
        stable_key: SharedString,
    },
    /// Error placeholder shown when collection loading failed
    Error {
        stable_key: SharedString,
        db_name: SharedString,
    },
    /// Empty placeholder shown when database has no collections
    Empty {
        stable_key: SharedString,
    },
}

/// Event emitted when a database is selected (for tree expansion)
#[derive(Clone)]
#[allow(dead_code)] // Event type kept for potential future use
pub struct DatabaseSelected(pub String);

#[allow(dead_code)] // Kept for potential future use
impl EventEmitter<DatabaseSelected> for ConnectionBrowser {}

/// Event emitted when a collection is selected
#[derive(Clone)]
pub struct CollectionSelected(pub String, pub String); // (database_name, collection_name)

impl EventEmitter<CollectionSelected> for ConnectionBrowser {}

/// Loading state for the browser
#[derive(Clone, PartialEq)]
pub enum LoadingState {
    /// Not yet connected - showing saved database names as preview
    NotConnected,
    Idle,
    LoadingDatabases,
    Error(String),
}

/// Loading state for collections (per database)
#[derive(Clone, PartialEq)]
pub enum CollectionLoadingState {
    NotLoaded,
    Loading,
    Loaded,
    Error(String),
}

/// The connection browser component that shows databases and collections
pub struct ConnectionBrowser {
    connection: Connection,
    // Source data
    databases: Vec<DatabaseInfo>,
    collections: HashMap<String, Vec<CollectionInfo>>,
    expanded_databases: HashSet<String>,
    collection_loading_states: HashMap<String, CollectionLoadingState>,
    selected_database: Option<String>,
    selected_collection: Option<(String, String)>,
    pub loading_state: LoadingState,
    visible_databases: Vec<String>,
    show_all_databases: bool,
    /// Whether the user has saved preferences (to distinguish from "no preferences yet")
    has_saved_preferences: bool,
    
    // Pre-computed render cache (flattened tree)
    flat_items: Vec<FlatTreeItem>,
    flat_items_dirty: bool,
}

impl ConnectionBrowser {
    pub fn new(connection: Connection) -> Self {
        Self {
            connection,
            databases: Vec::new(),
            collections: HashMap::new(),
            expanded_databases: HashSet::new(),
            collection_loading_states: HashMap::new(),
            selected_database: None,
            selected_collection: None,
            loading_state: LoadingState::NotConnected,
            visible_databases: Vec::new(),
            show_all_databases: false,
            has_saved_preferences: false,
            flat_items: Vec::new(),
            flat_items_dirty: true,
        }
    }

    /// Get the count of databases
    pub fn database_count(&self) -> usize {
        self.databases.len()
    }

    /// Get the count of visible databases
    pub fn visible_count(&self) -> usize {
        if self.show_all_databases {
            self.databases.len()
        } else {
            self.visible_databases.len().min(self.databases.len())
        }
    }

    /// Get the list of all database names
    pub fn database_names(&self) -> Vec<String> {
        self.databases.iter().map(|db| db.name.clone()).collect()
    }

    /// Get the list of visible database names
    pub fn visible_databases(&self) -> Vec<String> {
        self.visible_databases.clone()
    }

    /// Set which databases are visible (for filtering)
    pub fn set_visible_databases(&mut self, databases: Vec<String>, cx: &mut Context<Self>) {
        self.visible_databases = databases;
        self.show_all_databases = false;
        self.has_saved_preferences = true;
        self.flat_items_dirty = true;
        cx.notify();
    }

    /// Set show_all state
    pub fn set_show_all(&mut self, show_all: bool, cx: &mut Context<Self>) {
        self.show_all_databases = show_all;
        self.has_saved_preferences = true;
        self.flat_items_dirty = true;
        cx.notify();
    }

    /// Check if currently showing all databases
    pub fn is_showing_all(&self) -> bool {
        self.show_all_databases
    }

    /// Set the initial visible databases (called before databases are loaded)
    /// This is used to restore the user's database picker selection on app restart
    /// If databases is Some (even if empty), we have saved preferences
    pub fn set_initial_visible_databases(&mut self, databases: Option<Vec<String>>, show_all: bool) {
        self.show_all_databases = show_all;
        if let Some(dbs) = databases {
            self.visible_databases = dbs;
            self.has_saved_preferences = true;
        }
        // If databases is None and show_all is true, user explicitly chose "show all"
        if show_all {
            self.has_saved_preferences = true;
        }
        self.flat_items_dirty = true;
    }

    /// Populate the database list from saved names only (no connection required).
    /// This shows the saved databases as a preview in the tree without connecting.
    pub fn set_saved_databases_preview(&mut self, database_names: Vec<String>) {
        self.databases = database_names
            .iter()
            .map(|name| DatabaseInfo {
                name: name.clone(),
                size_bytes: None,
            })
            .collect();
        self.loading_state = LoadingState::NotConnected;
        self.flat_items_dirty = true;
    }

    /// Check if the browser is in the not-connected preview state
    pub fn is_not_connected(&self) -> bool {
        self.loading_state == LoadingState::NotConnected
    }

    /// Get the connection ID
    #[allow(dead_code)] // API method for external access
    pub fn connection_id(&self) -> &str {
        &self.connection.id
    }

    /// Simple hash function for stable keys
    fn hash_name(name: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        name.hash(&mut hasher);
        hasher.finish()
    }

    fn format_bytes(bytes: u64) -> String {
        const UNITS: &[&str; 5] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;
        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }
        format!("{:.1} {}", size, UNITS[unit_index])
    }

    /// Rebuild the flattened tree cache
    /// Called ONCE when state changes, NOT during render
    fn rebuild_flat_items(&mut self) {
        self.flat_items.clear();
        
        // Determine which databases to show
        let visible_db_names: HashSet<&str> = if self.show_all_databases {
            self.databases.iter().map(|db| db.name.as_str()).collect()
        } else {
            self.visible_databases.iter().map(|s| s.as_str()).collect()
        };

        for db in &self.databases {
            // Skip if not in visible set
            if !visible_db_names.contains(db.name.as_str()) {
                continue;
            }

            let is_expanded = self.expanded_databases.contains(&db.name);
            let coll_state = self.collection_loading_states
                .get(&db.name)
                .cloned()
                .unwrap_or(CollectionLoadingState::NotLoaded);
            let is_loading = matches!(coll_state, CollectionLoadingState::Loading);
            let has_error = matches!(coll_state, CollectionLoadingState::Error(_));

            // Add database row
            self.flat_items.push(FlatTreeItem::Database {
                stable_key: SharedString::from(format!("db-{:016x}", Self::hash_name(&db.name))),
                name: SharedString::from(db.name.clone()),
                formatted_size: db.size_bytes.map(|s| SharedString::from(Self::format_bytes(s))),
                is_expanded,
                is_loading,
            });

            // Add children if expanded
            if is_expanded {
                if is_loading {
                    // Loading placeholder
                    self.flat_items.push(FlatTreeItem::Loading {
                        stable_key: SharedString::from(format!("loading-{}", db.name)),
                    });
                } else if has_error {
                    // Error placeholder
                    self.flat_items.push(FlatTreeItem::Error {
                        stable_key: SharedString::from(format!("error-{}", db.name)),
                        db_name: SharedString::from(db.name.clone()),
                    });
                } else if let Some(colls) = self.collections.get(&db.name) {
                    if colls.is_empty() {
                        // Empty placeholder
                        self.flat_items.push(FlatTreeItem::Empty {
                            stable_key: SharedString::from(format!("empty-{}", db.name)),
                        });
                    } else {
                        // Collection rows
                        for (idx, coll) in colls.iter().enumerate() {
                            let is_selected = self.selected_collection
                                .as_ref()
                                .map(|(d, c)| d == &db.name && c == &coll.name)
                                .unwrap_or(false);

                            self.flat_items.push(FlatTreeItem::Collection {
                                stable_key: SharedString::from(format!(
                                    "coll-{:016x}-{}",
                                    Self::hash_name(&db.name),
                                    idx
                                )),
                                db_name: SharedString::from(db.name.clone()),
                                name: SharedString::from(coll.name.clone()),
                                doc_count: coll.document_count.map(|c| SharedString::from(c.to_string())),
                                is_selected,
                            });
                        }
                    }
                }
            }
        }

        self.flat_items_dirty = false;
    }

    pub fn load_databases(&mut self, cx: &mut Context<Self>) {
        if self.connection.db_type != DatabaseType::MongoDB {
            return;
        }

        // Remember which databases were expanded (from preview state)
        let previously_expanded = self.expanded_databases.clone();

        self.loading_state = LoadingState::LoadingDatabases;
        self.flat_items_dirty = true;

        let conn_string = self.connection.get_connection_string();
        let config = ConnectionConfig::new(self.connection.db_type, conn_string);

        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                match create_connection(config) {
                    Ok(conn) => conn.list_databases().await,
                    Err(e) => Err(e),
                }
            });
            tx.send(result).ok();
        });

        cx.spawn(async move |this, cx| {
            loop {
                match rx.try_recv() {
                    Ok(result) => {
                        this.update(cx, |browser, cx| {
                            match result {
                                Ok(databases) => {
                                    browser.databases = databases;
                                    browser.loading_state = LoadingState::Idle;

                                    // Only apply default if user has NO saved preferences
                                    // If has_saved_preferences is true, respect the saved state
                                    // (even if it results in showing all or showing a specific list)
                                    if !browser.has_saved_preferences {
                                        // No saved preferences - use default (first N databases)
                                        browser.visible_databases = browser.databases
                                            .iter()
                                            .take(MAX_DATABASES_SHOWN)
                                            .map(|db| db.name.clone())
                                            .collect();
                                    }
                                    // If has_saved_preferences is true:
                                    // - show_all_databases=true means show all
                                    // - visible_databases contains the specific list to show

                                    // Load collections for any databases that were expanded in preview
                                    for db_name in &previously_expanded {
                                        let db_exists = browser.databases.iter().any(|db| &db.name == db_name);
                                        if db_exists {
                                            browser.expanded_databases.insert(db_name.clone());
                                            browser.load_collections(db_name, cx);
                                        }
                                    }
                                }
                                Err(e) => {
                                    browser.loading_state = LoadingState::Error(e.to_string());
                                }
                            }
                            browser.flat_items_dirty = true;
                            cx.notify();
                        }).ok();
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        cx.background_executor().timer(std::time::Duration::from_millis(50)).await;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        this.update(cx, |browser, cx| {
                            browser.loading_state = LoadingState::Error("Failed to load databases".to_string());
                            browser.flat_items_dirty = true;
                            cx.notify();
                        }).ok();
                        break;
                    }
                }
            }
        }).detach();

        cx.notify();
    }

    /// Toggle database expansion and load collections if needed
    pub fn toggle_database(&mut self, db_name: &str, cx: &mut Context<Self>) {
        let db_name = db_name.to_string();

        if self.expanded_databases.contains(&db_name) {
            self.expanded_databases.remove(&db_name);
            self.selected_database = None;
        } else {
            self.expanded_databases.insert(db_name.clone());
            self.selected_database = Some(db_name.clone());

            // If we're in preview mode (not connected), trigger a real connection
            if self.is_not_connected() {
                self.load_databases(cx);
            }

            let load_state = self.collection_loading_states
                .get(&db_name)
                .cloned()
                .unwrap_or(CollectionLoadingState::NotLoaded);

            if !self.is_not_connected() && matches!(load_state, CollectionLoadingState::NotLoaded | CollectionLoadingState::Error(_)) {
                self.load_collections(&db_name, cx);
            }
        }
        
        self.flat_items_dirty = true;
        cx.notify();
    }

    /// Load collections for a specific database
    fn load_collections(&mut self, db_name: &str, cx: &mut Context<Self>) {
        let db_name = db_name.to_string();

        self.collection_loading_states
            .insert(db_name.clone(), CollectionLoadingState::Loading);
        self.flat_items_dirty = true;

        let conn_string = self.connection.get_connection_string();
        let config = ConnectionConfig::new(self.connection.db_type, conn_string);
        let db_name_clone = db_name.clone();

        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                match create_connection(config) {
                    Ok(conn) => conn.list_collections(&db_name_clone).await,
                    Err(e) => Err(e),
                }
            });
            tx.send(result).ok();
        });

        let db_name_for_task = db_name.clone();
        cx.spawn(async move |this, cx| {
            loop {
                match rx.try_recv() {
                    Ok(result) => {
                        this.update(cx, |browser, cx| {
                            match result {
                                Ok(collections) => {
                                    browser.collections.insert(db_name_for_task.clone(), collections);
                                    browser.collection_loading_states
                                        .insert(db_name_for_task.clone(), CollectionLoadingState::Loaded);
                                }
                                Err(e) => {
                                    browser.collection_loading_states
                                        .insert(db_name_for_task.clone(), CollectionLoadingState::Error(e.to_string()));
                                }
                            }
                            browser.flat_items_dirty = true;
                            cx.notify();
                        }).ok();
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        cx.background_executor().timer(std::time::Duration::from_millis(50)).await;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        this.update(cx, |browser, cx| {
                            browser.collection_loading_states
                                .insert(db_name_for_task.clone(), CollectionLoadingState::Error("Connection lost".to_string()));
                            browser.flat_items_dirty = true;
                            cx.notify();
                        }).ok();
                        break;
                    }
                }
            }
        }).detach();

        cx.notify();
    }

    /// Select a collection and emit event
    pub fn select_collection(&mut self, db_name: &str, coll_name: &str, cx: &mut Context<Self>) {
        self.selected_collection = Some((db_name.to_string(), coll_name.to_string()));
        self.flat_items_dirty = true;
        cx.emit(CollectionSelected(db_name.to_string(), coll_name.to_string()));
        cx.notify();
    }

    /// Retry loading collections for a database
    fn retry_load_collections(&mut self, db_name: &str, cx: &mut Context<Self>) {
        self.load_collections(db_name, cx);
    }
}

impl Render for ConnectionBrowser {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Colors - defined once at top
        let text_color = rgb(0xe0e0e0);
        let text_muted = rgb(0x808080);
        let hover_bg = rgb(0x252525);
        let accent_color = rgb(0x0078d4);
        let error_color = rgb(0xf44336);

        // Rebuild flat items if dirty (BEFORE render, not during)
        if self.flat_items_dirty {
            self.rebuild_flat_items();
        }

        // Loading databases state
        if matches!(self.loading_state, LoadingState::LoadingDatabases) {
            return div()
                .id("connection-browser")
                .flex()
                .flex_col()
                .w_full()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .px(px(8.0))
                        .py(px(8.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(text_muted)
                                .child("Loading databases..."),
                        ),
                )
                .into_any_element();
        }

        // Error state
        if matches!(self.loading_state, LoadingState::Error(_)) {
            return div()
                .id("connection-browser")
                .flex()
                .flex_col()
                .w_full()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .px(px(8.0))
                        .py(px(4.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(error_color)
                                .child("Failed to load databases"),
                        )
                        .child(
                            div()
                                .id("retry-load-databases")
                                .cursor_pointer()
                                .w(px(18.0))
                                .h(px(18.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(3.0))
                                .hover(|s| s.bg(rgb(0x333333)))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.load_databases(cx);
                                }))
                                .tooltip(Tooltip::text("Retry loading databases"))
                                .child(
                                    svg()
                                        .path("icons/refresh.svg")
                                        .size(px(14.0))
                                        .text_color(text_muted),
                                ),
                        ),
                )
                .into_any_element();
        }

        // Empty state
        if self.databases.is_empty() {
            return div()
                .id("connection-browser")
                .flex()
                .flex_col()
                .w_full()
                .child(
                    div()
                        .px(px(8.0))
                        .py(px(4.0))
                        .text_size(px(11.0))
                        .text_color(text_muted)
                        .child("No databases"),
                )
                .into_any_element();
        }

        // Main tree view with uniform_list for virtualization
        let item_count = self.flat_items.len();
        let flat_items = self.flat_items.clone(); // Single clone for the processor

        div()
            .id("connection-browser")
            .flex()
            .flex_col()
            .w_full()
            .flex_1()
            .overflow_hidden()
            .child(
                uniform_list(
                    "database-tree",
                    item_count,
                    cx.processor(move |browser, range: std::ops::Range<usize>, _window, cx| {
                        range
                            .filter_map(|ix| {
                                let item = flat_items.get(ix)?;
                                Some(render_flat_item(
                                    item,
                                    text_color,
                                    text_muted,
                                    hover_bg,
                                    accent_color,
                                    error_color,
                                    cx,
                                    browser,
                                ))
                            })
                            .collect()
                    }),
                )
                .size_full()
                .with_sizing_behavior(ListSizingBehavior::Infer),
            )
            .into_any_element()
    }
}

/// Render a single flat tree item - kept lightweight, uses pre-computed data
#[allow(clippy::too_many_arguments)]
fn render_flat_item(
    item: &FlatTreeItem,
    text_color: Rgba,
    text_muted: Rgba,
    hover_bg: Rgba,
    accent_color: Rgba,
    error_color: Rgba,
    cx: &mut Context<ConnectionBrowser>,
    _browser: &ConnectionBrowser,
) -> AnyElement {
    match item {
        FlatTreeItem::Database {
            stable_key,
            name,
            formatted_size,
            is_expanded,
            is_loading,
        } => {
            let db_name = name.to_string();
            let is_exp = *is_expanded;
            let is_load = *is_loading;

            div()
                .id(stable_key.clone())
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .w_full()
                .h(px(ITEM_HEIGHT))
                .px(px(8.0))
                .cursor_pointer()
                .rounded(px(4.0))
                .hover(|s| s.bg(hover_bg))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_database(&db_name, cx);
                }))
                // Chevron
                .child(
                    svg()
                        .path(if is_exp {
                            "icons/chevron-down.svg"
                        } else {
                            "icons/chevron-right.svg"
                        })
                        .size(px(10.0))
                        .text_color(text_muted)
                        .flex_none(),
                )
                // Database icon
                .child(
                    svg()
                        .path("icons/database-folder.svg")
                        .size(px(14.0))
                        .text_color(if is_exp { accent_color } else { text_muted })
                        .flex_none(),
                )
                // Name
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.0))
                        .text_color(if is_exp { accent_color } else { text_color })
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(name.clone()),
                )
                // Size
                .when_some(formatted_size.clone(), |el, size| {
                    el.child(
                        div()
                            .text_size(px(10.0))
                            .text_color(text_muted)
                            .child(size),
                    )
                })
                // Loading indicator
                .when(is_load, |el| {
                    el.child(
                        div()
                            .text_size(px(10.0))
                            .text_color(text_muted)
                            .child("..."),
                    )
                })
                .into_any_element()
        }

        FlatTreeItem::Collection {
            stable_key,
            db_name,
            name,
            doc_count,
            is_selected,
        } => {
            let db = db_name.to_string();
            let coll = name.to_string();
            let selected = *is_selected;

            div()
                .id(stable_key.clone())
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .w_full()
                .h(px(ITEM_HEIGHT))
                .pl(px(32.0)) // Indentation for collections
                .pr(px(8.0))
                .cursor_pointer()
                .rounded(px(4.0))
                // Subtle selection: light background tint instead of solid color
                .when(selected, |el| el.bg(rgba(0x0078d420))) // 12% opacity accent
                .hover(|s| s.bg(hover_bg))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_collection(&db, &coll, cx);
                }))
                // Left accent bar for selected item
                .child(
                    div()
                        .w(px(2.0))
                        .h(px(14.0))
                        .rounded(px(1.0))
                        .when(selected, |el| el.bg(accent_color))
                        .when(!selected, |el| el.bg(gpui::transparent_black())),
                )
                // Collection icon
                .child(
                    svg()
                        .path("icons/collection.svg")
                        .size(px(12.0))
                        .text_color(if selected { accent_color } else { text_muted })
                        .flex_none(),
                )
                // Name
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.0))
                        .text_color(if selected { accent_color } else { text_color })
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(name.clone()),
                )
                // Doc count
                .when_some(doc_count.clone(), |el, count| {
                    el.child(
                        div()
                            .text_size(px(10.0))
                            .text_color(text_muted)
                            .child(count),
                    )
                })
                .into_any_element()
        }

        FlatTreeItem::Loading { stable_key } => {
            div()
                .id(stable_key.clone())
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .h(px(ITEM_HEIGHT))
                .pl(px(32.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(text_muted)
                        .child("Loading collections..."),
                )
                .into_any_element()
        }

        FlatTreeItem::Error { stable_key, db_name } => {
            let db = db_name.to_string();

            div()
                .id(stable_key.clone())
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .w_full()
                .h(px(ITEM_HEIGHT))
                .pl(px(32.0))
                .pr(px(8.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(error_color)
                        .child("Failed to load"),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("retry-{}", db_name)))
                        .cursor_pointer()
                        .text_size(px(10.0))
                        .text_color(accent_color)
                        .hover(|s| s.text_color(text_color))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.retry_load_collections(&db, cx);
                        }))
                        .child("Retry"),
                )
                .into_any_element()
        }

        FlatTreeItem::Empty { stable_key } => {
            div()
                .id(stable_key.clone())
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .h(px(ITEM_HEIGHT))
                .pl(px(32.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(text_muted)
                        .child("No collections"),
                )
                .into_any_element()
        }
    }
}
