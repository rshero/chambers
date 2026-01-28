use gpui::{prelude::*, *};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::db::{Connection, ConnectionStorage, DatabaseType};
use crate::ui::database_menu::{DatabaseMenu, DatabaseSelected};
use crate::ui::tooltip::Tooltip;

/// Size of the resize handle in pixels
const RESIZE_HANDLE_SIZE: f32 = 6.0;

/// Default sidebar width
const DEFAULT_SIDEBAR_WIDTH: f32 = 240.0;

/// Minimum sidebar width
const MIN_SIDEBAR_WIDTH: f32 = 150.0;

/// Maximum sidebar width
const MAX_SIDEBAR_WIDTH: f32 = 600.0;

/// Event emitted when user wants to add a new connection
pub struct AddConnectionRequested(pub DatabaseType);

impl EventEmitter<AddConnectionRequested> for Sidebar {}

/// Drag payload for sidebar resize
#[derive(Clone)]
pub struct DraggedSidebar;

impl Render for DraggedSidebar {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// A toolbar icon button
#[derive(IntoElement)]
struct ToolbarButton {
    id: &'static str,
    icon_path: &'static str,
    tooltip_text: &'static str,
}

impl ToolbarButton {
    fn new(id: &'static str, icon_path: &'static str, tooltip_text: &'static str) -> Self {
        Self {
            id,
            icon_path,
            tooltip_text,
        }
    }
}

impl RenderOnce for ToolbarButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let icon_color = rgb(0x858585);
        let icon_hover_color = rgb(0xcccccc);
        let bg_hover = rgba(0xffffff11);

        div()
            .id(self.id)
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .w(px(26.0))
            .h(px(22.0))
            .rounded_md()
            .hover(|style| style.bg(bg_hover))
            .active(|style| style.bg(rgba(0xffffff08)))
            .child(
                svg()
                    .path(self.icon_path)
                    .size(px(16.0))
                    .text_color(icon_color)
                    .hover(|style| style.text_color(icon_hover_color)),
            )
            .tooltip(Tooltip::text(self.tooltip_text))
    }
}

/// The resizable sidebar component
pub struct Sidebar {
    width: Pixels,
    database_menu: Rc<RefCell<Option<Entity<DatabaseMenu>>>>,
    storage: Arc<ConnectionStorage>,
    connections: Vec<Connection>,
    expanded_connections: std::collections::HashSet<String>,
}

impl Sidebar {
    pub fn new(storage: Arc<ConnectionStorage>) -> Self {
        let connections = storage.get_all().unwrap_or_default();
        Self {
            width: px(DEFAULT_SIDEBAR_WIDTH),
            database_menu: Rc::new(RefCell::new(None)),
            storage,
            connections,
            expanded_connections: std::collections::HashSet::new(),
        }
    }

    pub fn refresh_connections(&mut self, _cx: &mut Context<Self>) {
        self.connections = self.storage.get_all().unwrap_or_default();
    }

    pub fn set_width(&mut self, width: Pixels) {
        self.width = width.max(px(MIN_SIDEBAR_WIDTH)).min(px(MAX_SIDEBAR_WIDTH));
    }

    pub fn resize(&mut self, new_width: Pixels, cx: &mut Context<Self>) {
        self.set_width(new_width);
        cx.notify();
    }

    fn show_database_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let menu = cx.new(|cx| DatabaseMenu::new(cx));

        // Subscribe to menu events
        cx.subscribe_in(&menu, window, |this, _, event: &DatabaseSelected, _, cx| {
            cx.emit(AddConnectionRequested(event.0));
            *this.database_menu.borrow_mut() = None;
            cx.notify();
        })
        .detach();

        cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, _, cx| {
            *this.database_menu.borrow_mut() = None;
            cx.notify();
        })
        .detach();

        // Focus the menu
        menu.focus_handle(cx).focus(window);

        *self.database_menu.borrow_mut() = Some(menu);
        cx.notify();
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border_color = rgb(0x2a2a2a);
        let icon_color = rgb(0x808080);
        let icon_hover_color = rgb(0xe0e0e0);
        let bg_hover = rgba(0xffffff0f);
        let menu = self.database_menu.borrow().clone();

        div()
            .id("sidebar-toolbar")
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(34.0))
            .px(px(8.0))
            .gap(px(2.0))
            .border_b_1()
            .border_color(border_color)
            // Add connection button with popover - use relative positioning
            .child(
                div()
                    .id("add-connection-wrapper")
                    .relative()
                    .child(
                        div()
                            .id("add-connection")
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(26.0))
                            .h(px(22.0))
                            .rounded_md()
                            .hover(|style| style.bg(bg_hover))
                            .active(|style| style.bg(rgba(0xffffff08)))
                            .child(
                                svg()
                                    .path("icons/plus.svg")
                                    .size(px(16.0))
                                    .text_color(icon_color)
                                    .hover(|style| style.text_color(icon_hover_color)),
                            )
                            .tooltip(Tooltip::text("Add new connection"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_database_menu(window, cx);
                            })),
                    )
                    // Menu positioned below the button
                    .when_some(menu, |el, menu| {
                        el.child(
                            deferred(div().absolute().top(px(26.0)).left_0().child(menu))
                                .with_priority(1),
                        )
                    }),
            )
            .child(ToolbarButton::new(
                "refresh-connections",
                "icons/refresh.svg",
                "Refresh connections",
            ))
            .child(ToolbarButton::new(
                "filter-connections",
                "icons/filter.svg",
                "Filter connections",
            ))
    }

    fn toggle_connection(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.expanded_connections.contains(id) {
            self.expanded_connections.remove(id);
        } else {
            self.expanded_connections.insert(id.to_string());
        }
        cx.notify();
    }

    fn render_connections(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let hover_bg = rgb(0x252525);
        let text_color = rgb(0xe0e0e0);
        let text_muted = rgb(0x808080);

        div()
            .id("connections-list")
            .flex_1()
            .w_full()
            .overflow_y_scroll()
            .py(px(4.0))
            .children(self.connections.iter().map(|conn| {
                let conn_id = conn.id.clone();
                let conn_name = conn.name.clone();
                let db_type = conn.db_type;
                let is_expanded = self.expanded_connections.contains(&conn.id);

                div()
                    .id(SharedString::from(format!("conn-{}", conn_id)))
                    .flex()
                    .flex_col()
                    .w_full()
                    // Connection row
                    .child(
                        div()
                            .id(SharedString::from(format!("conn-row-{}", conn_id)))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .w_full()
                            .px(px(8.0))
                            .py(px(6.0))
                            .cursor_pointer()
                            .rounded(px(4.0))
                            .hover(|s| s.bg(hover_bg))
                            .on_click(cx.listener({
                                let id = conn_id.clone();
                                move |this, _, _, cx| {
                                    this.toggle_connection(&id, cx);
                                }
                            }))
                            // Collapse/expand chevron
                            .child(
                                svg()
                                    .path(if is_expanded {
                                        "icons/chevron-down.svg"
                                    } else {
                                        "icons/chevron-right.svg"
                                    })
                                    .size(px(12.0))
                                    .text_color(text_muted)
                                    .flex_none(),
                            )
                            // Database icon
                            .child(img(db_type.icon_path()).size(px(16.0)).flex_none())
                            // Connection name
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(13.0))
                                    .text_color(text_color)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(conn_name),
                            ),
                    )
                    // Expanded content (placeholder for collections/tables)
                    .when(is_expanded, |el| {
                        el.child(
                            div()
                                .pl(px(36.0))
                                .pr(px(8.0))
                                .py(px(4.0))
                                .text_size(px(12.0))
                                .text_color(text_muted)
                                .child("Connect to browse..."),
                        )
                    })
            }))
            // Empty state
            .when(self.connections.is_empty(), |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .py(px(32.0))
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(text_muted)
                                .child("No connections"),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgb(0x606060))
                                .child("Click + to add one"),
                        ),
                )
            })
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar_bg = rgb(0x1a1a1a);
        let border_color = rgb(0x2a2a2a);
        let handle_hover_color = rgb(0x0078d4);

        div()
            .id("sidebar-container")
            .flex()
            .flex_row()
            .h_full()
            // Sidebar content area
            .child(
                div()
                    .id("sidebar")
                    .flex()
                    .flex_col()
                    .h_full()
                    .w(self.width)
                    .bg(sidebar_bg)
                    .border_r_1()
                    .border_color(border_color)
                    // Toolbar row at top
                    .child(self.render_toolbar(cx))
                    // Connections list
                    .child(self.render_connections(cx)),
            )
            // Resize handle
            .child(
                div()
                    .id("sidebar-resize-handle")
                    .h_full()
                    .w(px(RESIZE_HANDLE_SIZE))
                    .cursor_col_resize()
                    .bg(transparent_black())
                    .hover(|style| style.bg(handle_hover_color))
                    .active(|style| style.bg(handle_hover_color))
                    // Drag handling
                    .on_drag(DraggedSidebar, |_, _, _, cx| {
                        cx.stop_propagation();
                        cx.new(|_| DraggedSidebar)
                    })
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    // Double-click to reset to default width
                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                        if event.click_count() == 2 {
                            this.width = px(DEFAULT_SIDEBAR_WIDTH);
                            cx.notify();
                        }
                    })),
            )
    }
}
