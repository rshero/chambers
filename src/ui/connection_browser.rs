use gpui::{prelude::*, *};

use crate::db::driver::{CollectionInfo, DatabaseInfo};
use crate::db::{Connection, ConnectionConfig, DatabaseType};
use crate::db::driver::create_connection;
use crate::ui::tooltip::Tooltip;

/// Maximum number of databases to show initially
const MAX_DATABASES_SHOWN: usize = 10;

/// Threshold for using virtual list (uniform_list) instead of regular iteration
const VIRTUAL_LIST_THRESHOLD: usize = 50;

/// Pre-calculated item height for uniform_list (in pixels)
const DATABASE_ITEM_HEIGHT: f32 = 28.0;

/// Pre-computed database item for efficient rendering
/// Stores pre-calculated values to avoid computation during render
#[derive(Clone)]
struct DatabaseRenderItem {
    /// Stable unique key derived from database name hash
    stable_key: SharedString,
    /// Index in the original databases vec
    source_index: usize,
    /// Pre-formatted size string (e.g., "1.2 GB")
    formatted_size: Option<SharedString>,
    /// Database name as SharedString (avoids cloning)
    name: SharedString,
}

/// Event emitted when a database is selected
#[derive(Clone)]
pub struct DatabaseSelected(pub String);

impl EventEmitter<DatabaseSelected> for ConnectionBrowser {}

/// Event emitted when a collection is selected
#[derive(Clone)]
pub struct CollectionSelected(pub String, pub String); // (database_name, collection_name)

impl EventEmitter<CollectionSelected> for ConnectionBrowser {}

/// Loading state for the browser
#[derive(Clone, PartialEq)]
pub enum LoadingState {
    Idle,
    LoadingDatabases,
    Error(String),
}

/// The connection browser component that shows databases and collections
pub struct ConnectionBrowser {
    connection: Connection,
    databases: Vec<DatabaseInfo>,
    collections: std::collections::HashMap<String, Vec<CollectionInfo>>,
    selected_database: Option<String>,
    /// Current loading state
    pub loading_state: LoadingState,
    /// Which databases are currently visible (filtered by user selection)
    visible_databases: Vec<String>,
    /// Whether all databases should be shown (vs just visible_databases)
    show_all_databases: bool,
    /// Pre-computed render items (computed once when databases change)
    /// This avoids re-computing sizes and keys during every render
    render_cache: Vec<DatabaseRenderItem>,
    /// Cached visible indices (recomputed when visibility changes)
    visible_indices_cache: Vec<usize>,
    /// Flag to track if cache needs rebuild
    cache_dirty: bool,
}

impl ConnectionBrowser {
    pub fn new(connection: Connection) -> Self {
        Self {
            connection,
            databases: Vec::new(),
            collections: std::collections::HashMap::new(),
            selected_database: None,
            loading_state: LoadingState::Idle,
            visible_databases: Vec::new(),
            show_all_databases: false,
            render_cache: Vec::new(),
            visible_indices_cache: Vec::new(),
            cache_dirty: true,
        }
    }

