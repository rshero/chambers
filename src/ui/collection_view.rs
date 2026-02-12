use gpui::{prelude::*, *};
use gpui_component::menu::{PopupMenu, PopupMenuItem};
use serde_json::Value;
use std::collections::BTreeSet;

use crate::db::driver::{create_connection, ConnectionConfig};
use crate::db::DatabaseType;
use crate::ui::selectable_text::SelectableTextArea;
use crate::ui::table_view::{
    CellContextMenuRequested, CellDoubleClicked, Column, HeaderContextMenuRequested,
    PageChangeRequested, Row, SortChangeRequested, SortDirection, TableView, ViewDropdownToggled,
    ViewMode, ViewModeChanged, PAGE_SIZE,
};
use crate::ui::theme::AppColors;

/// Loading state change event
#[derive(Clone)]
pub struct LoadingStateChanged(pub bool);

impl EventEmitter<LoadingStateChanged> for CollectionView {}

/// Loading state for the collection view
#[derive(Clone, PartialEq)]
pub enum LoadingState {
    Loading,
    Loaded,
    Error(String),
}

/// Collection view - displays documents from a MongoDB collection
pub struct CollectionView {
    collection_name: String,
    database_name: String,
    connection_string: String,
    loading_state: LoadingState,
    table_view: Entity<TableView>,
    /// Raw documents from query
    documents: Vec<Value>,
    /// Extracted column names (schema)
    columns: Vec<String>,
    /// Total document count in collection
    total_count: usize,
    /// Current page (0-indexed)
    current_page: usize,
    /// Detail panel content (for viewing large values)
    detail_content: Option<DetailContent>,
    /// Selectable text area for detail panel
    detail_text_area: Option<Entity<SelectableTextArea>>,
    /// Current sort field
    sort_field: Option<String>,
    /// Current sort direction
    sort_direction: Option<SortDirection>,
    /// Current filter query
    #[allow(dead_code)]
    filter_query: String,
    /// Context menu using gpui-component's PopupMenu
    context_menu: Option<Entity<PopupMenu>>,
    /// Position where context menu should appear (window coordinates)
    context_menu_position: Option<Point<Pixels>>,
    /// Subscription for context menu dismiss events
    _context_menu_subscription: Option<Subscription>,
    /// Pending header context menu request (deferred from subscribe)
    pending_header_ctx_menu: Option<(String, Point<Pixels>)>,
    /// Pending cell context menu request (deferred from subscribe)
    pending_cell_ctx_menu: Option<PendingCellContextMenu>,
    /// View dropdown open state
    view_dropdown_open: bool,
    /// Current view mode (tracked here for dropdown rendering)
    current_view_mode: ViewMode,
}

/// Pending cell context menu request data
#[derive(Clone)]
#[allow(dead_code)]
struct PendingCellContextMenu {
    row_index: usize,
    col_index: usize,
    col_name: String,
    value: SharedString,
    position: Point<Pixels>,
}

/// Content for the detail panel
#[derive(Clone)]
struct DetailContent {
    column_name: String,
}

