use gpui::{prelude::*, *};
use std::sync::Arc;

use crate::db::{Connection, ConnectionStorage, DatabaseType};
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

/// The connection configuration window
pub struct ConnectionModal {
    title_bar: Entity<TitleBar>,
    storage: Arc<ConnectionStorage>,
    connections: Vec<Connection>,
    selected_connection_index: Option<usize>,
    db_type: DatabaseType,
    // Form fields
    name_input: Entity<TextInput>,
    host_input: Entity<TextInput>,
    port_input: Entity<TextInput>,
    database_input: Entity<TextInput>,
    user_input: Entity<TextInput>,
    password_input: Entity<TextInput>,
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

        Self {
            title_bar,
            storage,
            connections,
            selected_connection_index: None,
            db_type,
            name_input,
            host_input,
            port_input,
            database_input,
            user_input,
            password_input,
        }
    }

    fn select_connection(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected_connection_index = Some(index);
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
        }
        cx.notify();
    }

    fn new_connection(&mut self, cx: &mut Context<Self>) {
        self.selected_connection_index = None;
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
        cx.notify();
    }

    fn save_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name_input.read(cx).text();
        let host = self.host_input.read(cx).text();
        let port_str = self.port_input.read(cx).text();
        let database = self.database_input.read(cx).text();
        let username = self.user_input.read(cx).text();
        let password = self.password_input.read(cx).text();

        let port: u16 = port_str.parse().unwrap_or(self.db_type.default_port());

        let connection = Connection {
            id: uuid::Uuid::new_v4().to_string(),
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
        };

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

    fn render_form_field(label: &str, input: Entity<TextInput>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(0x909090))
                    .child(label.to_string()),
            )
            .child(input)
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
            // Content header with database type
            .child(
                div()
                    .px(px(24.0))
                    .py(px(14.0))
                    .border_b_1()
                    .border_color(border_color)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .child(img(self.db_type.icon_path()).size(px(22.0)).flex_none())
                    .child(
                        div()
                            .text_size(px(15.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xe0e0e0))
                            .child(format!("{} Connection", self.db_type.name())),
                    ),
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
                    .gap(px(18.0))
                    // Name field
                    .child(Self::render_form_field("Name", self.name_input.clone()))
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
                                div().w(px(120.0)).child(Self::render_form_field(
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
                    // Username field
                    .child(Self::render_form_field("User", self.user_input.clone()))
                    // Password field
                    .child(Self::render_form_field(
                        "Password",
                        self.password_input.clone(),
                    )),
            )
            // Footer with buttons
            .child(
                div()
                    .id("modal-footer")
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .px(px(24.0))
                    .py(px(16.0))
                    .border_t_1()
                    .border_color(border_color)
                    // Left side - Test Connection
                    .child(
                        div()
                            .id("test-connection")
                            .cursor_pointer()
                            .text_size(px(13.0))
                            .text_color(rgb(0x0078d4))
                            .hover(|s| s.text_color(rgb(0x1a8cff)))
                            .child("Test Connection"),
                    )
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
