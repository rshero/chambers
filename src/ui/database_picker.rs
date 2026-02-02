use gpui::{prelude::*, *};

use crate::ui::text_input::TextInput;

/// Event emitted when database visibility changes
#[derive(Clone)]
pub struct DatabaseVisibilityChanged {
    pub visible_databases: Vec<String>,
    pub show_all: bool,
}

impl EventEmitter<DatabaseVisibilityChanged> for DatabasePicker {}
impl EventEmitter<DismissEvent> for DatabasePicker {}

/// A dropdown picker for selecting which databases to show
pub struct DatabasePicker {
    focus_handle: FocusHandle,
    all_databases: Vec<String>,
    visible_databases: Vec<String>,
    show_all: bool,
    search_input: Entity<TextInput>,
}

impl DatabasePicker {
    pub fn new(
        all_databases: Vec<String>,
        visible_databases: Vec<String>,
        show_all: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_input = cx.new(|cx| TextInput::new(cx, "Search databases...", ""));

        Self {
            focus_handle: cx.focus_handle(),
            all_databases,
            visible_databases,
            show_all,
            search_input,
        }
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn toggle_show_all(&mut self, cx: &mut Context<Self>) {
        self.show_all = !self.show_all;

        cx.emit(DatabaseVisibilityChanged {
            visible_databases: self.visible_databases.clone(),
            show_all: self.show_all,
        });
        cx.notify();
    }

    fn toggle_database(&mut self, db_name: String, cx: &mut Context<Self>) {
        if self.show_all {
            // If show_all was on, turn it off and start with all databases visible
            self.show_all = false;
            self.visible_databases = self.all_databases.clone();
        }

        if let Some(pos) = self.visible_databases.iter().position(|d| d == &db_name) {
            self.visible_databases.remove(pos);
        } else {
            self.visible_databases.push(db_name);
        }

        cx.emit(DatabaseVisibilityChanged {
            visible_databases: self.visible_databases.clone(),
            show_all: self.show_all,
        });
        cx.notify();
    }

    fn is_database_visible(&self, db_name: &str) -> bool {
        if self.show_all {
            true
        } else {
            self.visible_databases.iter().any(|d| d == db_name)
        }
    }

    /// Focus the search input
    pub fn focus_search(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_input.focus_handle(cx).focus(window);
    }

    /// Get filtered databases based on current search query
    fn filtered_databases(&self, cx: &App) -> Vec<String> {
        let query = self.search_input.read(cx).text().to_lowercase();
        if query.is_empty() {
            self.all_databases.clone()
        } else {
            self.all_databases
                .iter()
                .filter(|db| db.to_lowercase().contains(&query))
                .cloned()
                .collect()
        }
    }
}

impl Focusable for DatabasePicker {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DatabasePicker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dropdown_bg = rgb(0x1f1f1f);
        let dropdown_border = rgb(0x333333);
        let text_muted = rgb(0x808080);
        let accent_color = rgb(0x0078d4);

        // Get filtered databases - computed fresh each render
        // This is efficient because TextInput already triggers re-renders when text changes
        let filtered = self.filtered_databases(cx);
        let total_count = self.all_databases.len();
        let filtered_count = filtered.len();
        let has_search_query = !self.search_input.read(cx).text().is_empty();

        div()
            .track_focus(&self.focus_handle)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.dismiss(cx);
            }))
            .bg(dropdown_bg)
            .border_1()
            .border_color(dropdown_border)
            .rounded(px(6.0))
            .shadow_lg()
            .w(px(280.0))
            .max_h(px(400.0))
            .flex()
            .flex_col()
            // Search input at top
            .child(
                div()
                    .px(px(10.0))
                    .py(px(8.0))
                    .border_b_1()
                    .border_color(dropdown_border)
                    .child(self.search_input.clone()),
            )
            // "Show All" option
            .child(
                div()
                    .id("show-all-option")
                    .px(px(10.0))
                    .py(px(6.0))
                    .mx(px(4.0))
                    .mt(px(4.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .hover(|s| s.bg(rgb(0x2a2a2a)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_show_all(cx);
                    }))
                    // Checkbox with tick
                    .child(render_checkbox(self.show_all, accent_color))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xe0e0e0))
                            .child("Show All"),
                    ),
            )
            // Divider
            .child(
                div()
                    .mx(px(10.0))
                    .my(px(4.0))
                    .h(px(1.0))
                    .bg(dropdown_border),
            )
            // Database list - scrollable with fixed height
            .child(
                div()
                    .id("database-list-container")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .h(px(240.0)) // Fixed height for scrollable area
                    .overflow_y_scroll()
                    .children(filtered.iter().map(|db_name| {
                        let is_visible = self.is_database_visible(db_name);
                        let db_name_clone = db_name.clone();

                        div()
                            .id(SharedString::from(format!("db-picker-{}", db_name)))
                            .px(px(10.0))
                            .py(px(6.0))
                            .mx(px(4.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .hover(|s| s.bg(rgb(0x2a2a2a)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_database(db_name_clone.clone(), cx);
                            }))
                            // Checkbox with tick
                            .child(render_checkbox(is_visible, accent_color))
                            // Database name
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xe0e0e0))
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(db_name.clone()),
                            )
                            .into_any_element()
                    })),
            )
            // Footer with count
            .child(
                div()
                    .px(px(10.0))
                    .py(px(6.0))
                    .border_t_1()
                    .border_color(dropdown_border)
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(text_muted)
                            .child(format!("{} of {} databases", filtered_count, total_count)),
                    )
                    .when(has_search_query, |el| {
                        el.child(
                            div()
                                .text_size(px(11.0))
                                .text_color(text_muted)
                                .child(format!("{} filtered", filtered_count)),
                        )
                    }),
            )
    }
}

/// Render a checkbox with tick mark when checked
fn render_checkbox(is_checked: bool, accent_color: Rgba) -> impl IntoElement {
    let checkbox_bg = rgb(0x1f1f1f);
    let border_color = if is_checked {
        accent_color
    } else {
        rgb(0x505050)
    };
    let bg_color = if is_checked {
        accent_color
    } else {
        checkbox_bg
    };

    div()
        .w(px(16.0))
        .h(px(16.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.0))
        .border_1()
        .border_color(border_color)
        .bg(bg_color)
        .child(
            svg()
                .path("icons/check.svg")
                .size(px(12.0))
                .text_color(rgb(0xffffff))
                .when(!is_checked, |el| el.invisible()),
        )
}