impl CollectionView {
    pub fn new(
        collection_name: String,
        database_name: String,
        connection_string: String,
        cx: &mut Context<Self>,
    ) -> Self {
        let table_view = cx.new(|_| TableView::new());

        // Subscribe to table view events
        cx.subscribe(&table_view, |this, _, event: &PageChangeRequested, cx| {
            this.on_page_change(event.page, cx);
        })
        .detach();

        // NOTE: CellClicked subscription removed - panel should only open from context menu "View" action
        // The CellClicked event is still emitted but we don't subscribe to it here

        cx.subscribe(&table_view, |this, _, event: &CellDoubleClicked, cx| {
            this.on_cell_double_clicked(event, cx);
        })
        .detach();

        cx.subscribe(&table_view, |this, _, event: &SortChangeRequested, cx| {
            this.on_sort_change(event, cx);
        })
        .detach();

        cx.subscribe(&table_view, |this, _, event: &ViewModeChanged, cx| {
            this.on_view_mode_change(event.0, cx);
        })
        .detach();

        cx.subscribe(&table_view, |this, _, event: &HeaderContextMenuRequested, cx| {
            // Defer to render() where we have window access
            this.pending_header_ctx_menu = Some((event.col_name.clone(), event.position));
            cx.notify();
        })
        .detach();

        cx.subscribe(&table_view, |this, _, event: &ViewDropdownToggled, cx| {
            this.view_dropdown_open = event.open;
            this.current_view_mode = event.view_mode;
            cx.notify();
        })
        .detach();

        cx.subscribe(&table_view, |this, _, event: &CellContextMenuRequested, cx| {
            // Defer to render() where we have window access
            this.pending_cell_ctx_menu = Some(PendingCellContextMenu {
                row_index: event.row_index,
                col_index: event.col_index,
                col_name: event.col_name.clone(),
                value: event.value.clone(),
                position: event.position,
            });
            cx.notify();
        })
        .detach();

        let mut view = Self {
            collection_name,
            database_name,
            connection_string,
            loading_state: LoadingState::Loading,
            table_view,
            documents: Vec::new(),
            columns: Vec::new(),
            total_count: 0,
            current_page: 0,
            detail_content: None,
            detail_text_area: None,
            sort_field: None,
            sort_direction: None,
            filter_query: String::new(),
            context_menu: None,
            context_menu_position: None,
            _context_menu_subscription: None,
            pending_header_ctx_menu: None,
            pending_cell_ctx_menu: None,
            view_dropdown_open: false,
            current_view_mode: ViewMode::Table,
        };

        // Start loading data
        view.load_documents(cx);

        view
    }

    /// Handle page change
    fn on_page_change(&mut self, page: usize, cx: &mut Context<Self>) {
        self.current_page = page;
        self.load_documents(cx);
    }

    /// Handle cell double-click to copy value
    fn on_cell_double_clicked(&mut self, event: &CellDoubleClicked, cx: &mut Context<Self>) {
        // Copy the value to clipboard
        cx.write_to_clipboard(ClipboardItem::new_string(event.value.to_string()));
        cx.notify();
    }

    /// Handle sort change
    fn on_sort_change(&mut self, event: &SortChangeRequested, cx: &mut Context<Self>) {
        // Empty field means clear sort
        if event.field.is_empty() {
            self.sort_field = None;
            self.sort_direction = None;
        } else {
            self.sort_field = Some(event.field.clone());
            self.sort_direction = Some(event.direction);
        }
        self.current_page = 0; // Reset to first page when sorting changes
        self.load_documents(cx);
    }

    /// Handle view mode change
    fn on_view_mode_change(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        if mode == ViewMode::Json {
            // Generate pretty-printed JSON for the current documents
            let json = serde_json::to_string_pretty(&self.documents)
                .unwrap_or_else(|_| "[]".to_string());
            self.table_view.update(cx, |table, cx| {
                table.set_raw_json(json, cx);
            });
        }
        cx.notify();
    }

