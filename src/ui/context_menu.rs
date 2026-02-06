use gpui::{prelude::*, *};

/// Style variant for a context menu item
#[derive(Clone, Copy, PartialEq)]
pub enum ContextMenuItemStyle {
    /// Normal menu item
    Normal,
    /// Danger item (red text, red icon when hovered)
    Danger,
}

/// A single context menu item
#[derive(Clone)]
pub struct ContextMenuItem {
    pub label: SharedString,
    pub icon: Option<&'static str>,
    pub style: ContextMenuItemStyle,
    pub action: ContextMenuAction,
}

/// Action to perform when a context menu item is clicked
#[derive(Clone)]
pub enum ContextMenuAction {
    /// Copy text to clipboard
    Copy(String),
    /// Custom action identified by a string key
    Custom(SharedString),
}

/// Event emitted when a context menu action is selected
#[derive(Clone)]
pub struct ContextMenuEvent {
    pub action: ContextMenuAction,
}

impl EventEmitter<ContextMenuEvent> for ContextMenu {}
impl EventEmitter<DismissEvent> for ContextMenu {}

/// A right-click context menu that positions itself at the mouse cursor.
/// Supports smart positioning to stay within window bounds.
pub struct ContextMenu {
    focus_handle: FocusHandle,
    items: Vec<ContextMenuItem>,
    /// Position in window coordinates where menu should appear
    position: Point<Pixels>,
    /// Window bounds for smart positioning
    window_size: Size<Pixels>,
    hovered_index: Option<usize>,
}

impl ContextMenu {
    pub fn new(
        items: Vec<ContextMenuItem>,
        position: Point<Pixels>,
        window_size: Size<Pixels>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            items,
            position,
            window_size,
            hovered_index: None,
        }
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn select_item(&mut self, index: usize, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(item) = self.items.get(index) {
            match &item.action {
                ContextMenuAction::Copy(text) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                }
                ContextMenuAction::Custom(_) => {}
            }
            cx.emit(ContextMenuEvent {
                action: item.action.clone(),
            });
            self.dismiss(cx);
        }
    }
}

impl Focusable for ContextMenu {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ContextMenu {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg = rgb(0x1f1f1f);
        let border_color = rgb(0x333333);
        let hover_bg = rgb(0x2a2a2a);
        let text_color = rgb(0xe0e0e0);
        let text_muted = rgb(0x808080);
        let danger_color = rgb(0xf44336);
        let danger_hover_bg = rgba(0xf4433615);

        // Estimate menu dimensions for smart positioning
        let menu_width = px(160.0);
        let menu_height = px((self.items.len() as f32) * 32.0 + 8.0); // item height + padding

        // Smart positioning: flip if too close to edges
        let mut x = self.position.x;
        let mut y = self.position.y;

        // If menu would overflow right edge, open to the left
        if x + menu_width > self.window_size.width {
            x = x - menu_width;
        }

        // If menu would overflow bottom edge, open upward
        if y + menu_height > self.window_size.height {
            y = y - menu_height;
        }

        // Ensure menu doesn't go off-screen to the left or top
        if x < px(0.0) {
            x = px(4.0);
        }
        if y < px(0.0) {
            y = px(4.0);
        }

        div()
            .track_focus(&self.focus_handle)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.dismiss(cx);
            }))
            .absolute()
            .left(x)
            .top(y)
            .occlude()
            .min_w(menu_width)
            .bg(bg)
            .border_1()
            .border_color(border_color)
            .rounded(px(6.0))
            .shadow_lg()
            .py(px(4.0))
            .flex()
            .flex_col()
            .children(self.items.iter().enumerate().map(|(index, item)| {
                let is_danger = item.style == ContextMenuItemStyle::Danger;
                let is_hovered = self.hovered_index == Some(index);
                let icon = item.icon;
                let label = item.label.clone();

                let item_text_color = if is_danger { danger_color } else { text_color };
                let item_icon_color = if is_danger {
                    if is_hovered {
                        danger_color
                    } else {
                        text_muted
                    }
                } else {
                    text_muted
                };
                let item_hover_bg = if is_danger { danger_hover_bg } else { hover_bg };
                let item_hover_text = if is_danger { danger_color } else { text_color };
                let item_hover_icon = if is_danger {
                    danger_color
                } else {
                    rgb(0xc0c0c0)
                };

                div()
                    .id(SharedString::from(format!("ctx-menu-item-{}", index)))
                    .px(px(8.0))
                    .py(px(4.0))
                    .mx(px(4.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .text_size(px(13.0))
                    .text_color(item_text_color)
                    .hover(move |s| s.bg(item_hover_bg).text_color(item_hover_text))
                    .on_mouse_move(cx.listener(move |this, _, _, cx| {
                        if this.hovered_index != Some(index) {
                            this.hovered_index = Some(index);
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_item(index, window, cx);
                    }))
                    // Icon — wrapped in fixed-height container for alignment
                    .when_some(icon, move |el, icon_path| {
                        el.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(16.0))
                                .flex_none()
                                .child(
                                    svg()
                                        .path(icon_path)
                                        .size(px(14.0))
                                        .text_color(item_icon_color)
                                        .hover(move |s| s.text_color(item_hover_icon)),
                                ),
                        )
                    })
                    // Label
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .h(px(16.0))
                            .line_height(px(16.0))
                            .child(label),
                    )
            }))
    }
}