    /// Get the list of all database names
    pub fn database_names(&self) -> Vec<String> {
        self.databases.iter().map(|db| db.name.clone()).collect()
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

    /// Get the list of visible database names
    pub fn visible_databases(&self) -> Vec<String> {
        self.visible_databases.clone()
    }

    /// Set which databases are visible (for filtering)
    /// Batches the update - only calls notify once
    pub fn set_visible_databases(&mut self, databases: Vec<String>, cx: &mut Context<Self>) {
        self.visible_databases = databases;
        self.show_all_databases = false;
        self.cache_dirty = true; // Mark cache as needing rebuild
        cx.notify(); // Single notification after all state changes
    }

    /// Set show_all state
    pub fn set_show_all(&mut self, show_all: bool, cx: &mut Context<Self>) {
        self.show_all_databases = show_all;
        self.cache_dirty = true;
        cx.notify();
    }

    /// Check if currently showing all databases
    pub fn is_showing_all(&self) -> bool {
        self.show_all_databases
    }

    /// Rebuild render cache from current databases
    /// Called lazily when cache_dirty is true
    fn rebuild_render_cache(&mut self) {
        self.render_cache = self
            .databases
            .iter()
            .enumerate()
            .map(|(i, db)| DatabaseRenderItem {
                stable_key: SharedString::from(format!("db-{:016x}", Self::hash_name(&db.name))),
                source_index: i,
                formatted_size: db.size_bytes.map(|s| SharedString::from(Self::format_bytes(s))),
                name: SharedString::from(db.name.clone()),
            })
            .collect();

        self.rebuild_visible_indices_cache();
    }

    /// Rebuild visible indices cache based on current filter
    fn rebuild_visible_indices_cache(&mut self) {
        self.visible_indices_cache = if self.show_all_databases {
            (0..self.render_cache.len()).collect()
        } else {
            self.render_cache
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    self.visible_databases
                        .iter()
                        .any(|v| v.as_str() == item.name.as_ref())
                })
                .map(|(i, _)| i)
                .collect()
        };
        self.cache_dirty = false;
    }

    /// Simple hash function for stable keys
    fn hash_name(name: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        name.hash(&mut hasher);
        hasher.finish()
    }

    pub fn load_databases(&mut self, cx: &mut Context<Self>) {
        if self.connection.db_type != DatabaseType::MongoDB {
            return;
        }

        self.loading_state = LoadingState::LoadingDatabases;
        // Don't notify yet - wait for all state changes

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
                                    // Batch all state updates together
                                    browser.visible_databases = databases
                                        .iter()
                                        .take(MAX_DATABASES_SHOWN)
                                        .map(|db| db.name.clone())
                                        .collect();
                                    browser.databases = databases;
                                    browser.loading_state = LoadingState::Idle;
                                    browser.cache_dirty = true;
                                    // Rebuild cache immediately for better first render
                                    browser.rebuild_render_cache();
                                }
                                Err(e) => {
                                    browser.loading_state = LoadingState::Error(e.to_string());
                                }
                            }
                            // Single notify after all state changes (batched update)
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
                            cx.notify();
                        }).ok();
                        break;
                    }
                }
            }
        }).detach();
        
        // Single notify for loading state change
        cx.notify();
    }

    fn format_bytes(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;
        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }
        format!("{:.1} {}", size, UNITS[unit_index])
    }

    /// Render a loading spinner
    fn render_loading_spinner() -> impl IntoElement {
        let accent_color = rgb(0x0078d4);
        let dot_count: usize = 3;
        let dot_size = 4.0_f32;
        let spacing = 3.0_f32;

        div()
            .id("loading-spinner")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(spacing))
            .children((0..dot_count).map(move |i| {
                let phase_offset = i as f32 * 0.33;
                
                div()
                    .id(("spinner-dot", i))
                    .w(px(dot_size))
                    .h(px(dot_size))
                    .rounded_full()
                    .bg(accent_color)
                    .with_animation(
                        ("spinner-dot-anim", i),
                        Animation::new(std::time::Duration::from_millis(1200)).repeat(),
                        move |el, delta| {
                            let adjusted_delta = (delta + phase_offset) % 1.0;
                            let pulse = ((adjusted_delta * 2.0 * std::f32::consts::PI).sin() + 1.0) / 2.0;
                            let opacity = 0.3 + (pulse * 0.7);
                            let scale = 0.7 + (pulse * 0.3);
                            
                            el.opacity(opacity)
                                .w(px(dot_size * scale))
                                .h(px(dot_size * scale))
                        },
                    )
            }))
    }

    /// Optimized render using pre-computed cache and stable keys
    fn render_database_item_cached(
        item: &DatabaseRenderItem,
        db: &DatabaseInfo,
        is_selected: bool,
        is_loading_collections: bool,
        collections: &std::collections::HashMap<String, Vec<CollectionInfo>>,
    ) -> impl IntoElement {
        let text_color = rgb(0xe0e0e0);
        let text_muted = rgb(0x808080);
        let selected_bg = rgb(0x0e4a7a);
        let hover_bg = rgb(0x252525);
        let accent_color = rgb(0x0078d4);

        // Use stable key from cache (avoids format! during render)
        let item_id = item.stable_key.clone();
        let db_name = item.name.clone();

        div()
            .id(item_id)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .w_full()
            .h(px(DATABASE_ITEM_HEIGHT)) // Fixed height for virtual list
            .px(px(8.0))
            .py(px(4.0))
            .cursor_pointer()
            .rounded(px(4.0))
            .when(is_selected, |el| el.bg(selected_bg))
            .hover(|s| s.bg(if is_selected { selected_bg } else { hover_bg }))
            // Database icon
            .child(
                svg()
                    .path("icons/database-folder.svg")
                    .size(px(14.0))
                    .text_color(if is_selected { accent_color } else { text_muted })
                    .flex_none(),
            )
            // Database name (uses pre-computed SharedString)
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.0))
                    .text_color(if is_selected { accent_color } else { text_color })
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(db_name.clone()),
            )
            // Database size (uses pre-formatted string from cache)
            .when_some(item.formatted_size.clone(), |el, size| {
                el.child(
                    div()
                        .text_size(px(10.0))
                        .text_color(text_muted)
                        .child(size),
                )
            })
            // Loading indicator or expand icon
            .child(
                if is_loading_collections {
                    Self::render_loading_spinner().into_any_element()
                } else {
                    svg()
                        .path(if is_selected {
                            "icons/chevron-down.svg"
                        } else {
                            "icons/chevron-right.svg"
                        })
                        .size(px(10.0))
                        .text_color(text_muted)
                        .flex_none()
                        .into_any_element()
                }
            )
            // Collections list when selected (rendered inline for simplicity)
            .when(is_selected, |el| {
                el.child(
                    div()
                        .pl(px(22.0))
                        .w_full()
                        .child(Self::render_collections_list_static(&db.name, collections))
                )
            })
    }

    /// Static version for use with uniform_list
    fn render_collections_list_static(
        db_name: &str,
        collections_map: &std::collections::HashMap<String, Vec<CollectionInfo>>,
    ) -> impl IntoElement {
        let text_color = rgb(0xe0e0e0);
        let text_muted = rgb(0x808080);
        let hover_bg = rgb(0x252525);

        let collections = collections_map.get(db_name).cloned().unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .w_full()
            .children(collections.iter().enumerate().map(|(index, coll)| {
                let coll_name = coll.name.clone();
                let doc_count = coll.document_count;

                // Use index-based ID for performance
                let coll_id = SharedString::from(format!("coll-{}", index));

                div()
                    .id(coll_id)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .w_full()
                    .px(px(6.0))
                    .py(px(3.0))
                    .cursor_pointer()
                    .rounded(px(4.0))
                    .hover(|s| s.bg(hover_bg))
                    // Collection icon
                    .child(
                        svg()
                            .path("icons/collection.svg")
                            .size(px(12.0))
                            .text_color(text_muted)
                            .flex_none(),
                    )
                    // Collection name
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.0))
                            .text_color(text_color)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(coll_name),
                    )
                    // Document count
                    .when_some(doc_count, |el, count| {
                        el.child(
                            div()
                                .text_size(px(10.0))
                                .text_color(text_muted)
                                .child(format!("{}", count)),
                        )
                    })
            }))
            .when(collections.is_empty(), |el| {
                el.child(
                    div()
                        .px(px(6.0))
                        .py(px(3.0))
                        .text_size(px(11.0))
                        .text_color(text_muted)
                        .child("No collections"),
                )
            })
    }
}