    /// Close detail panel
    fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.detail_content = None;
        self.detail_text_area = None;
        cx.notify();
    }

    /// Show header context menu (called from render where window is available)
    fn show_header_context_menu(
        &mut self,
        col_name: String,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Build the menu using gpui-component's PopupMenu
        let col_name_for_asc = col_name.clone();
        let col_name_for_desc = col_name.clone();
        let col_name_for_copy = col_name.clone();
        let table_view = self.table_view.clone();
        let table_view_for_desc = self.table_view.clone();

        let menu = PopupMenu::build(window, cx, move |menu, _window, _cx| {
            menu.item(
                PopupMenuItem::new("Copy")
                    .icon(gpui_component::IconName::Copy)
                    .on_click({
                        let col_name = col_name_for_copy.clone();
                        move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(col_name.clone()));
                        }
                    }),
            )
            .separator()
            .item(
                PopupMenuItem::new("Sort Ascending")
                    .icon(gpui_component::IconName::ChevronUp)
                    .on_click({
                        let col_name = col_name_for_asc.clone();
                        let table_view = table_view.clone();
                        move |_, _, cx| {
                            table_view.update(cx, |table, cx| {
                                table.set_sort(Some(col_name.clone()), Some(SortDirection::Ascending), cx);
                            });
                        }
                    }),
            )
            .item(
                PopupMenuItem::new("Sort Descending")
                    .icon(gpui_component::IconName::ChevronDown)
                    .on_click({
                        let col_name = col_name_for_desc.clone();
                        let table_view = table_view_for_desc.clone();
                        move |_, _, cx| {
                            table_view.update(cx, |table, cx| {
                                table.set_sort(Some(col_name.clone()), Some(SortDirection::Descending), cx);
                            });
                        }
                    }),
            )
        });

        // Subscribe to dismiss events
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.context_menu = None;
            this.context_menu_position = None;
            this._context_menu_subscription = None;
            cx.notify();
        });

        // Focus the menu
        menu.read(cx).focus_handle(cx).focus(window);

        self.context_menu = Some(menu);
        self.context_menu_position = Some(position);
        self._context_menu_subscription = Some(subscription);
        cx.notify();
    }

    /// Show cell context menu (called from render where window is available)
    fn show_cell_context_menu(
        &mut self,
        pending: PendingCellContextMenu,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let row_index = pending.row_index;
        let _col_index = pending.col_index;
        let col_name = pending.col_name.clone();
        let value = pending.value.clone();
        let value_for_copy = pending.value.to_string();
        let position = pending.position;

        // Get the raw JSON value for pretty printing in detail view
        let local_row_idx = row_index % PAGE_SIZE;
        let pretty_value = if let Some(doc) = self.documents.get(local_row_idx) {
            if let Value::Object(map) = doc {
                if let Some(raw_value) = map.get(&col_name) {
                    serde_json::to_string_pretty(raw_value)
                        .unwrap_or_else(|_| value.to_string())
                } else {
                    value.to_string()
                }
            } else {
                value.to_string()
            }
        } else {
            value.to_string()
        };

        let col_name_for_view = col_name.clone();
        let pretty_value_for_view = pretty_value.clone();

        // Get entity for View action callback
        let view_entity = cx.entity().clone();

        let menu = PopupMenu::build(window, cx, move |menu, _window, _cx| {
            menu.item(
                PopupMenuItem::new("View")
                    .icon(gpui_component::IconName::Eye)
                    .on_click({
                        let col_name = col_name_for_view.clone();
                        let pretty_value = pretty_value_for_view.clone();
                        let entity = view_entity.clone();
                        move |_, _, cx| {
                            entity.update(cx, |this, cx| {
                                this.open_detail_panel(col_name.clone(), pretty_value.clone(), cx);
                            });
                        }
                    }),
            )
            .item(
                PopupMenuItem::new("Copy")
                    .icon(gpui_component::IconName::Copy)
                    .on_click({
                        let value = value_for_copy.clone();
                        move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(value.clone()));
                        }
                    }),
            )
        });

        // Subscribe to dismiss events
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.context_menu = None;
            this.context_menu_position = None;
            this._context_menu_subscription = None;
            cx.notify();
        });

        // Focus the menu
        menu.read(cx).focus_handle(cx).focus(window);

        self.context_menu = Some(menu);
        self.context_menu_position = Some(position);
        self._context_menu_subscription = Some(subscription);
        cx.notify();
    }

    /// Open detail panel for a specific cell
    #[allow(dead_code)]
    fn open_detail_for_cell(
        &mut self,
        row_index: usize,
        _col_index: usize,
        col_name: String,
        value: SharedString,
        cx: &mut Context<Self>,
    ) {
        // Get the raw JSON value from documents for pretty printing
        let local_row_idx = row_index % PAGE_SIZE;
        let pretty_value = if let Some(doc) = self.documents.get(local_row_idx) {
            if let Value::Object(map) = doc {
                if let Some(raw_value) = map.get(&col_name) {
                    serde_json::to_string_pretty(raw_value)
                        .unwrap_or_else(|_| value.to_string())
                } else {
                    value.to_string()
                }
            } else {
                value.to_string()
            }
        } else {
            value.to_string()
        };

        // Create or update the selectable text area
        let content = pretty_value.clone();
        if let Some(text_area) = &self.detail_text_area {
            text_area.update(cx, |ta, cx| {
                ta.set_content(content, cx);
            });
        } else {
            self.detail_text_area = Some(cx.new(|cx| SelectableTextArea::new(cx, content)));
        }

        self.detail_content = Some(DetailContent {
            column_name: col_name,
        });
        cx.notify();
    }

    /// Open detail panel directly with pre-computed values (from context menu "View" action)
    fn open_detail_panel(&mut self, col_name: String, pretty_value: String, cx: &mut Context<Self>) {
        // Create or update the selectable text area
        if let Some(text_area) = &self.detail_text_area {
            text_area.update(cx, |ta, cx| {
                ta.set_content(pretty_value, cx);
            });
        } else {
            self.detail_text_area = Some(cx.new(|cx| SelectableTextArea::new(cx, pretty_value)));
        }

        self.detail_content = Some(DetailContent { column_name: col_name });
        cx.notify();
    }

    /// Close view dropdown
    fn close_view_dropdown(&mut self, cx: &mut Context<Self>) {
        self.view_dropdown_open = false;
        cx.notify();
    }

    /// Load documents from the collection
    fn load_documents(&mut self, cx: &mut Context<Self>) {
        self.loading_state = LoadingState::Loading;
        cx.emit(LoadingStateChanged(true));
        cx.notify();

        let conn_string = self.connection_string.clone();
        let db_name = self.database_name.clone();
        let coll_name = self.collection_name.clone();
        let offset = self.current_page * PAGE_SIZE;
        let limit = PAGE_SIZE as u32;

        let config = ConnectionConfig::new(DatabaseType::MongoDB, conn_string);

        // Use channel for async communication
        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                match create_connection(config) {
                    Ok(conn) => {
                        // Get documents for current page
                        let docs = conn
                            .query_documents(&db_name, &coll_name, limit, offset as u32)
                            .await?;

                        // Get total count
                        let count = conn.count_documents(&db_name, &coll_name).await?;

                        Ok((docs, count))
                    }
                    Err(e) => Err(e),
                }
            });
            tx.send(result).ok();
        });

        let current_page = self.current_page;

        cx.spawn(async move |this, cx| {
            loop {
                match rx.try_recv() {
                    Ok(result) => {
                        this.update(cx, |view, cx| {
                            match result {
                                Ok((documents, count)) => {
                                    view.documents = documents;
                                    view.total_count = count;
                                    view.extract_schema();
                                    view.populate_table(current_page, cx);
                                    view.loading_state = LoadingState::Loaded;
                                }
                                Err(e) => {
                                    view.loading_state = LoadingState::Error(e.to_string());
                                }
                            }
                            cx.emit(LoadingStateChanged(false));
                            cx.notify();
                        })
                        .ok();
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(50))
                            .await;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        this.update(cx, |view, cx| {
                            view.loading_state =
                                LoadingState::Error("Failed to load documents".to_string());
                            cx.emit(LoadingStateChanged(false));
                            cx.notify();
                        })
                        .ok();
                        break;
                    }
                }
            }
        })
        .detach();
    }

    /// Extract unique field names from documents to build schema
    fn extract_schema(&mut self) {
        let mut field_names: BTreeSet<String> = BTreeSet::new();

        // _id should always be first
        field_names.insert("_id".to_string());

        for doc in &self.documents {
            if let Value::Object(map) = doc {
                for key in map.keys() {
                    field_names.insert(key.clone());
                }
            }
        }

        // Convert to vec, ensuring _id is first
        self.columns = vec!["_id".to_string()];
        for name in field_names {
            if name != "_id" {
                self.columns.push(name);
            }
        }
    }

    /// Populate the table view with data
    fn populate_table(&self, page: usize, cx: &mut Context<Self>) {
        // Create columns with appropriate widths
        let columns: Vec<Column> = self
            .columns
            .iter()
            .map(|name| {
                let width = if name == "_id" {
                    200.0 // ObjectIds are long
                } else if name == "__v" {
                    60.0 // Version field is short
                } else if name.contains("date") || name.contains("Date") || name.contains("_at") {
                    180.0 // Dates need space
                } else {
                    150.0 // Default
                };
                Column::new(name.clone()).with_width(width)
            })
            .collect();

        // Create rows
        let rows: Vec<Row> = self
            .documents
            .iter()
            .map(|doc| {
                let values: Vec<SharedString> = self
                    .columns
                    .iter()
                    .map(|col_name| {
                        if let Value::Object(map) = doc {
                            map.get(col_name)
                                .map(value_to_display_string)
                                .unwrap_or_else(|| SharedString::from(""))
                        } else {
                            SharedString::from("")
                        }
                    })
                    .collect();
                Row::new(values)
            })
            .collect();

        // Update table view
        let total_count = self.total_count;
        self.table_view.update(cx, |table, cx| {
            table.set_columns(columns, cx);
            table.set_rows(rows, cx);
            table.set_total_items(total_count, cx);
            table.set_page(page, cx);
        });
    }

    /// Retry loading documents
    fn retry(&mut self, cx: &mut Context<Self>) {
        self.current_page = 0;
        self.load_documents(cx);
    }
}

