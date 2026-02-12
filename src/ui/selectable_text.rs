use std::ops::Range;

use gpui::{
    fill, point, prelude::*, px, rgba, App, Bounds, ClipboardItem, Context, CursorStyle,
    DispatchPhase, ElementId, Entity, FocusHandle, Focusable, GlobalElementId, Hitbox,
    HitboxBehavior, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, ShapedLine, SharedString, Style, TextRun, Window,
};

// Define actions for selectable text
gpui::actions!(selectable_text, [SelectAll, Copy]);

/// Register key bindings for selectable text
pub fn register_selectable_text_bindings(cx: &mut App) {
    cx.bind_keys([
        gpui::KeyBinding::new("ctrl-a", SelectAll, Some("SelectableText")),
        gpui::KeyBinding::new("ctrl-c", Copy, Some("SelectableText")),
    ]);
}

/// Selection mode for different click counts
#[derive(Clone, Debug, Default)]
enum SelectMode {
    #[default]
    Character,
    Word(Range<usize>),
    Line(Range<usize>),
    All,
}

/// Selection state
#[derive(Clone, Debug, Default)]
struct Selection {
    start: usize,
    end: usize,
    reversed: bool,
    pending: bool,
    mode: SelectMode,
}

impl Selection {
    fn is_empty(&self) -> bool {
        self.start == self.end
    }

    fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    fn tail(&self) -> usize {
        if self.reversed {
            self.end
        } else {
            self.start
        }
    }

    fn set_head(&mut self, head: usize, content: &str) {
        match &self.mode {
            SelectMode::Character => {
                if head < self.tail() {
                    if !self.reversed {
                        self.end = self.start;
                        self.reversed = true;
                    }
                    self.start = head;
                } else {
                    if self.reversed {
                        self.start = self.end;
                        self.reversed = false;
                    }
                    self.end = head;
                }
            }
            SelectMode::Word(original_range) | SelectMode::Line(original_range) => {
                let head_range = if matches!(self.mode, SelectMode::Word(_)) {
                    surrounding_word_range(content, head)
                } else {
                    surrounding_line_range(content, head)
                };

                if head < original_range.start {
                    self.start = head_range.start;
                    self.end = original_range.end;
                    self.reversed = true;
                } else if head >= original_range.end {
                    self.start = original_range.start;
                    self.end = head_range.end;
                    self.reversed = false;
                } else {
                    self.start = original_range.start;
                    self.end = original_range.end;
                    self.reversed = false;
                }
            }
            SelectMode::All => {
                self.start = 0;
                self.end = content.len();
                self.reversed = false;
            }
        }
    }
}

/// Get word boundaries around an index
fn surrounding_word_range(text: &str, index: usize) -> Range<usize> {
    let index = index.min(text.len());

    let start = text[..index]
        .char_indices()
        .rev()
        .find(|(_, c)| !c.is_alphanumeric() && *c != '_' && *c != '-')
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);

    let end = text[index..]
        .char_indices()
        .find(|(_, c)| !c.is_alphanumeric() && *c != '_' && *c != '-')
        .map(|(i, _)| index + i)
        .unwrap_or(text.len());

    start..end
}

/// Get line boundaries around an index
fn surrounding_line_range(text: &str, index: usize) -> Range<usize> {
    let index = index.min(text.len());

    let start = text[..index].rfind('\n').map(|i| i + 1).unwrap_or(0);

    let end = text[index..]
        .find('\n')
        .map(|i| index + i)
        .unwrap_or(text.len());

    start..end
}

/// Line layout info
#[derive(Clone)]
struct LineInfo {
    shaped: ShapedLine,
    byte_start: usize,
    byte_end: usize,
    y_offset: Pixels,
}

/// A multi-line read-only text area that supports selection and copy
pub struct SelectableTextArea {
    focus_handle: FocusHandle,
    content: String,
    selection: Selection,
    line_height: Pixels,
    font_size: Pixels,
    text_color: gpui::Hsla,
}

impl SelectableTextArea {
    pub fn new(cx: &mut Context<Self>, content: impl Into<String>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: content.into(),
            selection: Selection::default(),
            line_height: px(18.0),
            font_size: px(12.0),
            text_color: gpui::hsla(0., 0., 0.88, 1.0),
        }
    }

    pub fn set_content(&mut self, content: impl Into<String>, cx: &mut Context<Self>) {
        self.content = content.into();
        self.selection = Selection::default();
        cx.notify();
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.selection = Selection {
            start: 0,
            end: self.content.len(),
            reversed: false,
            pending: false,
            mode: SelectMode::All,
        };
        cx.notify();
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selection.is_empty() {
            let text = self.content[self.selection.range()].to_string();
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }
}