impl Render for ConnectionBrowser {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let text_muted = rgb(0x808080);

        // Rebuild cache if dirty (lazy rebuild on state change)
        if self.cache_dirty && !self.render_cache.is_empty() {
            self.rebuild_visible_indices_cache();
        } else if self.cache_dirty && !self.databases.is_empty() {
            self.rebuild_render_cache();
        }

        let visible_count = self.visible_indices_cache.len();
        let use_virtual_list = visible_count > VIRTUAL_LIST_THRESHOLD;

        // Clone only what we need for rendering
        let selected_database = self.selected_database.clone();
        let collections = self.collections.clone();

        div()
            .id("connection-browser")
            .flex()
            .flex_col()
            .w_full()
            // Use virtual list for large datasets (>50 items)
            .when(use_virtual_list, |el| {
                let visible_indices = self.visible_indices_cache.clone();
                let render_cache = self.render_cache.clone();
                let databases = self.databases.clone();
                let selected_db = selected_database.clone();
                let colls = collections.clone();

                el.child(
                    div()
                        .id("database-virtual-list-container")
                        .h(px((visible_count.min(20) as f32) * DATABASE_ITEM_HEIGHT)) // Show max 20 items height
                        .overflow_hidden()
                        .child(
                            uniform_list(
                                "database-list",
                                visible_count,
                                cx.processor(move |_browser, visible_range: std::ops::Range<usize>, _window, _cx| {
                                    visible_range
                                        .filter_map(|ix| {
                                            let cache_idx = *visible_indices.get(ix)?;
                                            let item = render_cache.get(cache_idx)?;
                                            let db = databases.get(item.source_index)?;
                                            let is_selected = selected_db.as_ref().map(|s| s == &db.name).unwrap_or(false);

                                            Some(
                                                Self::render_database_item_cached(
                                                    item,
                                                    db,
                                                    is_selected,
                                                    false,
                                                    &colls,
                                                )
                                                .into_any_element()
                                            )
                                        })
                                        .collect()
                                }),
                            )
                            .size_full()
                        )
                )
            })
            // Use regular iteration for small datasets (≤50 items) - avoids virtual list overhead
            .when(!use_virtual_list, |el| {
                el.children(self.visible_indices_cache.iter().filter_map(|&cache_idx| {
                    let item = self.render_cache.get(cache_idx)?;
                    let db = self.databases.get(item.source_index)?;
                    let is_selected = selected_database.as_ref().map(|s| s == &db.name).unwrap_or(false);

                    Some(Self::render_database_item_cached(
                        item,
                        db,
                        is_selected,
                        false,
                        &collections,
                    ))
                }))
            })
            // Loading state - shown at top when loading
            .when(matches!(self.loading_state, LoadingState::LoadingDatabases), |el| {
                el.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .px(px(8.0))
                        .py(px(4.0))
                        .child(Self::render_loading_spinner())
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(text_muted)
                                .child("Loading databases..."),
                        ),
                )
            })
            .when(matches!(self.loading_state, LoadingState::Error(_)), |el| {
                el.child(
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
                                .text_color(rgb(0xf44336))
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
                                        .text_color(rgb(0x808080))
                                        .hover(|s| s.text_color(rgb(0xe0e0e0))),
                                ),
                        ),
                )
            })
            .when(self.databases.is_empty() && matches!(self.loading_state, LoadingState::Idle), |el| {
                el.child(
                    div()
                        .px(px(8.0))
                        .py(px(4.0))
                        .text_size(px(11.0))
                        .text_color(text_muted)
                        .child("Click to load databases"),
                )
            })
    }
}