/// Convert a JSON value to a human-readable display string
/// Handles MongoDB Extended JSON format (BSON types serialized to JSON)
fn value_to_display_string(value: &Value) -> SharedString {
    match value {
        Value::Null => SharedString::from("null"),
        Value::Bool(b) => SharedString::from(b.to_string()),
        Value::Number(n) => SharedString::from(n.to_string()),
        Value::String(s) => SharedString::from(s.clone()),
        // For arrays, recursively format elements
        Value::Array(arr) => {
            let formatted: Vec<String> = arr.iter().map(|v| format_bson_value(v)).collect();
            SharedString::from(format!("[{}]", formatted.join(", ")))
        }
        Value::Object(obj) => SharedString::from(format_bson_object(obj)),
    }
}

/// Format a BSON object, handling MongoDB Extended JSON types
fn format_bson_object(obj: &serde_json::Map<String, Value>) -> String {
    // MongoDB ObjectId: {"$oid": "..."}
    if let Some(Value::String(s)) = obj.get("$oid") {
        return s.clone();
    }

    // MongoDB Date: {"$date": "..."} or {"$date": {"$numberLong": "..."}}
    if let Some(date) = obj.get("$date") {
        return format_bson_date(date);
    }

    // MongoDB NumberLong: {"$numberLong": "..."}
    if let Some(Value::String(s)) = obj.get("$numberLong") {
        return s.clone();
    }

    // MongoDB NumberDouble: {"$numberDouble": "..."}
    if let Some(Value::String(s)) = obj.get("$numberDouble") {
        // Handle special values
        if s == "Infinity" || s == "-Infinity" || s == "NaN" {
            return s.clone();
        }
        // Try to format as a clean number
        if let Ok(n) = s.parse::<f64>() {
            return format_number(n);
        }
        return s.clone();
    }

    // MongoDB NumberDecimal: {"$numberDecimal": "..."}
    if let Some(Value::String(s)) = obj.get("$numberDecimal") {
        return s.clone();
    }

    // MongoDB NumberInt: {"$numberInt": "..."}
    if let Some(Value::String(s)) = obj.get("$numberInt") {
        return s.clone();
    }

    // MongoDB Binary: {"$binary": {"base64": "...", "subType": "..."}}
    if let Some(Value::Object(binary)) = obj.get("$binary") {
        let subtype = binary
            .get("subType")
            .and_then(|v| v.as_str())
            .unwrap_or("00");
        let base64 = binary
            .get("base64")
            .and_then(|v| v.as_str())
            .unwrap_or("...");

        // UUID subtype (04)
        if subtype == "04" || subtype == "03" {
            if let Some(uuid) = decode_uuid_from_base64(base64) {
                return uuid;
            }
        }
        return format!("Binary({}, {})", subtype, truncate_str(base64, 20));
    }

    // MongoDB UUID: {"$uuid": "..."}
    if let Some(Value::String(s)) = obj.get("$uuid") {
        return s.clone();
    }

    // MongoDB Timestamp: {"$timestamp": {"t": ..., "i": ...}}
    if let Some(Value::Object(ts)) = obj.get("$timestamp") {
        let t = ts.get("t").and_then(|v| v.as_u64()).unwrap_or(0);
        let i = ts.get("i").and_then(|v| v.as_u64()).unwrap_or(0);
        // Format timestamp as datetime
        if let Some(dt) = chrono::DateTime::from_timestamp(t as i64, 0) {
            return format!("{} (i:{})", dt.format("%Y-%m-%d %H:%M:%S"), i);
        }
        return format!("Timestamp({}, {})", t, i);
    }

    // MongoDB Regex: {"$regularExpression": {"pattern": "...", "options": "..."}}
    if let Some(Value::Object(regex)) = obj.get("$regularExpression") {
        let pattern = regex.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
        let options = regex.get("options").and_then(|v| v.as_str()).unwrap_or("");
        return format!("/{}/{}", pattern, options);
    }

    // MongoDB MinKey/MaxKey
    if obj.get("$minKey").is_some() {
        return "MinKey".to_string();
    }
    if obj.get("$maxKey").is_some() {
        return "MaxKey".to_string();
    }

    // MongoDB Undefined
    if obj.get("$undefined").is_some() {
        return "undefined".to_string();
    }

    // MongoDB DBRef: {"$ref": "...", "$id": ...}
    if let (Some(Value::String(ref_coll)), Some(id)) = (obj.get("$ref"), obj.get("$id")) {
        let id_str = format_bson_value(id);
        return format!("DBRef({}, {})", ref_coll, id_str);
    }

    // MongoDB Code: {"$code": "..."}
    if let Some(Value::String(code)) = obj.get("$code") {
        let scope = obj.get("$scope");
        if scope.is_some() {
            return format!("Code({}, <scope>)", truncate_str(code, 30));
        }
        return format!("Code({})", truncate_str(code, 50));
    }

    // MongoDB Symbol: {"$symbol": "..."}
    if let Some(Value::String(s)) = obj.get("$symbol") {
        return format!("Symbol({})", s);
    }

    // Regular object - format with human-readable values
    let formatted: Vec<String> = obj
        .iter()
        .map(|(k, v)| format!("{}: {}", k, format_bson_value(v)))
        .collect();
    format!("{{{}}}", formatted.join(", "))
}

