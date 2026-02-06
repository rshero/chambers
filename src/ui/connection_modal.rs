use gpui::{prelude::*, *};
use std::f32::consts::PI;
use std::sync::Arc;
use std::time::Duration;

use crate::db::{create_connection, Connection, ConnectionConfig, ConnectionStorage, DatabaseType};
use crate::ui::text_input::TextInput;
use crate::ui::title_bar::TitleBar;

// Tab navigation actions for connection modal
gpui::actions!(connection_modal, [FocusNextField, FocusPreviousField]);

/// Register key bindings for connection modal
pub fn register_connection_modal_bindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", FocusNextField, Some("ConnectionModal")),
        KeyBinding::new("shift-tab", FocusPreviousField, Some("ConnectionModal")),
    ]);
}

/// Test connection result state
#[derive(Clone)]
enum TestResult {
    None,
    Testing,
    Success { version: String, latency_ms: u64 },
    Error(String),
}

/// A loading spinner component with rotating dots
/// Uses GPUI's native animation system for smooth 60fps animation
fn render_loading_spinner(id: impl Into<ElementId>) -> impl IntoElement {
    let accent_color = rgb(0x0078d4);
    let dot_count: usize = 3;
    let dot_size = 6.0_f32;
    let spacing = 4.0_f32;

    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(spacing))
        .children((0..dot_count).map(move |i| {
            // Each dot pulses with a phase offset for a wave effect
            let phase_offset = i as f32 * 0.33; // Stagger by 1/3 of the cycle
            
            div()
                .id(("spinner-dot", i))
                .w(px(dot_size))
                .h(px(dot_size))
                .rounded_full()
                .bg(accent_color)
                .with_animation(
                    ("spinner-dot-anim", i),
                    Animation::new(Duration::from_millis(1200)).repeat(),
                    move |el, delta| {
                        // Apply phase offset and create smooth pulsing
                        let adjusted_delta = (delta + phase_offset) % 1.0;
                        // Use sine wave for smooth pulsing (0.3 to 1.0 opacity range)
                        let pulse = ((adjusted_delta * 2.0 * PI).sin() + 1.0) / 2.0;
                        let opacity = 0.3 + (pulse * 0.7);
                        // Also scale slightly for depth effect
                        let scale = 0.7 + (pulse * 0.3);
                        
                        el.opacity(opacity)
                            .w(px(dot_size * scale))
                            .h(px(dot_size * scale))
                    },
                )
        }))
}

/// The connection configuration window
pub struct ConnectionModal {
    title_bar: Entity<TitleBar>,
    storage: Arc<ConnectionStorage>,
    connections: Vec<Connection>,
    selected_connection_index: Option<usize>,
    db_type: DatabaseType,
    test_result: TestResult,
    /// Whether the database type dropdown is currently open
    db_type_dropdown_open: bool,
    // Form fields
    name_input: Entity<TextInput>,
    host_input: Entity<TextInput>,
    port_input: Entity<TextInput>,
    database_input: Entity<TextInput>,
    user_input: Entity<TextInput>,
    password_input: Entity<TextInput>,
    connection_string_input: Entity<TextInput>,
}

