use gpui::{prelude::*, *};

const TITLE_BAR_HEIGHT: f32 = 34.0;

/// Window control button type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowControlType {
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl WindowControlType {
    fn icon_path(&self) -> &'static str {
        match self {
            WindowControlType::Minimize => "icons/minimize.svg",
            WindowControlType::Maximize => "icons/maximize.svg",
            WindowControlType::Restore => "icons/restore.svg",
            WindowControlType::Close => "icons/close.svg",
        }
    }
}

/// A single window control button (minimize, maximize, close)
#[derive(IntoElement)]
pub struct WindowControlButton {
    control_type: WindowControlType,
    is_close: bool,
}

impl WindowControlButton {
    pub fn new(control_type: WindowControlType) -> Self {
        Self {
            control_type,
            is_close: matches!(control_type, WindowControlType::Close),
        }
    }
}

impl RenderOnce for WindowControlButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let control_type = self.control_type;
        let is_close = self.is_close;

        let id: &'static str = match control_type {
            WindowControlType::Minimize => "window-control-minimize",
            WindowControlType::Maximize => "window-control-maximize",
            WindowControlType::Restore => "window-control-restore",
            WindowControlType::Close => "window-control-close",
        };

        div()
            .id(id)
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .rounded_xl()
            .w(px(20.0))
            .h(px(20.0))
            .hover(|style| {
                if is_close {
                    style.bg(rgb(0xe81123))
                } else {
                    style.bg(rgba(0xffffff33))
                }
            })
            .active(|style| {
                if is_close {
                    style.bg(rgb(0xf1707a))
                } else {
                    style.bg(rgba(0xffffff22))
                }
            })
            .child(
                svg()
                    .path(control_type.icon_path())
                    .size(px(14.0))
                    .text_color(rgb(0xcccccc))
                    .hover(|style| {
                        if is_close {
                            style.text_color(rgb(0xffffff))
                        } else {
                            style.text_color(rgb(0xffffff))
                        }
                    }),
            )
            .on_mouse_move(|_, _, cx| cx.stop_propagation())
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                match control_type {
                    WindowControlType::Minimize => window.minimize_window(),
                    WindowControlType::Maximize | WindowControlType::Restore => {
                        window.zoom_window()
                    }
                    WindowControlType::Close => cx.quit(),
                }
            })
    }
}

/// Window controls container (minimize, maximize/restore, close)
#[derive(IntoElement)]
pub struct WindowControls;

impl WindowControls {
    pub fn new() -> Self {
        Self
    }
}

impl RenderOnce for WindowControls {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let is_maximized = window.is_maximized();

        div()
            .id("window-controls")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .px(px(12.0))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(WindowControlButton::new(WindowControlType::Minimize))
            .child(WindowControlButton::new(if is_maximized {
                WindowControlType::Restore
            } else {
                WindowControlType::Maximize
            }))
            .child(WindowControlButton::new(WindowControlType::Close))
    }
}

/// Menu button on the left side of the title bar
#[derive(IntoElement)]
pub struct MenuButton;

impl MenuButton {
    pub fn new() -> Self {
        Self
    }
}

impl RenderOnce for MenuButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .id("menu-button")
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .w(px(28.0))
            .h(px(24.0))
            .hover(|style| style.bg(rgba(0xffffff22)))
            .active(|style| style.bg(rgba(0xffffff11)))
            .child(
                svg()
                    .path("icons/menu.svg")
                    .size(px(16.0))
                    .text_color(rgb(0xcccccc)),
            )
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
    }
}

/// Project name display
#[derive(IntoElement)]
pub struct ProjectName {
    name: SharedString,
}

impl ProjectName {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self { name: name.into() }
    }
}

impl RenderOnce for ProjectName {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .id("project-name")
            .cursor_pointer()
            .flex()
            .items_center()
            .px(px(8.0))
            .py(px(4.0))
            .rounded_md()
            .hover(|style| style.bg(rgba(0xffffff22)))
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(0xcccccc))
                    .child(self.name),
            )
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
    }
}

/// The main title bar component
pub struct TitleBar {
    should_move: bool,
}

impl TitleBar {
    pub fn new() -> Self {
        Self { should_move: false }
    }

    pub fn height() -> Pixels {
        px(TITLE_BAR_HEIGHT)
    }
}

impl Render for TitleBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title_bar_bg = rgb(0x2d2d2d);

        div()
            .id("title-bar")
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .h(Self::height())
            .bg(title_bar_bg)
            .border_b_1()
            .border_color(rgb(0x3d3d3d))
            // Enable window dragging
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down_out(cx.listener(|this, _, _, _| {
                this.should_move = false;
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.should_move = false;
                }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.should_move = true;
                }),
            )
            .on_mouse_move(cx.listener(|this, _, window, _| {
                if this.should_move {
                    this.should_move = false;
                    window.start_window_move();
                }
            }))
            // Double-click to maximize/restore
            .on_click(|event, window, _| {
                if event.click_count() == 2 {
                    window.zoom_window();
                }
            })
            // Left side: menu button and project name
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .pl(px(8.0))
                    .child(MenuButton::new())
                    .child(ProjectName::new("chambers")),
            )
            // Right side: window controls
            .child(WindowControls::new())
    }
}