/// Format a MongoDB date value
fn format_bson_date(date: &Value) -> String {
    match date {
        Value::String(s) => {
            // ISO date string - already human readable
            s.clone()
        }
        Value::Object(date_obj) => {
            // {"$numberLong": "1234567890123"}
            if let Some(Value::String(ms_str)) = date_obj.get("$numberLong") {
                if let Ok(ms) = ms_str.parse::<i64>() {
                    let secs = ms / 1000;
                    let nsecs = ((ms % 1000) * 1_000_000) as u32;
                    if let Some(dt) = chrono::DateTime::from_timestamp(secs, nsecs) {
                        return dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
                    }
                }
            }
            // Fallback
            serde_json::to_string(date_obj).unwrap_or_else(|_| "Invalid Date".to_string())
        }
        Value::Number(n) => {
            // Numeric timestamp (milliseconds)
            if let Some(ms) = n.as_i64() {
                let secs = ms / 1000;
                let nsecs = ((ms % 1000) * 1_000_000) as u32;
                if let Some(dt) = chrono::DateTime::from_timestamp(secs, nsecs) {
                    return dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
                }
            }
            n.to_string()
        }
        _ => "Invalid Date".to_string(),
    }
}

/// Recursively format a BSON value for display
fn format_bson_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("\"{}\"", s),
        Value::Array(arr) => {
            let formatted: Vec<String> = arr.iter().map(|v| format_bson_value(v)).collect();
            format!("[{}]", formatted.join(", "))
        }
        Value::Object(obj) => format_bson_object(obj),
    }
}