impl ConnectionModal {
    pub fn new(
        db_type: DatabaseType,
        storage: Arc<ConnectionStorage>,
        cx: &mut Context<Self>,
    ) -> Self {
        let connections = storage.get_all().unwrap_or_default();
        let conn = Connection::new(db_type);

        let title_bar = cx.new(|_| TitleBar::modal("Database Connections"));
        let name_input = cx.new(|cx| TextInput::new(cx, "Connection name", &conn.name));
        let host_input = cx.new(|cx| TextInput::new(cx, "localhost", &conn.host));
        let port_input = cx.new(|cx| TextInput::new(cx, "Port", &conn.port.to_string()));
        let database_input = cx.new(|cx| TextInput::new(cx, "Database name", ""));
        let user_input = cx.new(|cx| TextInput::new(cx, "Username", ""));
        let password_input = cx.new(|cx| TextInput::new(cx, "Password", "").password());
        let placeholder = match db_type {
            DatabaseType::PostgreSQL => "postgresql://user:pass@host:port/db",
            DatabaseType::MongoDB => "mongodb://user:pass@host:port/db",
            DatabaseType::Redis => "redis://user:pass@host:port",
            DatabaseType::MySQL => "mysql://user:pass@host:port/db",
            DatabaseType::SQLite => "/path/to/database.db",
        };
        let connection_string_input =
            cx.new(|cx| TextInput::new(cx, placeholder, ""));

        Self {
            title_bar,
            storage,
            connections,
            selected_connection_index: None,
            db_type,
            test_result: TestResult::None,
            db_type_dropdown_open: false,
            name_input,
            host_input,
            port_input,
            database_input,
            user_input,
            password_input,
            connection_string_input,
        }
    }

    fn select_connection(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected_connection_index = Some(index);
        self.test_result = TestResult::None;
        self.db_type_dropdown_open = false; // Close dropdown when selecting existing connection
        if let Some(conn) = self.connections.get(index) {
            self.db_type = conn.db_type;
            self.name_input
                .update(cx, |input, _| input.set_text(&conn.name));
            self.host_input
                .update(cx, |input, _| input.set_text(&conn.host));
            self.port_input
                .update(cx, |input, _| input.set_text(&conn.port.to_string()));
            self.database_input.update(cx, |input, _| {
                input.set_text(conn.database.as_deref().unwrap_or(""))
            });
            self.user_input.update(cx, |input, _| {
                input.set_text(conn.username.as_deref().unwrap_or(""))
            });
            self.password_input.update(cx, |input, _| {
                input.set_text(conn.password.as_deref().unwrap_or(""))
            });
            self.connection_string_input.update(cx, |input, _| {
                input.set_text(conn.connection_string.as_deref().unwrap_or(""))
            });
        }
        cx.notify();
    }

    /// Select a connection by its ID (used when opening Properties from context menu)
    pub fn select_connection_by_id(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(index) = self.connections.iter().position(|c| c.id == id) {
            self.select_connection(index, cx);
        }
    }

    fn new_connection(&mut self, cx: &mut Context<Self>) {
        self.selected_connection_index = None;
        self.test_result = TestResult::None;
        self.db_type_dropdown_open = false;
        let conn = Connection::new(self.db_type);
        self.name_input
            .update(cx, |input, _| input.set_text(&conn.name));
        self.host_input
            .update(cx, |input, _| input.set_text(&conn.host));
        self.port_input
            .update(cx, |input, _| input.set_text(&conn.port.to_string()));
        self.database_input
            .update(cx, |input, _| input.set_text(""));
        self.user_input.update(cx, |input, _| input.set_text(""));
        self.password_input
            .update(cx, |input, _| input.set_text(""));
        self.connection_string_input
            .update(cx, |input, _| input.set_text(""));
        cx.notify();
    }

    /// Change the database type and reset form fields appropriately.
    /// Only allowed when creating a new connection (not editing existing).
    fn change_db_type(&mut self, new_type: DatabaseType, cx: &mut Context<Self>) {
        // Only allow changing type for new connections
        if self.selected_connection_index.is_some() {
            return;
        }

        self.db_type = new_type;
        self.db_type_dropdown_open = false;
        self.test_result = TestResult::None;

        // Reset form with new database type defaults
        let conn = Connection::new(new_type);
        self.name_input
            .update(cx, |input, _| input.set_text(&conn.name));
        self.host_input
            .update(cx, |input, _| input.set_text(&conn.host));
        self.port_input
            .update(cx, |input, _| input.set_text(&conn.port.to_string()));
        self.database_input
            .update(cx, |input, _| input.set_text(""));
        self.user_input.update(cx, |input, _| input.set_text(""));
        self.password_input
            .update(cx, |input, _| input.set_text(""));

        // Update connection string placeholder
        let placeholder = match new_type {
            DatabaseType::PostgreSQL => "postgresql://user:pass@host:port/db",
            DatabaseType::MongoDB => "mongodb://user:pass@host:port/db",
            DatabaseType::Redis => "redis://user:pass@host:port",
            DatabaseType::MySQL => "mysql://user:pass@host:port/db",
            DatabaseType::SQLite => "/path/to/database.db",
        };
        self.connection_string_input
            .update(cx, |input, _| input.set_placeholder(placeholder));
        self.connection_string_input
            .update(cx, |input, _| input.set_text(""));

        cx.notify();
    }

