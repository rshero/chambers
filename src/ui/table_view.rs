use gpui::{prelude::*, *};

use crate::ui::tooltip::Tooltip;

/// Row height for table rows
const ROW_HEIGHT: f32 = 28.0;

/// Header height
const HEADER_HEIGHT: f32 = 32.0;

/// Default column width
const DEFAULT_COLUMN_WIDTH: f32 = 150.0;

/// Row number column width
const ROW_NUM_WIDTH: f32 = 50.0;

/// Maximum display length for cell values (truncate long content)
const MAX_CELL_DISPLAY_LENGTH: usize = 100;

/// Items per page
pub const PAGE_SIZE: usize = 20;

/// Scrollbar thumb minimum height
const SCROLLBAR_THUMB_MIN_SIZE: f32 = 30.0;

/// Scrollbar track height (for horizontal scrollbar)
const SCROLLBAR_TRACK_HEIGHT: f32 = 12.0;

/// A column definition
#[derive(Clone)]
pub struct Column {
    pub name: SharedString,
    pub width: f32,
}

impl Column {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            width: DEFAULT_COLUMN_WIDTH,
        }
    }

    pub fn with_width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

/// A row of data
#[derive(Clone)]
pub struct Row {
    /// Values for each column (in same order as columns)
    pub values: Vec<SharedString>,
    /// Original full values (for detail view)
    pub full_values: Vec<SharedString>,
}

impl Row {
    pub fn new(values: Vec<SharedString>) -> Self {
        // Store full values and create truncated display values
        let display_values: Vec<SharedString> = values
            .iter()
            .map(|v| {
                if v.len() > MAX_CELL_DISPLAY_LENGTH {
                    SharedString::from(format!("{}...", &v[..MAX_CELL_DISPLAY_LENGTH]))
                } else {
                    v.clone()
                }
            })
            .collect();

        Self {
            values: display_values,
            full_values: values,
        }
    }
}

/// Event emitted when a row is selected
#[derive(Clone)]
#[allow(dead_code)] // Event payload for subscribers
pub struct RowSelected(pub usize);

/// Event emitted when a cell is clicked for detail view
#[derive(Clone)]
pub struct CellClicked {
    #[allow(dead_code)] // Field for future use
    pub row_index: usize,
    pub col_index: usize,
    pub value: SharedString,
}

impl EventEmitter<RowSelected> for TableView {}
impl EventEmitter<CellClicked> for TableView {}

/// Event for page change requests
#[derive(Clone)]
pub struct PageChangeRequested {
    pub page: usize,
}

impl EventEmitter<PageChangeRequested> for TableView {}

/// DataGrip-style table view component with pagination and horizontal scrolling
pub struct TableView {
    columns: Vec<Column>,
    rows: Vec<Row>,
    selected_row: Option<usize>,
    /// Vertical scroll handle
    vertical_scroll: ScrollHandle,
    /// Horizontal scroll offset (positive = scrolled right)
    horizontal_scroll_offset: Pixels,
    /// Captured viewport width from layout
    viewport_width: Pixels,
    /// Whether scrollbar thumb is being dragged
    is_dragging_scrollbar: bool,
    /// Drag start position (click_x, initial_scroll_offset)
    drag_start: Option<(Pixels, Pixels)>,
    /// Current page (0-indexed)
    current_page: usize,
    /// Total number of items (may be more than rows.len() if paginated at source)
    total_items: usize,
}