/// Format a floating point number, avoiding unnecessary decimals
fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{:.0}", n)
    } else {
        format!("{}", n)
    }
}

/// Decode a UUID from base64
fn decode_uuid_from_base64(base64: &str) -> Option<String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64)
        .ok()?;
    if bytes.len() != 16 {
        return None;
    }
    Some(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    ))
}

/// Truncate a string for display
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}

impl Render for CollectionView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Process pending header context menu (deferred from subscribe which lacks window)
        if let Some((col_name, position)) = self.pending_header_ctx_menu.take() {
            self.show_header_context_menu(col_name, position, window, cx);
        }

        // Process pending cell context menu (deferred from subscribe which lacks window)
        if let Some(pending) = self.pending_cell_ctx_menu.take() {
            self.show_cell_context_menu(pending, window, cx);
        }

        match &self.loading_state {
            LoadingState::Loading => {
                div()
                    .id("collection-view-loading")
                    .flex()
                    .flex_col()
                    .size_full()
                    .bg(AppColors::bg_main())
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(12.0))
                            .child(
                                svg()
                                    .path("icons/refresh.svg")
                                    .size(px(32.0))
                                    .text_color(AppColors::accent())
                                    .with_animation(
                                        "loading-spin",
                                        Animation::new(std::time::Duration::from_millis(1000))
                                            .repeat(),
                                        |el, delta| {
                                            el.with_transformation(Transformation::rotate(
                                                percentage(delta),
                                            ))
                                        },
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(AppColors::text_muted())
                                    .child("Loading documents..."),
                            ),
                    )
                    .into_any_element()
            }
            LoadingState::Error(err) => {
                let error_msg = err.clone();
                div()
                    .id("collection-view-error")
                    .flex()
                    .flex_col()
                    .size_full()
                    .bg(AppColors::bg_main())
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(12.0))
                            .child(
                                svg()
                                    .path("icons/close.svg")
                                    .size(px(32.0))
                                    .text_color(AppColors::error()),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(AppColors::error())
                                    .child("Failed to load documents"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(AppColors::text_muted())
                                    .max_w(px(400.0))
                                    .text_ellipsis()
                                    .child(error_msg),
                            )
                            .child(
                                div()
                                    .id("retry-button")
                                    .cursor_pointer()
                                    .px(px(12.0))
                                    .py(px(6.0))
                                    .mt(px(8.0))
                                    .rounded(px(4.0))
                                    .bg(AppColors::bg_active())
                                    .hover(|s| s.bg(AppColors::bg_hover()))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.retry(cx);
                                    }))
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(AppColors::text())
                                            .child("Retry"),
                                    ),
                            ),
                    )
                    .into_any_element()
            }
            LoadingState::Loaded => {
                let has_detail = self.detail_content.is_some();
                let detail_content = self.detail_content.clone();
                let detail_text_area = self.detail_text_area.clone();
                let context_menu = self.context_menu.clone();
                let view_dropdown_open = self.view_dropdown_open;
                let current_view_mode = self.current_view_mode;
                let table_view = self.table_view.clone();

                div()
                    .id("collection-view")
                    .flex()
                    .flex_row()
                    .size_full()
                    .bg(AppColors::bg_main())
                    .relative()
                    // Table view
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .overflow_hidden()
                            .child(self.table_view.clone()),
                    )
                    // Detail panel
                    .when(has_detail, |el| {
                        let content = detail_content.unwrap();
                        el.child(
                            div()
                                .id("detail-panel")
                                .flex()
                                .flex_col()
                                .w(px(400.0))
                                .h_full()
                                .bg(AppColors::bg_secondary())
                                .border_l_1()
                                .border_color(AppColors::border_subtle())
                                // Title bar
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_between()
                                        .h(px(32.0))
                                        .px(px(12.0))
                                        .bg(AppColors::bg_header())
                                        .border_b_1()
                                        .border_color(AppColors::border_subtle())
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(AppColors::text())
                                                .child("View"),
                                        )
                                        .child(
                                            div()
                                                .id("close-detail")
                                                .cursor_pointer()
                                                .p(px(4.0))
                                                .rounded(px(3.0))
                                                .hover(|s| s.bg(AppColors::bg_hover()))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.close_detail(cx);
                                                }))
                                                .child(
                                                    svg()
                                                        .path("icons/close.svg")
                                                        .size(px(12.0))
                                                        .text_color(AppColors::text_muted()),
                                                ),
                                        ),
                                )
                                // Field name
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .h(px(28.0))
                                        .px(px(12.0))
                                        .bg(AppColors::bg_secondary())
                                        .border_b_1()
                                        .border_color(AppColors::border_subtle())
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(AppColors::text_muted())
                                                .child("Field: "),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(AppColors::accent())
                                                .child(content.column_name.clone()),
                                        ),
                                )
                                // Content — selectable text area
                                .child(
                                    div()
                                        .id("detail-content-scroll")
                                        .flex_1()
                                        .p(px(12.0))
                                        .overflow_y_scroll()
                                        .overflow_x_scroll()
                                        .when_some(detail_text_area, |el, text_area| {
                                            el.child(text_area)
                                        }),
                                ),
                        )
                    })
                    // View dropdown overlay (rendered here, outside overflow_hidden)
                    .when(view_dropdown_open, |el| {
                        el.child(
                            div()
                                .id("view-dropdown-backdrop")
                                .absolute()
                                .top_0()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                    this.close_view_dropdown(cx);
                                }))
                                .child(
                                    div()
                                        .id("view-dropdown-menu")
                                        .absolute()
                                        .top(px(38.0)) // toolbar height + small offset
                                        .right(px(14.0))
                                        .occlude()
                                        .min_w(px(110.0))
                                        .bg(AppColors::menu_bg())
                                        .border_1()
                                        .border_color(AppColors::border())
                                        .rounded(px(4.0))
                                        .shadow_lg()
                                        .py(px(4.0))
                                        .child(
                                            div()
                                                .id("view-table")
                                                .px(px(12.0))
                                                .py(px(6.0))
                                                .cursor_pointer()
                                                .hover(|s| s.bg(AppColors::menu_hover()))
                                                .on_click({
                                                    let table_view = table_view.clone();
                                                    cx.listener(move |this, _, _, cx| {
                                                        table_view.update(cx, |t, cx| {
                                                            t.set_view_mode(ViewMode::Table, cx);
                                                        });
                                                        this.view_dropdown_open = false;
                                                        this.current_view_mode = ViewMode::Table;
                                                        cx.notify();
                                                    })
                                                })
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_row()
                                                        .items_center()
                                                        .gap(px(8.0))
                                                        .child(
                                                            svg()
                                                                .path("icons/table.svg")
                                                                .size(px(14.0))
                                                                .text_color(
                                                                    if current_view_mode
                                                                        == ViewMode::Table
                                                                    {
                                                                        AppColors::accent()
                                                                    } else {
                                                                        AppColors::text_dim()
                                                                    },
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(12.0))
                                                                .text_color(
                                                                    if current_view_mode
                                                                        == ViewMode::Table
                                                                    {
                                                                        AppColors::accent()
                                                                    } else {
                                                                        AppColors::text_secondary()
                                                                    },
                                                                )
                                                                .child("Table"),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .id("view-json")
                                                .px(px(12.0))
                                                .py(px(6.0))
                                                .cursor_pointer()
                                                .hover(|s| s.bg(AppColors::menu_hover()))
                                                .on_click({
                                                    let table_view = table_view.clone();
                                                    cx.listener(move |this, _, _, cx| {
                                                        table_view.update(cx, |t, cx| {
                                                            t.set_view_mode(ViewMode::Json, cx);
                                                        });
                                                        this.view_dropdown_open = false;
                                                        this.current_view_mode = ViewMode::Json;
                                                        cx.notify();
                                                    })
                                                })
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_row()
                                                        .items_center()
                                                        .gap(px(8.0))
                                                        .child(
                                                            svg()
                                                                .path("icons/code.svg")
                                                                .size(px(14.0))
                                                                .text_color(
                                                                    if current_view_mode
                                                                        == ViewMode::Json
                                                                    {
                                                                        AppColors::accent()
                                                                    } else {
                                                                        AppColors::text_dim()
                                                                    },
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(12.0))
                                                                .text_color(
                                                                    if current_view_mode
                                                                        == ViewMode::Json
                                                                    {
                                                                        AppColors::accent()
                                                                    } else {
                                                                        AppColors::text_secondary()
                                                                    },
                                                                )
                                                                .child("JSON"),
                                                        ),
                                                ),
                                        ),
                                ),
                        )
                    })
                    // Context menu overlay (rendered here, outside overflow_hidden)
                    .when_some(context_menu, |el, menu| {
                        if let Some(position) = self.context_menu_position {
                            el.child(
                                deferred(
                                    anchored()
                                        .position(position)
                                        .snap_to_window_with_margin(px(8.0))
                                        .anchor(Corner::TopLeft)
                                        .child(
                                            div()
                                                .occlude()
                                                .child(menu)
                                        )
                                )
                                .with_priority(2)
                            )
                        } else {
                            el.child(deferred(div().occlude().child(menu)).with_priority(2))
                        }
                    })
                    .into_any_element()
            }
        }
    }
}
