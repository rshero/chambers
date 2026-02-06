use gpui::{prelude::*, *};
use std::collections::HashSet;

use crate::db::DatabaseType;

/// Event emitted when filter selection changes
pub struct FilterChanged(pub HashSet<DatabaseType>);

impl EventEmitter<FilterChanged> for FilterMenu {}
impl EventEmitter<DismissEvent> for FilterMenu {}

/// Menu for filtering connections by database type
pub struct FilterMenu {
    focus_handle: FocusHandle,
    selected_types: HashSet<DatabaseType>,
}

impl FilterMenu {
    pub fn new(initial_filter: HashSet<DatabaseType>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            selected_types: initial_filter,
        }
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn toggle_type(&mut self, db_type: DatabaseType, cx: &mut Context<Self>) {
        if self.selected_types.contains(&db_type) {
            self.selected_types.remove(&db_type);
        } else {
            self.selected_types.insert(db_type);
        }

        cx.emit(FilterChanged(self.selected_types.clone()));
        cx.notify();
    }

    fn clear_all(&mut self, cx: &mut Context<Self>) {
        self.selected_types.clear();
        cx.emit(FilterChanged(self.selected_types.clone()));
        cx.notify();
    }

    fn select_all(&mut self, cx: &mut Context<Self>) {
        for db_type in DatabaseType::all() {
            self.selected_types.insert(*db_type);
        }
        cx.emit(FilterChanged(self.selected_types.clone()));
        cx.notify();
    }
}

impl Focusable for FilterMenu {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FilterMenu {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg = rgb(0x1f1f1f);
        let border_color = rgb(0x333333);
        let hover_bg = rgb(0x2a2a2a);
        let accent_color = rgb(0x0078d4);
        let text_color = rgb(0xe0e0e0);
        let text_muted = rgb(0x808080);

        let all_types = DatabaseType::all();
        let all_selected = all_types.iter().all(|t| self.selected_types.contains(t));
        let none_selected = self.selected_types.is_empty();

        div()
            .id("filter-menu-container")
            .track_focus(&self.focus_handle)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.dismiss(cx);
            }))
            .occlude()
            .bg(bg)
            .border_1()
            .border_color(border_color)
            .rounded(px(4.0))
            .shadow_md()
            .py(px(4.0))
            .w(px(180.0))
            .flex()
            .flex_col()
            // Header
            .child(
                div()
                    .px(px(12.0))
                    .py(px(6.0))
                    .text_size(px(11.0))
                    .text_color(text_muted)
                    .child("Filter by type"),
            )
            // Divider
            .child(div().mx(px(8.0)).my(px(4.0)).h(px(1.0)).bg(border_color))
            // Database type options with checkboxes
            .children(all_types.iter().map(|db_type| {
                let db_type = *db_type;
                let is_selected = self.selected_types.contains(&db_type);

                div()
                    .id(SharedString::from(format!("filter-{}", db_type.name())))
                    .px(px(8.0))
                    .py(px(4.0))
                    .mx(px(4.0))
                    .rounded(px(3.0))
                    .cursor_pointer()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .text_size(px(13.0))
                    .text_color(text_color)
                    .hover(|style| style.bg(hover_bg))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_type(db_type, cx);
                    }))
                    // Checkbox
                    .child(render_checkbox(is_selected, accent_color))
                    // Database icon
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(16.0))
                            .flex_none()
                            .child(img(db_type.icon_path()).size(px(14.0))),
                    )
                    // Database name
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .h(px(16.0))
                            .line_height(px(16.0))
                            .child(db_type.name()),
                    )
            }))
            // Divider
            .child(div().mx(px(8.0)).my(px(4.0)).h(px(1.0)).bg(border_color))
            // Clear / Select All row
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(8.0))
                    .py(px(2.0))
                    .child(
                        div()
                            .id("clear-filter")
                            .cursor_pointer()
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded(px(3.0))
                            .text_size(px(10.0))
                            .text_color(if none_selected {
                                text_muted
                            } else {
                                accent_color
                            })
                            .when(!none_selected, |el| el.hover(|s| s.bg(hover_bg)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.clear_all(cx);
                            }))
                            .child("Clear"),
                    )
                    .child(
                        div()
                            .id("select-all-filter")
                            .cursor_pointer()
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded(px(3.0))
                            .text_size(px(10.0))
                            .text_color(if all_selected {
                                text_muted
                            } else {
                                accent_color
                            })
                            .when(!all_selected, |el| el.hover(|s| s.bg(hover_bg)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.select_all(cx);
                            }))
                            .child("Select All"),
                    ),
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
        .w(px(14.0))
        .h(px(14.0))
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
                .size(px(10.0))
                .text_color(rgb(0xffffff))
                .when(!is_checked, |el| el.invisible()),
        )
}