impl TableView {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            selected_row: None,
            vertical_scroll: ScrollHandle::new(),
            horizontal_scroll_offset: px(0.0),
            viewport_width: px(0.0),
            is_dragging_scrollbar: false,
            drag_start: None,
            current_page: 0,
            total_items: 0,
        }
    }

    /// Set the columns for this table
    pub fn set_columns(&mut self, columns: Vec<Column>, cx: &mut Context<Self>) {
        self.columns = columns;
        // Reset horizontal scroll when columns change
        self.horizontal_scroll_offset = px(0.0);
        cx.notify();
    }

    /// Set the data rows (doesn't reset pagination - use set_rows_with_pagination for that)
    pub fn set_rows(&mut self, rows: Vec<Row>, cx: &mut Context<Self>) {
        self.rows = rows;
        self.selected_row = None;
        cx.notify();
    }

    /// Set total items count (for server-side pagination)
    pub fn set_total_items(&mut self, total: usize, cx: &mut Context<Self>) {
        self.total_items = total;
        cx.notify();
    }

    /// Set current page
    pub fn set_page(&mut self, page: usize, cx: &mut Context<Self>) {
        self.current_page = page;
        cx.notify();
    }

    /// Reset pagination (call when loading fresh data)
    #[allow(dead_code)] // API method for fresh data loads
    pub fn reset_pagination(&mut self, cx: &mut Context<Self>) {
        self.current_page = 0;
        self.total_items = self.rows.len();
        cx.notify();
    }

    /// Get current page
    #[allow(dead_code)] // API method for external access
    pub fn current_page(&self) -> usize {
        self.current_page
    }

    /// Get total pages
    pub fn total_pages(&self) -> usize {
        if self.total_items == 0 {
            1
        } else {
            self.total_items.div_ceil(PAGE_SIZE)
        }
    }

    /// Select a row by index
    pub fn select_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.rows.len() {
            self.selected_row = Some(index);
            cx.emit(RowSelected(index));
            cx.notify();
        }
    }

    /// Get selected row index
    #[allow(dead_code)] // API method for external access
    pub fn selected_row(&self) -> Option<usize> {
        self.selected_row
    }

    /// Go to next page
    fn next_page(&mut self, cx: &mut Context<Self>) {
        if self.current_page < self.total_pages() - 1 {
            self.current_page += 1;
            cx.emit(PageChangeRequested {
                page: self.current_page,
            });
            cx.notify();
        }
    }

    /// Go to previous page
    fn prev_page(&mut self, cx: &mut Context<Self>) {
        if self.current_page > 0 {
            self.current_page -= 1;
            cx.emit(PageChangeRequested {
                page: self.current_page,
            });
            cx.notify();
        }
    }

    /// Calculate total content width
    fn total_width(&self) -> f32 {
        ROW_NUM_WIDTH + self.columns.iter().map(|c| c.width).sum::<f32>()
    }

    /// Get max horizontal scroll based on viewport
    fn max_horizontal_scroll(&self) -> Pixels {
        let viewport_width: f32 = self.viewport_width.into();
        let content_width = self.total_width();
        px((content_width - viewport_width).max(0.0))
    }

    /// Clamp horizontal scroll to valid range
    fn clamp_scroll(&mut self) {
        let max_scroll = self.max_horizontal_scroll();
        self.horizontal_scroll_offset = self
            .horizontal_scroll_offset
            .clamp(px(0.0), max_scroll.max(px(0.0)));
    }

    /// Handle horizontal scroll wheel
    fn handle_scroll_wheel(&mut self, delta_x: Pixels, cx: &mut Context<Self>) {
        self.horizontal_scroll_offset = self.horizontal_scroll_offset - delta_x;
        self.clamp_scroll();
        cx.notify();
    }

    /// Render the header row with horizontal offset applied
    fn render_header(&self, offset: Pixels) -> impl IntoElement {
        let header_bg = rgb(0x252525);
        let text_color = rgb(0xb0b0b0);
        let border_color = rgb(0x3a3a3a);
        let total_width = self.total_width();

        div()
            .id("table-header")
            .flex()
            .flex_row()
            .w(px(total_width))
            .h(px(HEADER_HEIGHT))
            .bg(header_bg)
            .border_b_1()
            .border_color(border_color)
            .ml(px(-f32::from(offset))) // Apply horizontal offset
            // Row number column header
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .w(px(ROW_NUM_WIDTH))
                    .h_full()
                    .border_r_1()
                    .border_color(border_color)
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(0x606060))
                            .child("#"),
                    ),
            )
            // Column headers
            .children(self.columns.iter().enumerate().map(|(idx, col)| {
                div()
                    .id(SharedString::from(format!("header-{}", idx)))
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .px(px(8.0))
                    .w(px(col.width))
                    .h_full()
                    .border_r_1()
                    .border_color(border_color)
                    .overflow_hidden()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(text_color)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(col.name.clone()),
                    )
            }))
    }

    /// Render pagination controls (fixed at bottom)
    fn render_pagination(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let bg_color = rgba(0x1e1e1eff);
        let text_color = rgb(0xb0b0b0);
        let text_muted = rgb(0x606060);
        let accent_color = rgb(0x0078d4);

        let current_page = self.current_page;
        let total_pages = self.total_pages();
        let total_items = self.total_items;

        let start_item = if total_items == 0 {
            0
        } else {
            current_page * PAGE_SIZE + 1
        };
        let end_item = ((current_page + 1) * PAGE_SIZE).min(total_items);

        let can_prev = current_page > 0;
        let can_next = current_page < total_pages - 1 && total_items > 0;

        div()
            .id("pagination-bar")
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .py(px(8.0))
            .bg(rgb(0x1e1e1e))
            .border_t_1()
            .border_color(rgb(0x3a3a3a))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(12.0))
                    .py(px(6.0))
                    .rounded(px(6.0))
                    .bg(bg_color)
                    .border_1()
                    .border_color(rgb(0x3a3a3a))
                    .shadow_sm()
                    // Previous button
                    .child(
                        div()
                            .id("prev-page")
                            .flex()
                            .items_center()
                            .justify_center()
                            .px(px(8.0))
                            .py(px(2.0))
                            .rounded(px(3.0))
                            .cursor(if can_prev {
                                CursorStyle::PointingHand
                            } else {
                                CursorStyle::default()
                            })
                            .when(can_prev, |el| {
                                el.hover(|s| s.bg(rgb(0x3a3a3a))).on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.prev_page(cx);
                                    },
                                ))
                            })
                            .child(
                                svg()
                                    .path("icons/chevron-left.svg")
                                    .size(px(14.0))
                                    .text_color(if can_prev { text_color } else { text_muted }),
                            ),
                    )
                    // Page info
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(text_color)
                                    .child(format!("{} - {}", start_item, end_item)),
                            )
                            .child(div().text_size(px(11.0)).text_color(text_muted).child("of"))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(accent_color)
                                    .child(format!("{}", total_items)),
                            ),
                    )
                    // Next button
                    .child(
                        div()
                            .id("next-page")
                            .flex()
                            .items_center()
                            .justify_center()
                            .px(px(8.0))
                            .py(px(2.0))
                            .rounded(px(3.0))
                            .cursor(if can_next {
                                CursorStyle::PointingHand
                            } else {
                                CursorStyle::default()
                            })
                            .when(can_next, |el| {
                                el.hover(|s| s.bg(rgb(0x3a3a3a))).on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.next_page(cx);
                                    },
                                ))
                            })
                            .child(
                                svg()
                                    .path("icons/chevron-right.svg")
                                    .size(px(14.0))
                                    .text_color(if can_next { text_color } else { text_muted }),
                            ),
                    ),
            )
    }

    /// Render a single row with horizontal offset applied
    fn render_row(
        &self,
        global_row_idx: usize,
        row: &Row,
        offset: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let row_even_bg = rgb(0x1e1e1e);
        let row_odd_bg = rgb(0x1a1a1a);
        let row_selected_bg = rgba(0x0078d430);
        let row_hover_bg = rgb(0x252525);
        let text_color = rgb(0xe0e0e0);
        let text_muted = rgb(0x808080);
        let border_color = rgb(0x3a3a3a);
        let accent_color = rgb(0x0078d4);

        let is_selected = self.selected_row == Some(global_row_idx);
        let is_even = global_row_idx % 2 == 0;
        let total_width = self.total_width();
        let columns = self.columns.clone();

        let row_bg = if is_selected {
            row_selected_bg
        } else if is_even {
            row_even_bg
        } else {
            row_odd_bg
        };

        div()
            .id(SharedString::from(format!("row-{}", global_row_idx)))
            .flex()
            .flex_row()
            .w(px(total_width))
            .h(px(ROW_HEIGHT))
            .bg(row_bg)
            .border_b_1()
            .border_color(border_color)
            .cursor_pointer()
            .hover(|s| s.bg(row_hover_bg))
            .ml(px(-f32::from(offset))) // Apply horizontal offset
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_row(global_row_idx, cx);
            }))
            // Selection indicator
            .when(is_selected, |el| {
                el.child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(px(3.0))
                        .bg(accent_color),
                )
            })
            // Row number
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .w(px(ROW_NUM_WIDTH))
                    .h_full()
                    .border_r_1()
                    .border_color(border_color)
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(text_muted)
                            .child(format!("{}", global_row_idx + 1)),
                    ),
            )
            // Cell values
            .children(columns.iter().enumerate().map(|(col_idx, col)| {
                let value = row
                    .values
                    .get(col_idx)
                    .cloned()
                    .unwrap_or_else(|| SharedString::from(""));

                let full_value = row
                    .full_values
                    .get(col_idx)
                    .cloned()
                    .unwrap_or_else(|| SharedString::from(""));

                let is_truncated = full_value.len() > MAX_CELL_DISPLAY_LENGTH;

                let cell_row_idx = global_row_idx;
                let cell_col_idx = col_idx;
                let cell_value = full_value.clone();

                div()
                    .id(SharedString::from(format!(
                        "cell-{}-{}",
                        global_row_idx, col_idx
                    )))
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .px(px(8.0))
                    .w(px(col.width))
                    .h_full()
                    .border_r_1()
                    .border_color(border_color)
                    .overflow_hidden()
                    .when(is_truncated, |el| {
                        el.cursor_pointer()
                            .tooltip(Tooltip::text("Click to view full content"))
                            .on_click(cx.listener(move |_this, _, _, cx| {
                                cx.emit(CellClicked {
                                    row_index: cell_row_idx,
                                    col_index: cell_col_idx,
                                    value: cell_value.clone(),
                                });
                            }))
                    })
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(if is_truncated {
                                rgb(0x6eb5ff)
                            } else {
                                text_color
                            })
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(value),
                    )
            }))
    }
}

