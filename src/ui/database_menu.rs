use gpui::{prelude::*, rems, *};

use crate::db::DatabaseType;

/// Event emitted when a database type is selected
pub struct DatabaseSelected(pub DatabaseType);

impl EventEmitter<DatabaseSelected> for DatabaseMenu {}
impl EventEmitter<DismissEvent> for DatabaseMenu {}

/// Menu showing available database types
pub struct DatabaseMenu {
    focus_handle: FocusHandle,
    selected_index: Option<usize>,
}

impl DatabaseMenu {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            selected_index: None,
        }
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn select_database(&mut self, db_type: DatabaseType, cx: &mut Context<Self>) {
        cx.emit(DatabaseSelected(db_type));
        cx.emit(DismissEvent);
    }
}

impl Focusable for DatabaseMenu {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DatabaseMenu {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg = rgb(0x1f1f1f);
        let border_color = rgb(0x333333);
        let hover_bg = rgb(0x2a2a2a);
        let selected_bg = rgb(0x0e4a7a);

        div()
            .track_focus(&self.focus_handle)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.dismiss(cx);
            }))
            .occlude()
            .bg(bg)
            .border_1()
            .border_color(border_color)
            .rounded(px(4.0)) // Keep border radius as px
            .shadow_md()
            .py(rems(0.25)) // 4px
            .flex()
            .flex_col()
            // Database options (no header)
            .children(
                DatabaseType::all()
                    .iter()
                    .enumerate()
                    .map(|(index, db_type)| {
                        let db_type = *db_type;
                        let is_selected = self.selected_index == Some(index);

                        div()
                            .id(db_type.name())
                            .px(rems(0.5)) // 8px
                            .py(rems(0.25)) // 4px
                            .mx(rems(0.25)) // 4px
                            .rounded(px(3.0)) // Keep border radius as px
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(rems(0.375)) // 6px
                            .text_size(rems(0.8125)) // 13px
                            .text_color(rgb(0xe0e0e0))
                            .when(is_selected, |el| el.bg(selected_bg))
                            .hover(|style| style.bg(hover_bg))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_database(db_type, cx);
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size(rems(1.0)) // 16px
                                    .flex_none()
                                    .child(img(db_type.icon_path()).size(rems(1.0))), // 16px
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .h(rems(1.0)) // 16px
                                    .line_height(rems(1.0)) // 16px
                                    .child(db_type.name()),
                            )
                    }),
            )
    }
}
