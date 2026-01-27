use gpui::{prelude::*, *};

/// A simple tooltip component
pub struct Tooltip {
    text: SharedString,
}

impl Tooltip {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }

    /// Creates a tooltip builder function for use with `.tooltip()`
    pub fn text(text: impl Into<SharedString> + Clone + 'static) -> impl Fn(&mut Window, &mut App) -> AnyView {
        let text = text.into();
        move |_, cx| {
            cx.new(|_| Tooltip::new(text.clone())).into()
        }
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(8.0))
            .py(px(4.0))
            .bg(rgb(0x1e1e1e))
            .border_1()
            .border_color(rgb(0x454545))
            .rounded_md()
            .shadow_md()
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(0xcccccc))
                    .child(self.text.clone()),
            )
    }
}