    /// Toggle the database type dropdown open/closed
    fn toggle_db_type_dropdown(&mut self, cx: &mut Context<Self>) {
        // Only allow toggling for new connections
        if self.selected_connection_index.is_some() {
            return;
        }
        self.db_type_dropdown_open = !self.db_type_dropdown_open;
        cx.notify();
    }

    fn build_connection(&self, cx: &App) -> Connection {
        let name = self.name_input.read(cx).text();
        let host = self.host_input.read(cx).text();
        let port_str = self.port_input.read(cx).text();
        let database = self.database_input.read(cx).text();
        let username = self.user_input.read(cx).text();
        let password = self.password_input.read(cx).text();
        let connection_string = self.connection_string_input.read(cx).text();

        let port: u16 = port_str.parse().unwrap_or(self.db_type.default_port());

        Connection {
            id: self
                .selected_connection_index
                .and_then(|i| self.connections.get(i))
                .map(|c| c.id.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            name,
            db_type: self.db_type,
            host,
            port,
            database: if database.is_empty() {
                None
            } else {
                Some(database)
            },
            username: if username.is_empty() {
                None
            } else {
                Some(username)
            },
            password: if password.is_empty() {
                None
            } else {
                Some(password)
            },
            connection_string: if connection_string.is_empty() {
                None
            } else {
                Some(connection_string)
            },
            // Preserve visible_databases and show_all_databases if editing existing connection
            visible_databases: self
                .selected_connection_index
                .and_then(|i| self.connections.get(i))
                .and_then(|c| c.visible_databases.clone()),
            show_all_databases: self
                .selected_connection_index
                .and_then(|i| self.connections.get(i))
                .and_then(|c| c.show_all_databases),
        }
    }

    fn save_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let connection = self.build_connection(cx);

        if let Err(e) = self.storage.save(&connection) {
            eprintln!("Failed to save connection: {}", e);
        } else {
            // Refresh connections list
            self.connections = self.storage.get_all().unwrap_or_default();
            cx.notify();
            // Close the window after saving
            window.remove_window();
        }
    }

    /// Validate that required connection fields are filled
    fn validate_connection(&self, cx: &App) -> Result<(), String> {
        let host = self.host_input.read(cx).text();
        let connection_string = self.connection_string_input.read(cx).text();
        let database = self.database_input.read(cx).text();
        
        // If connection string is provided, that's sufficient
        if !connection_string.is_empty() {
            return Ok(());
        }
        
        // For SQLite, we need a database path
        if self.db_type == DatabaseType::SQLite {
            if database.is_empty() {
                return Err("Please provide a database file path or connection string".to_string());
            }
            return Ok(());
        }
        
        // For Redis, host is sufficient (no database name needed)
        if self.db_type == DatabaseType::Redis {
            if host.is_empty() {
                return Err("Please provide a host or connection string".to_string());
            }
            return Ok(());
        }
        
        // For PostgreSQL, MySQL, MongoDB - need host AND database
        if host.is_empty() {
            return Err("Please provide a host or connection string".to_string());
        }
        if database.is_empty() {
            return Err("Please provide a database name".to_string());
        }
        
        Ok(())
    }