impl Focusable for SelectableTextArea {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SelectableTextArea {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        SelectableTextAreaElement {
            entity: cx.entity().clone(),
        }
    }
}

/// The actual Element that handles rendering and mouse events
struct SelectableTextAreaElement {
    entity: Entity<SelectableTextArea>,
}

impl IntoElement for SelectableTextAreaElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

/// Layout state passed between phases
struct LayoutState {
    lines: Vec<LineInfo>,
    content: String,
    selection: Selection,
    line_height: Pixels,
    #[allow(dead_code)]
    text_color: gpui::Hsla,
}

impl gpui::Element for SelectableTextAreaElement {
    type RequestLayoutState = LayoutState;
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::Name("selectable-text-area".into()))
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let state = self.entity.read(cx);
        let content = state.content.clone();
        let selection = state.selection.clone();
        let line_height = state.line_height;
        let font_size = state.font_size;
        let text_color = state.text_color;

        // Shape all lines
        let style = window.text_style();
        let mut lines = Vec::new();
        let mut byte_offset = 0usize;
        let mut total_height = px(0.0);
        let mut max_width = px(0.0);

        for line_text in content.lines() {
            let line_len = line_text.len();
            let display_text: SharedString = if line_text.is_empty() {
                " ".into()
            } else {
                line_text.to_string().into()
            };

            let run = TextRun {
                len: display_text.len(),
                font: style.font(),
                color: text_color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };

            let shaped = window
                .text_system()
                .shape_line(display_text, font_size, &[run], None);

            // Track maximum line width
            max_width = max_width.max(shaped.width);

            lines.push(LineInfo {
                shaped,
                byte_start: byte_offset,
                byte_end: byte_offset + line_len,
                y_offset: total_height,
            });

            total_height += line_height;
            byte_offset += line_len + 1; // +1 for newline
        }

        // If content is empty, still have minimum height
        if lines.is_empty() {
            total_height = line_height;
        }

        let mut layout_style = Style::default();
        // Use content width so the entire text is selectable without scrolling
        layout_style.size.width = max_width.into();
        layout_style.size.height = total_height.into();

        let layout_id = window.request_layout(layout_style, [], cx);

        (
            layout_id,
            LayoutState {
                lines,
                content,
                selection,
                line_height,
                text_color,
            },
        )
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // Set up focus handle
        let focus_handle = self.entity.read(cx).focus_handle.clone();
        window.set_focus_handle(&focus_handle, cx);

        // Create hitbox for mouse interaction - THIS IS CRITICAL
        

        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Set key context for actions
        let mut key_context = gpui::KeyContext::default();
        key_context.add("SelectableText");
        window.set_key_context(key_context);

        // Register actions
        window.on_action(std::any::TypeId::of::<SelectAll>(), {
            let entity = self.entity.clone();
            move |action, phase, window, cx| {
                if phase == DispatchPhase::Bubble {
                    entity.update(cx, |this, cx| {
                        this.select_all(action.downcast_ref().unwrap(), window, cx);
                    });
                }
            }
        });

        window.on_action(std::any::TypeId::of::<Copy>(), {
            let entity = self.entity.clone();
            move |action, phase, window, cx| {
                if phase == DispatchPhase::Bubble {
                    entity.update(cx, |this, cx| {
                        this.copy(action.downcast_ref().unwrap(), window, cx);
                    });
                }
            }
        });

        // Set cursor style - uses hitbox!
        window.set_cursor_style(CursorStyle::IBeam, hitbox);

        // Paint selection highlight
        self.paint_selection(bounds, layout, window);

        // Paint text
        for line_info in &layout.lines {
            let text_origin = point(bounds.left(), bounds.top() + line_info.y_offset);
            line_info
                .shaped
                .paint(text_origin, layout.line_height, window, cx)
                .ok();
        }

        // Set up mouse event handlers
        self.paint_mouse_listeners(bounds, hitbox, layout, window, cx);
    }
}

