use gpui::{prelude::*, *};

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
            .rounded(px(4.0))
            .shadow_md()
            .py(px(4.0))
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
                            .px(px(8.0))
                            .py(px(5.0))
                            .mx(px(4.0))
                            .rounded(px(3.0))
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .when(is_selected, |el| el.bg(selected_bg))
                            .hover(|style| style.bg(hover_bg))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_database(db_type, cx);
                            }))
                            .child(img(db_type.icon_path()).size(px(16.0)).flex_none())
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xe0e0e0))
                                    .child(db_type.name()),
                            )
                    }),
            )
    }
}
