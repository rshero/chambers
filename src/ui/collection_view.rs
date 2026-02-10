use gpui::{prelude::*, *};
use serde_json::Value;
use std::collections::BTreeSet;

use crate::db::driver::{create_connection, ConnectionConfig};
use crate::db::DatabaseType;
use crate::ui::table_view::{CellClicked, Column, PageChangeRequested, Row, TableView, PAGE_SIZE};

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
}

/// Content for the detail panel
#[derive(Clone)]
struct DetailContent {
    column_name: String,
    value: String,
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

        cx.subscribe(&table_view, |this, _, event: &CellClicked, cx| {
            this.on_cell_clicked(event, cx);
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

    /// Handle cell click for detail view
    fn on_cell_clicked(&mut self, event: &CellClicked, cx: &mut Context<Self>) {
        let column_name = self
            .columns
            .get(event.col_index)
            .cloned()
            .unwrap_or_default();

        self.detail_content = Some(DetailContent {
            column_name,
            value: event.value.to_string(),
        });
        cx.notify();
    }

    /// Close detail panel
    fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.detail_content = None;
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg_color = rgb(0x1a1a1a);
        let text_muted = rgb(0x808080);
        let error_color = rgb(0xf44336);
        let accent_color = rgb(0x0078d4);

        match &self.loading_state {
            LoadingState::Loading => {
                // Loading state
                div()
                    .id("collection-view-loading")
                    .flex()
                    .flex_col()
                    .size_full()
                    .bg(bg_color)
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
                                    .text_color(accent_color)
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
                                    .text_color(text_muted)
                                    .child("Loading documents..."),
                            ),
                    )
                    .into_any_element()
            }
            LoadingState::Error(err) => {
                let error_msg = err.clone();
                // Error state
                div()
                    .id("collection-view-error")
                    .flex()
                    .flex_col()
                    .size_full()
                    .bg(bg_color)
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
                                    .text_color(error_color),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(error_color)
                                    .child("Failed to load documents"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(text_muted)
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
                                    .bg(rgb(0x2a2a2a))
                                    .hover(|s| s.bg(rgb(0x3a3a3a)))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.retry(cx);
                                    }))
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(rgb(0xe0e0e0))
                                            .child("Retry"),
                                    ),
                            ),
                    )
                    .into_any_element()
            }
            LoadingState::Loaded => {
                let has_detail = self.detail_content.is_some();
                let detail_content = self.detail_content.clone();

                // Loaded state - show table with optional detail panel
                div()
                    .id("collection-view")
                    .flex()
                    .flex_row()
                    .size_full()
                    .bg(bg_color)
                    // Table view
                    .child(
                        div()
                            .flex_1()
                            .min_w_0() // Critical: allow shrinking below content width for horizontal scroll
                            .h_full()
                            .overflow_hidden()
                            .child(self.table_view.clone()),
                    )
                    // Detail panel (when viewing large content)
                    .when(has_detail, |el| {
                        let content = detail_content.unwrap();
                        el.child(
                            div()
                                .id("detail-panel")
                                .flex()
                                .flex_col()
                                .w(px(400.0))
                                .h_full()
                                .bg(rgb(0x1e1e1e))
                                .border_l_1()
                                .border_color(rgb(0x2a2a2a))
                                // Title bar with "View"
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_between()
                                        .h(px(32.0))
                                        .px(px(12.0))
                                        .bg(rgb(0x252525))
                                        .border_b_1()
                                        .border_color(rgb(0x2a2a2a))
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(rgb(0xe0e0e0))
                                                .child("View"),
                                        )
                                        .child(
                                            div()
                                                .id("close-detail")
                                                .cursor_pointer()
                                                .p(px(4.0))
                                                .rounded(px(3.0))
                                                .hover(|s| s.bg(rgb(0x3a3a3a)))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.close_detail(cx);
                                                }))
                                                .child(
                                                    svg()
                                                        .path("icons/close.svg")
                                                        .size(px(12.0))
                                                        .text_color(rgb(0x808080)),
                                                ),
                                        ),
                                )
                                // Field name label
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .h(px(28.0))
                                        .px(px(12.0))
                                        .bg(rgb(0x1e1e1e))
                                        .border_b_1()
                                        .border_color(rgb(0x2a2a2a))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(rgb(0x808080))
                                                .child("Field: "),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(accent_color)
                                                .child(content.column_name.clone()),
                                        ),
                                )
                                // Content
                                .child(
                                    div()
                                        .id("detail-content-scroll")
                                        .flex_1()
                                        .p(px(12.0))
                                        .overflow_y_scroll()
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .text_color(rgb(0xe0e0e0))
                                                .whitespace_normal()
                                                .child(content.value),
                                        ),
                                ),
                        )
                    })
                    .into_any_element()
            }
        }
    }
}