impl SelectableTextAreaElement {
    fn paint_selection(&self, bounds: Bounds<Pixels>, layout: &LayoutState, window: &mut Window) {
        if layout.selection.is_empty() {
            return;
        }

        let selection_color = rgba(0x0078d450);
        let selection_range = layout.selection.range();

        for line_info in &layout.lines {
            // Check if selection overlaps this line
            if selection_range.start >= line_info.byte_end
                || selection_range.end <= line_info.byte_start
            {
                continue;
            }

            let sel_start_in_line = selection_range.start.saturating_sub(line_info.byte_start);
            let sel_end_in_line = (selection_range.end - line_info.byte_start)
                .min(line_info.byte_end - line_info.byte_start);

            let start_x = if sel_start_in_line == 0 {
                px(0.0)
            } else {
                line_info.shaped.x_for_index(sel_start_in_line)
            };

            let end_x = line_info.shaped.x_for_index(sel_end_in_line);

            let selection_rect = Bounds::from_corners(
                point(bounds.left() + start_x, bounds.top() + line_info.y_offset),
                point(
                    bounds.left() + end_x,
                    bounds.top() + line_info.y_offset + layout.line_height,
                ),
            );

            window.paint_quad(fill(selection_rect, selection_color));
        }
    }

    fn paint_mouse_listeners(
        &self,
        bounds: Bounds<Pixels>,
        hitbox: &Hitbox,
        layout: &LayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let entity = self.entity.clone();
        let content = layout.content.clone();
        let lines = layout.lines.clone();
        let line_height = layout.line_height;

        // Mouse down - start selection
        window.on_mouse_event({
            let hitbox = hitbox.clone();
            let entity = entity.clone();
            let content = content.clone();
            let lines = lines.clone();
            move |event: &MouseDownEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble
                    && event.button == MouseButton::Left
                    && hitbox.is_hovered(window)
                {
                    let source_index =
                        index_for_position(event.position, bounds, &lines, line_height);

                    let (range, mode) = match event.click_count {
                        1 => (source_index..source_index, SelectMode::Character),
                        2 => {
                            let r = surrounding_word_range(&content, source_index);
                            (r.clone(), SelectMode::Word(r))
                        }
                        3 => {
                            let r = surrounding_line_range(&content, source_index);
                            (r.clone(), SelectMode::Line(r))
                        }
                        _ => {
                            let r = 0..content.len();
                            (r, SelectMode::All)
                        }
                    };

                    entity.update(cx, |this, _cx| {
                        this.selection = Selection {
                            start: range.start,
                            end: range.end,
                            reversed: false,
                            pending: true,
                            mode,
                        };
                    });

                    // Focus using window
                    let focus_handle = entity.read(cx).focus_handle.clone();
                    window.focus(&focus_handle);
                    cx.notify(entity.entity_id());

                    window.prevent_default();
                }
            }
        });

        // Mouse move - update selection while dragging
        window.on_mouse_event({
            let entity = entity.clone();
            let content = content.clone();
            let lines = lines.clone();
            move |event: &MouseMoveEvent, phase, _window, cx| {
                if phase == DispatchPhase::Bubble {
                    entity.update(cx, |this, cx| {
                        if this.selection.pending {
                            let source_index =
                                index_for_position(event.position, bounds, &lines, line_height);
                            this.selection.set_head(source_index, &content);
                            cx.notify();
                        }
                    });
                }
            }
        });

        // Mouse up - finalize selection
        window.on_mouse_event({
            let entity = entity.clone();
            move |_event: &MouseUpEvent, phase, _window, cx| {
                if phase == DispatchPhase::Bubble {
                    entity.update(cx, |this, cx| {
                        if this.selection.pending {
                            this.selection.pending = false;

                            // Copy to clipboard (primary selection on Linux)
                            if !this.selection.is_empty() {
                                let text = this.content[this.selection.range()].to_string();
                                cx.write_to_clipboard(ClipboardItem::new_string(text));
                            }

                            cx.notify();
                        }
                    });
                }
            }
        });
    }
}

/// Convert mouse position to text index
fn index_for_position(
    position: Point<Pixels>,
    bounds: Bounds<Pixels>,
    lines: &[LineInfo],
    line_height: Pixels,
) -> usize {
    let relative_y = position.y - bounds.top();
    let relative_x = position.x - bounds.left();

    // Find which line
    for line_info in lines {
        let line_bottom = line_info.y_offset + line_height;
        if relative_y < line_bottom {
            if relative_x <= px(0.0) {
                return line_info.byte_start;
            }
            let line_index = line_info.shaped.closest_index_for_x(relative_x);
            return (line_info.byte_start + line_index).min(line_info.byte_end);
        }
    }

    // Below all lines
    lines.last().map(|l| l.byte_end).unwrap_or(0)
}