impl Default for TableView {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for TableView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg_color = rgb(0x1a1a1a);

        // Capture scroll offset for rendering
        let scroll_offset = self.horizontal_scroll_offset;

        // Build rows data
        let page_rows: Vec<(usize, Row)> = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let global_idx = self.current_page * PAGE_SIZE + i;
                (global_idx, r.clone())
            })
            .collect();

        let entity = cx.entity().clone();

        div()
            .id("table-view")
            .size_full()
            .bg(bg_color)
            .flex()
            .flex_col()
            .overflow_hidden()
            .relative() // For absolute positioning of canvas
            // Canvas to measure actual table-view bounds (before content expands it)
            .child({
                let entity_for_canvas = entity.clone();
                canvas(
                    move |bounds, _, cx| {
                        // Capture viewport bounds in prepaint and update state
                        let new_width = bounds.size.width;
                        entity_for_canvas.update(cx, |this, cx| {
                            if (f32::from(this.viewport_width) - f32::from(new_width)).abs() > 1.0 {
                                this.viewport_width = new_width;
                                this.clamp_scroll();
                                cx.notify();
                            }
                        });
                    },
                    |_bounds, _, _window, _cx| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
            })
            // Main table area with scroll wheel handling
            .child(
                div()
                    .id("table-scroll-container")
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .min_w_0() // Allow shrinking below content
                    .overflow_hidden()
                    .relative() // For absolute content positioning
                    .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                        let delta = event.delta.pixel_delta(px(20.0));
                        // Handle horizontal scroll (shift+scroll or horizontal scroll)
                        if delta.x.abs() > px(0.5) {
                            this.handle_scroll_wheel(delta.x, cx);
                        }
                    }))
                    // Clipped content area - use absolute positioning to prevent layout expansion
                    .child(
                        div()
                            .id("table-content-clip")
                            .absolute()
                            .top_0()
                            .left_0()
                            .right_0()
                            .bottom_0()
                            .overflow_hidden()
                            .overflow_y_scroll()
                            .track_scroll(&self.vertical_scroll)
                            .child(
                                div()
                                    .id("table-content")
                                    .flex()
                                    .flex_col()
                                    // Header
                                    .child(self.render_header(scroll_offset))
                                    // Rows
                                    .children(page_rows.iter().map(|(idx, row)| {
                                        self.render_row(*idx, row, scroll_offset, cx)
                                    })),
                            ),
                    ),
            )
            // Horizontal scrollbar - canvas-based for proper bounds tracking
            .child({
                let entity_for_scrollbar = entity.clone();
                div()
                    .id("horizontal-scrollbar-container")
                    .w_full()
                    .h(px(SCROLLBAR_TRACK_HEIGHT))
                    .bg(rgb(0x1e1e1e))
                    .border_t_1()
                    .border_color(rgb(0x3a3a3a))
                    .px(px(2.0))
                    .py(px(2.0))
                    .child(
                        // Track div with canvas for mouse events and thumb painting
                        div()
                            .id("scrollbar-track")
                            .relative()
                            .w_full()
                            .h(px(SCROLLBAR_TRACK_HEIGHT - 4.0))
                            .rounded(px(4.0))
                            .bg(rgb(0x2a2a2a))
                            .cursor(CursorStyle::PointingHand)
                            // Canvas to capture track bounds, handle mouse events, and paint thumb
                            .child(
                                canvas(|_, _, _| {}, {
                                    let entity = entity_for_scrollbar.clone();
                                    move |track_bounds, _, window, cx| {
                                        let track_width: f32 = track_bounds.size.width.into();
                                        let track_origin_x = track_bounds.origin.x;

                                        // Read state to compute thumb bounds
                                        let state = entity.read(cx);
                                        let vp_width: f32 = state.viewport_width.into();
                                        let content_w = state.total_width();
                                        let max_scroll: f32 = state.max_horizontal_scroll().into();
                                        let scroll_offset: f32 =
                                            state.horizontal_scroll_offset.into();

                                        let needs_scrollbar =
                                            content_w > vp_width && vp_width > 0.0;

                                        // Calculate thumb geometry
                                        let (thumb_width, thumb_left) = if needs_scrollbar {
                                            let ratio = (vp_width / content_w).clamp(0.0, 1.0);
                                            let tw =
                                                (track_width * ratio).max(SCROLLBAR_THUMB_MIN_SIZE);
                                            let available = (track_width - tw).max(1.0);
                                            let scroll_ratio = if max_scroll > 0.0 {
                                                (scroll_offset / max_scroll).clamp(0.0, 1.0)
                                            } else {
                                                0.0
                                            };
                                            (tw, scroll_ratio * available)
                                        } else {
                                            (0.0, 0.0)
                                        };

                                        let thumb_bounds = Bounds {
                                            origin: point(
                                                track_origin_x + px(thumb_left),
                                                track_bounds.origin.y,
                                            ),
                                            size: size(px(thumb_width), track_bounds.size.height),
                                        };

                                        // Paint thumb
                                        if needs_scrollbar {
                                            let color = if state.is_dragging_scrollbar {
                                                rgb(0x808080)
                                            } else {
                                                rgb(0x606060)
                                            };

                                            window.paint_quad(PaintQuad {
                                                bounds: thumb_bounds.clone(),
                                                corner_radii: Corners::all(px(4.0)),
                                                background: color.into(),
                                                border_widths: Edges::default(),
                                                border_color: Hsla::transparent_black(),
                                                border_style: BorderStyle::default(),
                                            });
                                        }

                                        // Mouse down - start drag on thumb, or jump on track
                                        window.on_mouse_event({
                                            let entity = entity.clone();
                                            let track_bounds = track_bounds.clone();
                                            let thumb_bounds = thumb_bounds.clone();
                                            move |ev: &MouseDownEvent, _, _, cx| {
                                                if !track_bounds.contains(&ev.position) {
                                                    return;
                                                }

                                                if thumb_bounds.contains(&ev.position) {
                                                    // Click on thumb - start drag
                                                    entity.update(cx, |this, cx| {
                                                        this.is_dragging_scrollbar = true;
                                                        // Store offset within thumb from its left edge
                                                        this.drag_start = Some((
                                                            ev.position.x - thumb_bounds.origin.x,
                                                            track_origin_x,
                                                        ));
                                                        cx.notify();
                                                    });
                                                } else {
                                                    // Click on track - jump to position (center thumb on click)
                                                    entity.update(cx, |this, cx| {
                                                        let click_relative = f32::from(
                                                            ev.position.x - track_origin_x,
                                                        );
                                                        let track_w: f32 =
                                                            track_bounds.size.width.into();
                                                        let thumb_w = thumb_width;
                                                        let available =
                                                            (track_w - thumb_w).max(1.0);

                                                        // Center thumb on click position
                                                        let thumb_left_target = (click_relative
                                                            - thumb_w / 2.0)
                                                            .clamp(0.0, available);
                                                        let scroll_ratio =
                                                            thumb_left_target / available;

                                                        this.horizontal_scroll_offset =
                                                            px(scroll_ratio * max_scroll);
                                                        this.clamp_scroll();
                                                        cx.notify();
                                                    });
                                                }
                                            }
                                        });

                                        // Mouse up - end drag
                                        window.on_mouse_event({
                                            let entity = entity.clone();
                                            move |_: &MouseUpEvent, _, _, cx| {
                                                entity.update(cx, |this, cx| {
                                                    if this.is_dragging_scrollbar {
                                                        this.is_dragging_scrollbar = false;
                                                        this.drag_start = None;
                                                        cx.notify();
                                                    }
                                                });
                                            }
                                        });

                                        // Mouse move - handle drag
                                        window.on_mouse_event({
                                            let entity = entity.clone();
                                            let track_bounds = track_bounds.clone();
                                            move |ev: &MouseMoveEvent, _, _, cx| {
                                                let state = entity.read(cx);
                                                if !state.is_dragging_scrollbar || !ev.dragging() {
                                                    return;
                                                }

                                                let Some((thumb_click_offset, track_origin)) =
                                                    state.drag_start
                                                else {
                                                    return;
                                                };

                                                entity.update(cx, |this, cx| {
                                                    let vp_w: f32 = this.viewport_width.into();
                                                    let content_w = this.total_width();
                                                    let max_s: f32 =
                                                        this.max_horizontal_scroll().into();
                                                    let track_w: f32 =
                                                        track_bounds.size.width.into();

                                                    if track_w > 0.0 && max_s > 0.0 {
                                                        // Calculate thumb width
                                                        let ratio =
                                                            (vp_w / content_w).clamp(0.0, 1.0);
                                                        let thumb_w = (track_w * ratio)
                                                            .max(SCROLLBAR_THUMB_MIN_SIZE);
                                                        let available =
                                                            (track_w - thumb_w).max(1.0);

                                                        // Where should thumb left edge be?
                                                        // Mouse position - track origin - offset within thumb
                                                        let thumb_left = f32::from(
                                                            ev.position.x
                                                                - track_origin
                                                                - thumb_click_offset,
                                                        );
                                                        let thumb_left_clamped =
                                                            thumb_left.clamp(0.0, available);

                                                        let scroll_ratio =
                                                            thumb_left_clamped / available;
                                                        this.horizontal_scroll_offset =
                                                            px(scroll_ratio * max_s);
                                                        this.clamp_scroll();
                                                        cx.notify();
                                                    }
                                                });
                                            }
                                        });
                                    }
                                })
                                .size_full(),
                            ),
                    )
            })
            // Pagination
            .child(self.render_pagination(cx))
    }
}
