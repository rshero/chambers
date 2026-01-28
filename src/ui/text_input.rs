use gpui::{prelude::*, *};
use std::cell::RefCell;
use std::rc::Rc;

/// A simple text input field
pub struct TextInput {
    focus_handle: FocusHandle,
    text: Rc<RefCell<String>>,
    placeholder: &'static str,
    is_password: bool,
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>, placeholder: &'static str, initial_value: &str) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            text: Rc::new(RefCell::new(initial_value.to_string())),
            placeholder,
            is_password: false,
        }
    }

    pub fn password(mut self) -> Self {
        self.is_password = true;
        self
    }

    #[allow(dead_code)]
    pub fn text(&self) -> String {
        self.text.borrow().clone()
    }

    pub fn set_text(&self, text: &str) {
        *self.text.borrow_mut() = text.to_string();
    }

    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.text.borrow_mut().pop();
                cx.notify();
            }
            key if key.len() == 1
                && !event.keystroke.modifiers.control
                && !event.keystroke.modifiers.alt =>
            {
                // Single character input
                self.text.borrow_mut().push_str(key);
                cx.notify();
            }
            _ => {}
        }
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let text = self.text.borrow().clone();
        let display_text = if self.is_password && !text.is_empty() {
            "*".repeat(text.len())
        } else if text.is_empty() {
            self.placeholder.to_string()
        } else {
            text.clone()
        };

        let is_empty = text.is_empty();
        let is_focused = self.focus_handle.is_focused(window);
        let input_bg = rgb(0x252525);
        let input_border = if is_focused {
            rgb(0x0078d4)
        } else {
            rgb(0x3a3a3a)
        };
        let text_color = if is_empty {
            rgb(0x606060)
        } else {
            rgb(0xe0e0e0)
        };

        div()
            .id("text-input")
            .track_focus(&self.focus_handle)
            .key_context("TextInput")
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                this.handle_key(event, cx);
            }))
            .px(px(12.0))
            .py(px(10.0))
            .w_full()
            .bg(input_bg)
            .border_1()
            .border_color(input_border)
            .rounded_md()
            .cursor_text()
            .on_click(cx.listener(|this, _, window, _cx| {
                this.focus_handle.focus(window);
            }))
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(text_color)
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(display_text)
                    .when(is_focused, |el| {
                        el.child(div().w(px(2.0)).h(px(16.0)).bg(rgb(0x0078d4)).ml(px(1.0)))
                    }),
            )
    }
}
