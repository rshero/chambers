use gpui::{prelude::*, *};

use crate::ui::text_input::TextInput;

/// Maximum number of items to show in picker (like Zed's file finder limit)
const MAX_PICKER_ITEMS: usize = 100;

/// Pre-calculated item height for uniform_list
const PICKER_ITEM_HEIGHT: f32 = 32.0;

/// Pre-computed picker item for efficient rendering
#[derive(Clone)]
struct PickerRenderItem {
    /// Stable key derived from database name hash
    stable_key: SharedString,
    /// Database name as SharedString (avoids cloning)
    name: SharedString,
}

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
    /// Pre-computed render items (built once at construction)
    render_items: Vec<PickerRenderItem>,
    /// Cached filtered indices (recomputed when query changes)
    filtered_indices: Vec<usize>,
    /// Last search query used for cache
    last_query: String,
}

impl DatabasePicker {
    pub fn new(
        all_databases: Vec<String>,
        visible_databases: Vec<String>,
        show_all: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_input = cx.new(|cx| TextInput::new(cx, "Search databases...", ""));

        // Pre-compute render items with stable keys (done once at construction)
        let render_items: Vec<PickerRenderItem> = all_databases
            .iter()
            .map(|name| PickerRenderItem {
                stable_key: SharedString::from(format!("picker-{:016x}", Self::hash_name(name))),
                name: SharedString::from(name.clone()),
            })
            .collect();

        // Initial filtered list (first MAX_PICKER_ITEMS)
        let filtered_indices: Vec<usize> = (0..render_items.len().min(MAX_PICKER_ITEMS)).collect();

        Self {
            focus_handle: cx.focus_handle(),
            all_databases,
            visible_databases,
            show_all,
            search_input,
            render_items,
            filtered_indices,
            last_query: String::new(),
        }
    }

    /// Simple hash function for stable keys
    fn hash_name(name: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        name.hash(&mut hasher);
        hasher.finish()
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn toggle_show_all(&mut self, cx: &mut Context<Self>) {
        // Update local state immediately
        self.show_all = !self.show_all;

        // When unchecking "Show All", clear all visible databases
        if !self.show_all {
            self.visible_databases.clear();
        }

        // Notify first to ensure checkbox updates visually
        cx.notify();

        // Then emit event for subscribers
        cx.emit(DatabaseVisibilityChanged {
            visible_databases: self.visible_databases.clone(),
            show_all: self.show_all,
        });
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

    /// Focus the search input
    pub fn focus_search(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_input.focus_handle(cx).focus(window);
    }

    /// Update filtered indices if query changed
    fn update_filtered_indices(&mut self, cx: &App) {
        let query = self.search_input.read(cx).text().to_lowercase();

        // Only recompute if query changed
        if query == self.last_query {
            return;
        }

        self.last_query = query.clone();

        if query.is_empty() {
            // No filter - take first MAX_PICKER_ITEMS indices
            self.filtered_indices = (0..self.render_items.len().min(MAX_PICKER_ITEMS)).collect();
        } else {
            // Filter by name and limit results
            self.filtered_indices = self
                .render_items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.name.to_lowercase().contains(&query))
                .take(MAX_PICKER_ITEMS)
                .map(|(i, _)| i)
                .collect();
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

        // Update filtered indices if needed (only recomputes when query changes)
        self.update_filtered_indices(cx);

        let total_count = self.all_databases.len();
        let filtered_count = self.filtered_indices.len();
        let has_search_query = !self.last_query.is_empty();

        // Clone render items for use in uniform_list closure
        let render_items = self.render_items.clone();
        let filtered_indices = self.filtered_indices.clone();
        let visible_databases = self.visible_databases.clone();
        let show_all = self.show_all;

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
            // Database list - using uniform_list for virtualization
            .child(
                div()
                    .id("database-list-container")
                    .h(px(240.0)) // Fixed height required for uniform_list
                    .overflow_hidden() // Required for uniform_list to work
                    .child(
                        uniform_list(
                            "database-picker-list",
                            filtered_count,
                            cx.processor(
                                move |_picker,
                                      visible_range: std::ops::Range<usize>,
                                      _window,
                                      cx| {
                                    // Only render items in the visible range!
                                    visible_range
                                        .filter_map(|ix| {
                                            let item_idx = *filtered_indices.get(ix)?;
                                            let item = render_items.get(item_idx)?;
                                            let is_visible = if show_all {
                                                true
                                            } else {
                                                visible_databases
                                                    .iter()
                                                    .any(|d| d.as_str() == item.name.as_ref())
                                            };

                                            Some(render_database_item_optimized(
                                                &item.stable_key,
                                                &item.name,
                                                is_visible,
                                                accent_color,
                                                cx.listener({
                                                    let db_name = item.name.to_string();
                                                    move |picker, _, _, cx| {
                                                        picker.toggle_database(db_name.clone(), cx);
                                                    }
                                                }),
                                            ))
                                        })
                                        .collect()
                                },
                            ),
                        )
                        .size_full(),
                    ),
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
                            .child(format!(
                                "{} of {} databases",
                                filtered_count.min(MAX_PICKER_ITEMS),
                                total_count
                            )),
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

/// Render a single database item using pre-computed stable key (optimized)
fn render_database_item_optimized<F>(
    stable_key: &SharedString,
    db_name: &SharedString,
    is_visible: bool,
    accent_color: Rgba,
    on_click: F,
) -> AnyElement
where
    F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
{
    div()
        .id(stable_key.clone())
        .h(px(PICKER_ITEM_HEIGHT)) // Fixed height for virtual list
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
        .on_click(on_click)
        // Checkbox with tick
        .child(render_checkbox(is_visible, accent_color))
        // Database name (uses pre-computed SharedString)
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