    fn test_connection(&mut self, cx: &mut Context<Self>) {
        // Don't allow multiple concurrent tests
        if matches!(self.test_result, TestResult::Testing) {
            return;
        }

        // Validate required fields first
        if let Err(msg) = self.validate_connection(cx) {
            self.test_result = TestResult::Error(msg);
            cx.notify();
            return;
        }

        // Check if driver is available
        if !self.db_type.is_available() {
            self.test_result = TestResult::Error(format!(
                "{} driver not compiled. Rebuild with --features {}",
                self.db_type.name(),
                self.db_type.feature_name()
            ));
            cx.notify();
            return;
        }

        self.test_result = TestResult::Testing;
        cx.notify();

        // Build connection config
        let connection = self.build_connection(cx);
        let conn_string = connection.get_connection_string();
        let config = ConnectionConfig::new(self.db_type, conn_string);
        
        // Use a channel to communicate between threads
        let (tx, rx) = std::sync::mpsc::channel();
        
        // Run test in a separate thread with its own tokio runtime
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                match create_connection(config) {
                    Ok(conn) => conn.test_connection().await,
                    Err(e) => Err(e),
                }
            });
            tx.send(result).ok();
        });
        
        // Poll for result using spawn
        cx.spawn(async move |this, cx| {
            loop {
                // Check if result is ready
                match rx.try_recv() {
                    Ok(result) => {
                        this.update(cx, |modal: &mut ConnectionModal, cx| {
                            modal.test_result = match result {
                                Ok(info) => TestResult::Success {
                                    version: info.server_version.unwrap_or_else(|| "Unknown".to_string()),
                                    latency_ms: info.latency_ms,
                                },
                                Err(e) => TestResult::Error(e.to_string()),
                            };
                            cx.notify();
                        }).ok();
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        // Not ready yet, wait a bit
                        cx.background_executor().timer(std::time::Duration::from_millis(50)).await;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        // Thread died
                        this.update(cx, |modal: &mut ConnectionModal, cx| {
                            modal.test_result = TestResult::Error("Connection test failed unexpectedly".to_string());
                            cx.notify();
                        }).ok();
                        break;
                    }
                }
            }
        })
        .detach();
    }

    fn focus_next_field(
        &mut self,
        _: &FocusNextField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let fields = [
            &self.name_input,
            &self.host_input,
            &self.port_input,
            &self.database_input,
            &self.user_input,
            &self.password_input,
            &self.connection_string_input,
        ];

        let current_idx = fields
            .iter()
            .position(|f| f.focus_handle(cx).is_focused(window));
        let next_idx = match current_idx {
            Some(idx) => (idx + 1) % fields.len(),
            None => 0,
        };
        fields[next_idx].focus_handle(cx).focus(window);
    }

    fn focus_previous_field(
        &mut self,
        _: &FocusPreviousField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let fields = [
            &self.name_input,
            &self.host_input,
            &self.port_input,
            &self.database_input,
            &self.user_input,
            &self.password_input,
            &self.connection_string_input,
        ];

        let current_idx = fields
            .iter()
            .position(|f| f.focus_handle(cx).is_focused(window));
        let prev_idx = match current_idx {
            Some(idx) => {
                if idx == 0 {
                    fields.len() - 1
                } else {
                    idx - 1
                }
            }
            None => fields.len() - 1,
        };
        fields[prev_idx].focus_handle(cx).focus(window);
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar_bg = rgb(0x1a1a1a);
        let border_color = rgb(0x2a2a2a);
        let selected_bg = rgb(0x0e4a7a);
        let hover_bg = rgb(0x252525);

        div()
            .id("modal-sidebar")
            .flex()
            .flex_col()
            .w(px(220.0))
            .h_full()
            .bg(sidebar_bg)
            .border_r_1()
            .border_color(border_color)
            // Header
            .child(
                div()
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(border_color)
                    .text_size(px(11.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(0x808080))
                    .child("SAVED CONNECTIONS"),
            )
            // Connections list
            .child(
                div()
                    .id("connections-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .py(px(6.0))
                    .children(self.connections.iter().enumerate().map(|(index, conn)| {
                        let is_selected = self.selected_connection_index == Some(index);
                        let conn_name = conn.name.clone();
                        let db_type = conn.db_type;

                        div()
                            .id(("connection", index))
                            .px(px(12.0))
                            .py(px(8.0))
                            .mx(px(6.0))
                            .rounded_md()
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(10.0))
                            .when(is_selected, |el| el.bg(selected_bg))
                            .hover(|style| style.bg(hover_bg))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_connection(index, cx);
                            }))
                            .child(img(db_type.icon_path()).size(px(16.0)).flex_none())
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(rgb(0xe0e0e0))
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(conn_name),
                            )
                    }))
                    // Show placeholder if no connections
                    .when(self.connections.is_empty(), |el| {
                        el.child(
                            div()
                                .px(px(16.0))
                                .py(px(24.0))
                                .text_size(px(12.0))
                                .text_color(rgb(0x606060))
                                .child("No saved connections"),
                        )
                    }),
            )
            // New Connection button at bottom
            .child(
                div()
                    .id("new-connection-btn")
                    .px(px(12.0))
                    .py(px(10.0))
                    .border_t_1()
                    .border_color(border_color)
                    .child(
                        div()
                            .id("new-conn-inner")
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .px(px(10.0))
                            .py(px(8.0))
                            .rounded_md()
                            .hover(|s| s.bg(hover_bg))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.new_connection(cx);
                            }))
                            .child(
                                svg()
                                    .path("icons/plus.svg")
                                    .size(px(14.0))
                                    .text_color(rgb(0x0078d4)),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(rgb(0x0078d4))
                                    .child("New Connection"),
                            ),
                    ),
            )
    }

    /// Renders the database type selector in the header.
    /// Shows current type with icon, clickable dropdown when creating new connection.
    fn render_db_type_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_new_connection = self.selected_connection_index.is_none();
        let dropdown_open = self.db_type_dropdown_open;
        let current_type = self.db_type;

        // Colors
        let accent_color = rgb(0x0078d4);
        let normal_text = rgb(0xe0e0e0);
        let muted_text = rgb(0x808080);
        
        // Accent when open, normal when closed (following Zed pattern)
        let trigger_text_color = if dropdown_open && is_new_connection { accent_color } else { normal_text };
        let trigger_icon_color = if dropdown_open && is_new_connection { accent_color } else { muted_text };
        
        let dropdown_bg = rgb(0x1f1f1f);
        let dropdown_border = rgb(0x333333);
        let item_hover_bg = rgb(0x2a2a2a);

        // Chevron icon path based on state
        let chevron_icon = if dropdown_open { "icons/chevron-up.svg" } else { "icons/chevron-down.svg" };

        div()
            .relative()
            .child(
                // Main selector button - simpler, cleaner design
                div()
                    .id("db-type-selector")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .when(is_new_connection, |el| {
                        el.cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_db_type_dropdown(cx);
                            }))
                    })
                    // Database icon
                    .child(img(current_type.icon_path()).size(px(20.0)).flex_none())
                    // Database name
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(trigger_text_color)
                            .child(format!("{} Connection", current_type.name())),
                    )
                    // Chevron indicator (only for new connections)
                    .when(is_new_connection, |el| {
                        el.child(
                            svg()
                                .path(chevron_icon)
                                .size(px(14.0))
                                .text_color(trigger_icon_color),
                        )
                    }),
            )
            // Dropdown menu
            .when(dropdown_open && is_new_connection, |el| {
                el.child(
                    deferred(
                        div()
                            .id("db-type-dropdown")
                            .absolute()
                            .top(px(28.0))
                            .left_0()
                            .min_w(px(220.0))
                            .bg(dropdown_bg)
                            .border_1()
                            .border_color(dropdown_border)
                            .rounded(px(6.0))
                            .shadow_lg()
                            .py(px(4.0))
                            .occlude()
                            .children(
                                DatabaseType::all()
                                    .iter()
                                    .map(|db_type| {
                                        let db_type = *db_type;
                                        let is_selected = db_type == current_type;
                                        let selected_bg = rgba(0x0078d430); // accent with transparency

                                        div()
                                            .id(db_type.name())
                                            .px(px(12.0))
                                            .py(px(8.0))
                                            .mx(px(4.0))
                                            .rounded(px(4.0))
                                            .cursor_pointer()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(10.0))
                                            .when(is_selected, |el| el.bg(selected_bg))
                                            .hover(|s| s.bg(item_hover_bg))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.change_db_type(db_type, cx);
                                            }))
                                            .child(img(db_type.icon_path()).size(px(18.0)).flex_none())
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .text_size(px(13.0))
                                                    .text_color(normal_text)
                                                    .child(db_type.name()),
                                            )
                                            .when(is_selected, |el| {
                                                el.child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .text_color(accent_color)
                                                        .child("✓"),
                                                )
                                            })
                                    }),
                            ),
                    )
                    .with_priority(1),
                )
            })
    }

    fn render_form_field(label: &str, input: Entity<TextInput>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(0x909090))
                    .child(label.to_string()),
            )
            .child(input)
    }

    fn render_form_field_with_hint(
        label: &str,
        hint: &str,
        input: Entity<TextInput>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0x909090))
                            .child(label.to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(0x606060))
                            .child(hint.to_string()),
                    ),
            )
            .child(input)
    }

    fn render_test_result(&self) -> impl IntoElement {
        match &self.test_result {
            TestResult::None => div(),
            TestResult::Testing => div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.0))
                .child(render_loading_spinner("connection-test-spinner"))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(0x0078d4))
                        .child("Testing connection..."),
                ),
            TestResult::Success { version, latency_ms } => div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .w(px(8.0))
                                .h(px(8.0))
                                .rounded_full()
                                .bg(rgb(0x4caf50)),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(rgb(0x4caf50))
                                .child("Connection successful"),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(0x808080))
                        .child(format!("{} ({}ms)", version, latency_ms)),
                ),
            TestResult::Error(msg) => div()
                .flex()
                .flex_col()
                .w_full()
                .min_w_0()
                .gap(px(4.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .flex_shrink_0()
                        .child(
                            div()
                                .w(px(8.0))
                                .h(px(8.0))
                                .flex_none()
                                .rounded_full()
                                .bg(rgb(0xf44336)),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(rgb(0xf44336))
                                .child("Connection failed"),
                        ),
                )
                .child(
                    div()
                        .max_h(px(48.0))
                        .overflow_hidden()
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgb(0x808080))
                                .child(msg.clone()),
                        ),
                ),
        }
    }

    fn render_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let content_bg = rgb(0x1a1a1a);
        let border_color = rgb(0x2a2a2a);

        div()
            .id("modal-content")
            .flex_1()
            .h_full()
            .bg(content_bg)
            .flex()
            .flex_col()
            // Content header with database type selector
            .child(
                div()
                    .relative()
                    .px(px(24.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(border_color)
                    .child(self.render_db_type_selector(cx)),
            )
            // Form content
            .child(
                div()
                    .id("form-content")
                    .flex_1()
                    .overflow_y_scroll()
                    .p(px(24.0))
                    .flex()
                    .flex_col()
                    .gap(px(16.0))
                    // Name field
                    .child(Self::render_form_field("Name", self.name_input.clone()))
                    // Connection String field (overrides below)
                    .child(Self::render_form_field_with_hint(
                        "Connection String",
                        "Overrides fields below if set",
                        self.connection_string_input.clone(),
                    ))
                    // Divider
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(12.0))
                            .child(div().flex_1().h(px(1.0)).bg(rgb(0x333333)))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(rgb(0x606060))
                                    .child("OR"),
                            )
                            .child(div().flex_1().h(px(1.0)).bg(rgb(0x333333))),
                    )
                    // Host and Port row
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(16.0))
                            .child(
                                div().flex_1().child(Self::render_form_field(
                                    "Host",
                                    self.host_input.clone(),
                                )),
                            )
                            .child(
                                div().w(px(100.0)).child(Self::render_form_field(
                                    "Port",
                                    self.port_input.clone(),
                                )),
                            ),
                    )
                    // Database field
                    .child(Self::render_form_field(
                        "Database",
                        self.database_input.clone(),
                    ))
                    // Username and Password row
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(16.0))
                            .child(
                                div().flex_1().child(Self::render_form_field(
                                    "User",
                                    self.user_input.clone(),
                                )),
                            )
                            .child(
                                div().flex_1().child(Self::render_form_field(
                                    "Password",
                                    self.password_input.clone(),
                                )),
                            ),
                    ),
            )
            // Footer with buttons
            .child(
                div()
                    .id("modal-footer")
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .gap(px(12.0))
                    .px(px(24.0))
                    .py(px(16.0))
                    .border_t_1()
                    .border_color(border_color)
                    // Test result - constrained width
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .child(self.render_test_result())
                    )
                    // Buttons row
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_shrink_0()
                            .justify_between()
                            .items_center()
                            // Left side - Test Connection
                            .child({
                                let is_testing = matches!(self.test_result, TestResult::Testing);
                                div()
                                    .id("test-connection")
                                    .when(!is_testing, |el| el.cursor_pointer())
                                    .px(px(14.0))
                                    .py(px(8.0))
                                    .rounded_md()
                                    .border_1()
                                    .when(is_testing, |el| {
                                        el.border_color(rgb(0x404040))
                                            .text_color(rgb(0x606060))
                                    })
                                    .when(!is_testing, |el| {
                                        el.border_color(rgb(0x0078d4))
                                            .text_color(rgb(0x0078d4))
                                            .hover(|s| s.bg(rgba(0x0078d420)))
                                    })
                                    .text_size(px(13.0))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.test_connection(cx);
                                    }))
                                    .child(if is_testing { "Testing..." } else { "Test Connection" })
                            })
                            // Right side - buttons
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(px(10.0))
                                    .child(
                                        div()
                                            .id("cancel-btn")
                                            .cursor_pointer()
                                            .px(px(18.0))
                                            .py(px(8.0))
                                            .bg(rgb(0x333333))
                                            .rounded_md()
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(rgb(0xffffff))
                                            .hover(|s| s.bg(rgb(0x404040)))
                                            .on_click(|_, window, _cx| {
                                                window.remove_window();
                                            })
                                            .child("Cancel"),
                                    )
                                    .child(
                                        div()
                                            .id("save-btn")
                                            .cursor_pointer()
                                            .px(px(18.0))
                                            .py(px(8.0))
                                            .bg(rgb(0x0078d4))
                                            .rounded_md()
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(rgb(0xffffff))
                                            .hover(|s| s.bg(rgb(0x1a8cff)))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.save_connection(window, cx);
                                            }))
                                            .child("Save"),
                                    ),
                            ),
                    ),
            )
    }
}

impl Render for ConnectionModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg = rgb(0x1a1a1a);

        div()
            .id("connection-modal")
            .key_context("ConnectionModal")
            .on_action(cx.listener(Self::focus_next_field))
            .on_action(cx.listener(Self::focus_previous_field))
            .size_full()
            .flex()
            .flex_col()
            .bg(bg)
            .font_family("Fira Code")
            // Custom title bar
            .child(self.title_bar.clone())
            // Body with sidebar and content
            .child(
                div()
                    .id("modal-body")
                    .flex_1()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    .child(self.render_sidebar(cx))
                    .child(self.render_content(cx)),
            )
    }
}
