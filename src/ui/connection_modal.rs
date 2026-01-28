use gpui::{prelude::*, *};
use std::sync::Arc;

use crate::db::{Connection, ConnectionStorage, DatabaseType};
use crate::ui::text_input::TextInput;
use crate::ui::title_bar::TitleBar;

/// The connection configuration window
pub struct ConnectionModal {
    title_bar: Entity<TitleBar>,
    #[allow(dead_code)]
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

    fn render_content(&self, _cx: &mut Context<Self>) -> impl IntoElement {
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
                            .child(Self::render_button("Cancel", false))
                            .child(Self::render_button("Save", true)),
                    ),
            )
    }

    fn render_button(label: &'static str, is_primary: bool) -> impl IntoElement {
        let bg = if is_primary {
            rgb(0x0078d4)
        } else {
            rgb(0x333333)
        };
        let hover_bg = if is_primary {
            rgb(0x1a8cff)
        } else {
            rgb(0x404040)
        };

        div()
            .id(label)
            .cursor_pointer()
            .px(px(18.0))
            .py(px(8.0))
            .bg(bg)
            .rounded_md()
            .text_size(px(13.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(0xffffff))
            .hover(|s| s.bg(hover_bg))
            .on_click(move |_, window, _cx| {
                if label == "Cancel" {
                    window.remove_window();
                }
                // TODO: Save logic for "Save" button
            })
            .child(label)
    }
}

impl Render for ConnectionModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg = rgb(0x1a1a1a);

        div()
            .id("connection-modal")
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
