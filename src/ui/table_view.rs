//! Table view component using gpui-component's Table.
//!
//! This module provides a wrapper around gpui-component's Table that maintains
//! the same public interface as the original custom implementation.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{prelude::*, rems, *};
use gpui_component::{
    table::{Column as GpuiColumn, ColumnSort, Table, TableDelegate, TableEvent, TableState},
    ActiveTheme,
};

use crate::ui::selectable_text::SelectableTextArea;
use crate::ui::theme::AppColors;

/// Items per page
pub const PAGE_SIZE: usize = 20;

/// Maximum display length for cell values (truncate long content)
const MAX_CELL_DISPLAY_LENGTH: usize = 100;

/// Sort direction
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl From<ColumnSort> for SortDirection {
    fn from(sort: ColumnSort) -> Self {
        match sort {
            ColumnSort::Ascending => SortDirection::Ascending,
            ColumnSort::Descending | ColumnSort::Default => SortDirection::Descending,
        }
    }
}

impl From<SortDirection> for ColumnSort {
    fn from(dir: SortDirection) -> Self {
        match dir {
            SortDirection::Ascending => ColumnSort::Ascending,
            SortDirection::Descending => ColumnSort::Descending,
        }
    }
}

/// View mode for the table
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum ViewMode {
    #[default]
    Table,
    Json,
}

/// A column definition (public interface)
#[derive(Clone)]
pub struct Column {
    pub name: SharedString,
    pub width: f32,
}

impl Column {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            width: 150.0,
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

// ── Events ──────────────────────────────────────────────────────────────

/// Event emitted when a row is selected
#[derive(Clone)]
#[allow(dead_code)]
pub struct RowSelected(pub usize);

/// Event emitted when a cell is double-clicked (for copy)
#[derive(Clone)]
pub struct CellDoubleClicked {
    #[allow(dead_code)]
    pub row_index: usize,
    #[allow(dead_code)]
    pub col_index: usize,
    pub value: SharedString,
}

/// Event for page change requests
#[derive(Clone)]
pub struct PageChangeRequested {
    pub page: usize,
}

/// Event for sort change requests
#[derive(Clone)]
pub struct SortChangeRequested {
    pub field: String,
    pub direction: SortDirection,
}

/// Event for view mode change
#[derive(Clone)]
pub struct ViewModeChanged(pub ViewMode);

/// Event: header right-clicked, bubble context menu to parent
#[derive(Clone)]
pub struct HeaderContextMenuRequested {
    pub col_name: String,
    pub position: Point<Pixels>,
}

/// Event: view dropdown toggled, bubble to parent
#[derive(Clone)]
pub struct ViewDropdownToggled {
    pub open: bool,
    pub view_mode: ViewMode,
}

/// Event: cell right-clicked, bubble context menu to parent
#[derive(Clone)]
pub struct CellContextMenuRequested {
    pub row_index: usize,
    pub col_index: usize,
    pub col_name: String,
    pub value: SharedString,
    pub position: Point<Pixels>,
}

// ── Table Delegate ──────────────────────────────────────────────────────

/// Shared state for tracking cell interactions between delegate and TableView
#[derive(Clone, Default)]
pub struct CellInteractionState {
    /// Last clicked cell (row_ix, col_ix)
    pub last_clicked_cell: Option<(usize, usize)>,
    /// Currently selected cell for highlighting (row_ix, col_ix)
    pub selected_cell: Option<(usize, usize)>,
    /// Pending context menu request (row_ix, col_ix, position)
    pub pending_context_menu: Option<(usize, usize, Point<Pixels>)>,
    /// Pending header context menu request (col_ix, position)
    pub pending_header_context_menu: Option<(usize, Point<Pixels>)>,
    /// Flag to indicate a right-click just happened (for context menu tracking)
    pub right_click_pending: bool,
}

/// Delegate for the gpui-component Table
pub struct TableViewDelegate {
    /// Stored columns for the delegate
    gpui_columns: Vec<GpuiColumn>,
    /// Our column definitions
    columns: Vec<Column>,
    /// Row data
    rows: Vec<Row>,
    /// Sort field
    sort_field: Option<String>,
    /// Sort direction
    sort_direction: Option<SortDirection>,
    /// Shared interaction state
    interaction_state: Rc<RefCell<CellInteractionState>>,
}

impl TableViewDelegate {
    pub fn new(interaction_state: Rc<RefCell<CellInteractionState>>) -> Self {
        Self {
            gpui_columns: Vec::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            sort_field: None,
            sort_direction: None,
            interaction_state,
        }
    }

    /// Update columns and rebuild gpui_columns
    pub fn set_columns(&mut self, columns: Vec<Column>) {
        self.columns = columns;
        self.rebuild_gpui_columns();
    }

    /// Update sort state and rebuild gpui_columns
    pub fn set_sort(&mut self, field: Option<String>, direction: Option<SortDirection>) {
        self.sort_field = field;
        self.sort_direction = direction;
        self.rebuild_gpui_columns();
    }

    fn rebuild_gpui_columns(&mut self) {
        self.gpui_columns = self
            .columns
            .iter()
            .map(|col| {
                let is_sorted = self.sort_field.as_deref() == Some(&col.name);

                let mut gpui_col = GpuiColumn::new(col.name.clone(), col.name.clone())
                    .width(px(col.width))
                    .sortable();

                if is_sorted {
                    gpui_col = match self.sort_direction {
                        Some(SortDirection::Ascending) => gpui_col.ascending(),
                        Some(SortDirection::Descending) => gpui_col.descending(),
                        None => gpui_col,
                    };
                }

                gpui_col
            })
            .collect();
    }
}

impl TableDelegate for TableViewDelegate {
    fn columns_count(&self, _cx: &App) -> usize {
        self.gpui_columns.len()
    }

    fn rows_count(&self, _cx: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _cx: &App) -> &GpuiColumn {
        &self.gpui_columns[col_ix]
    }

    fn perform_sort(
        &mut self,
        _col_ix: usize,
        _sort: ColumnSort,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) {
        // Sort is handled externally via events
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let value = self
            .rows
            .get(row_ix)
            .and_then(|r| r.values.get(col_ix))
            .cloned()
            .unwrap_or_else(|| SharedString::from(""));

        let is_truncated = self
            .rows
            .get(row_ix)
            .and_then(|r| r.full_values.get(col_ix))
            .map(|v| v.len() > MAX_CELL_DISPLAY_LENGTH)
            .unwrap_or(false);

        let text_color = if is_truncated {
            AppColors::truncated_text()
        } else {
            cx.theme().foreground
        };

        // Check if this cell is selected
        let is_selected = self
            .interaction_state
            .borrow()
            .selected_cell
            .map(|(r, c)| r == row_ix && c == col_ix)
            .unwrap_or(false);

        let interaction_state = self.interaction_state.clone();
        let interaction_state_for_right_click = self.interaction_state.clone();

        div()
            .id(SharedString::from(format!("cell-{}-{}", row_ix, col_ix)))
            .size_full()
            .overflow_hidden()
            .text_ellipsis()
            .whitespace_nowrap()
            .text_color(text_color)
            .text_size(rems(0.75)) // 12px
            // Cell selection highlight
            .when(is_selected, |this| {
                this.bg(AppColors::bg_cell_selected())
                    .border_1()
                    .border_color(AppColors::border_active())
            })
            // Track which cell was clicked (for CellClicked event) and select it
            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                let mut state = interaction_state.borrow_mut();
                state.last_clicked_cell = Some((row_ix, col_ix));
                state.selected_cell = Some((row_ix, col_ix));
            })
            // Right-click for context menu - capture window position using canvas bounds
            .on_mouse_down(MouseButton::Right, move |_event, window, _| {
                // event.position is relative to the element, we need window coordinates
                // The mouse position in the event is already in window coordinates for mouse events
                let window_position = window.mouse_position();
                let mut state = interaction_state_for_right_click.borrow_mut();
                state.pending_context_menu = Some((row_ix, col_ix, window_position));
                state.right_click_pending = true;
            })
            .child(value)
    }

    fn render_th(
        &mut self,
        col_ix: usize,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let col_name = self
            .columns
            .get(col_ix)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| SharedString::from(""));

        let interaction_state = self.interaction_state.clone();

        div()
            .id(("header", col_ix))
            .size_full()
            .flex()
            .items_center()
            // Match toolbar text size (px(12.0))
            .text_size(rems(0.75)) // 12px
            .text_color(cx.theme().muted_foreground)
            // Right-click for header context menu (sort options)
            .on_mouse_down(MouseButton::Right, move |_, window, _| {
                // Use window mouse position for accurate coordinates
                let window_position = window.mouse_position();
                interaction_state.borrow_mut().pending_header_context_menu =
                    Some((col_ix, window_position));
            })
            .child(col_name)
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(cx.theme().muted_foreground)
            .child("No data")
    }
}

// ── TableView ───────────────────────────────────────────────────────────

/// TableView wraps gpui-component's Table with the same public interface
pub struct TableView {
    table_state: Option<Entity<TableState<TableViewDelegate>>>,
    columns: Vec<Column>,
    rows: Vec<Row>,
    current_page: usize,
    total_items: usize,
    sort_field: Option<String>,
    sort_direction: Option<SortDirection>,
    filter_query: String,
    view_mode: ViewMode,
    raw_json: Option<String>,
    json_text_area: Option<Entity<SelectableTextArea>>,
    /// Shared interaction state for tracking cell clicks
    interaction_state: Rc<RefCell<CellInteractionState>>,
}

impl EventEmitter<RowSelected> for TableView {}
impl EventEmitter<CellDoubleClicked> for TableView {}
impl EventEmitter<PageChangeRequested> for TableView {}
impl EventEmitter<SortChangeRequested> for TableView {}
impl EventEmitter<ViewModeChanged> for TableView {}
impl EventEmitter<HeaderContextMenuRequested> for TableView {}
impl EventEmitter<ViewDropdownToggled> for TableView {}
impl EventEmitter<CellContextMenuRequested> for TableView {}

impl TableView {
    pub fn new() -> Self {
        Self {
            table_state: None,
            columns: Vec::new(),
            rows: Vec::new(),
            current_page: 0,
            total_items: 0,
            sort_field: None,
            sort_direction: None,
            filter_query: String::new(),
            view_mode: ViewMode::Table,
            raw_json: None,
            json_text_area: None,
            interaction_state: Rc::new(RefCell::new(CellInteractionState::default())),
        }
    }

    fn ensure_table_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.table_state.is_some() {
            return;
        }

        // Create delegate with shared interaction state
        let mut delegate = TableViewDelegate::new(self.interaction_state.clone());
        delegate.set_columns(self.columns.clone());
        delegate.rows = self.rows.clone();
        delegate.set_sort(self.sort_field.clone(), self.sort_direction);

        let table_state = cx.new(|cx| {
            TableState::new(delegate, window, cx)
                .sortable(true)
                .col_resizable(true)
                .row_selectable(true)
        });

        // Subscribe to table events
        let interaction_state = self.interaction_state.clone();
        cx.subscribe(&table_state, move |this, _table, event: &TableEvent, cx| {
            match event {
                TableEvent::SelectRow(row_ix) => {
                    // Reset right-click flag (used for context menu tracking)
                    interaction_state.borrow_mut().right_click_pending = false;

                    let global_idx = this.current_page * PAGE_SIZE + *row_ix;
                    cx.emit(RowSelected(global_idx));
                    // NOTE: CellClicked is no longer emitted - panel opens only from context menu
                }
                TableEvent::DoubleClickedRow(row_ix) => {
                    let global_idx = this.current_page * PAGE_SIZE + *row_ix;

                    // Get the clicked column from interaction state, default to 0
                    let col_ix = interaction_state
                        .borrow()
                        .last_clicked_cell
                        .filter(|(r, _)| *r == *row_ix)
                        .map(|(_, c)| c)
                        .unwrap_or(0);

                    if let Some(row) = this.rows.get(*row_ix) {
                        if let Some(value) = row.full_values.get(col_ix) {
                            cx.emit(CellDoubleClicked {
                                row_index: global_idx,
                                col_index: col_ix,
                                value: value.clone(),
                            });
                        }
                    }
                }
                _ => {}
            }
        })
        .detach();

        self.table_state = Some(table_state);
    }

    fn update_delegate(&mut self, cx: &mut Context<Self>) {
        if let Some(table_state) = &self.table_state {
            let columns = self.columns.clone();
            let rows = self.rows.clone();
            let sort_field = self.sort_field.clone();
            let sort_direction = self.sort_direction;

            table_state.update(cx, |state, cx| {
                let delegate = state.delegate_mut();
                delegate.set_columns(columns);
                delegate.rows = rows;
                delegate.set_sort(sort_field, sort_direction);
                state.refresh(cx);
            });
        }
    }

    pub fn set_columns(&mut self, columns: Vec<Column>, cx: &mut Context<Self>) {
        self.columns = columns;
        self.update_delegate(cx);
        cx.notify();
    }

    pub fn set_rows(&mut self, rows: Vec<Row>, cx: &mut Context<Self>) {
        self.rows = rows;
        self.update_delegate(cx);
        cx.notify();
    }

    pub fn set_total_items(&mut self, total: usize, cx: &mut Context<Self>) {
        self.total_items = total;
        cx.notify();
    }

    pub fn set_page(&mut self, page: usize, cx: &mut Context<Self>) {
        self.current_page = page;
        cx.notify();
    }

    pub fn set_raw_json(&mut self, json: String, cx: &mut Context<Self>) {
        let content = json.clone();
        if let Some(text_area) = &self.json_text_area {
            text_area.update(cx, |ta, cx| {
                ta.set_content(content, cx);
            });
        } else {
            self.json_text_area = Some(cx.new(|cx| SelectableTextArea::new(cx, content)));
        }
        self.raw_json = Some(json);
        cx.notify();
    }

    #[allow(dead_code)]
    pub fn set_filter_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.filter_query = query;
        cx.notify();
    }

    pub fn set_sort(
        &mut self,
        field: Option<String>,
        direction: Option<SortDirection>,
        cx: &mut Context<Self>,
    ) {
        self.sort_field = field;
        self.sort_direction = direction;
        self.update_delegate(cx);
        cx.notify();
    }

    pub fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        self.view_mode = mode;
        cx.emit(ViewModeChanged(mode));
        cx.notify();
    }

    #[allow(dead_code)]
    pub fn reset_pagination(&mut self, cx: &mut Context<Self>) {
        self.current_page = 0;
        self.total_items = self.rows.len();
        cx.notify();
    }

    #[allow(dead_code)]
    pub fn current_page(&self) -> usize {
        self.current_page
    }

    pub fn total_pages(&self) -> usize {
        if self.total_items == 0 {
            1
        } else {
            self.total_items.div_ceil(PAGE_SIZE)
        }
    }

    #[allow(dead_code)]
    pub fn select_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.rows.len() {
            if let Some(table_state) = &self.table_state {
                table_state.update(cx, |state, cx| {
                    state.set_selected_row(index, cx);
                });
            }
            cx.emit(RowSelected(index));
            cx.notify();
        }
    }

    #[allow(dead_code)]
    pub fn selected_row(&self) -> Option<usize> {
        self.table_state.as_ref().and_then(|_ts| {
            // We can't easily read the state here without cx, so return None
            None
        })
    }

    fn next_page(&mut self, cx: &mut Context<Self>) {
        if self.current_page < self.total_pages() - 1 {
            self.current_page += 1;
            cx.emit(PageChangeRequested {
                page: self.current_page,
            });
            cx.notify();
        }
    }

    fn prev_page(&mut self, cx: &mut Context<Self>) {
        if self.current_page > 0 {
            self.current_page -= 1;
            cx.emit(PageChangeRequested {
                page: self.current_page,
            });
            cx.notify();
        }
    }

    /// Process pending cell/header interactions and emit appropriate events
    fn process_pending_interactions(&mut self, cx: &mut Context<Self>) {
        // Check for pending cell context menu
        if let Some((row_ix, col_ix, position)) = self
            .interaction_state
            .borrow_mut()
            .pending_context_menu
            .take()
        {
            let global_idx = self.current_page * PAGE_SIZE + row_ix;
            let col_name = self
                .columns
                .get(col_ix)
                .map(|c| c.name.to_string())
                .unwrap_or_default();
            let value = self
                .rows
                .get(row_ix)
                .and_then(|r| r.full_values.get(col_ix))
                .cloned()
                .unwrap_or_else(|| SharedString::from(""));

            cx.emit(CellContextMenuRequested {
                row_index: global_idx,
                col_index: col_ix,
                col_name,
                value,
                position,
            });
        }

        // Check for pending header context menu
        if let Some((col_ix, position)) = self
            .interaction_state
            .borrow_mut()
            .pending_header_context_menu
            .take()
        {
            let col_name = self
                .columns
                .get(col_ix)
                .map(|c| c.name.to_string())
                .unwrap_or_default();

            cx.emit(HeaderContextMenuRequested { col_name, position });
        }
    }

    // ── Toolbar ─────────────────────────────────────────────────────────

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let filter_display = if self.filter_query.is_empty() {
            "{}".to_string()
        } else {
            self.filter_query.clone()
        };

        let sort_display = match (&self.sort_field, &self.sort_direction) {
            (Some(field), Some(SortDirection::Ascending)) => format!("{{\"{}\": 1}}", field),
            (Some(field), Some(SortDirection::Descending)) => format!("{{\"{}\": -1}}", field),
            _ => "{}".to_string(),
        };

        let current_view = self.view_mode;

        div()
            .id("table-toolbar")
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .h(rems(2.25)) // 36px
            .px(rems(0.75)) // 12px
            .bg(AppColors::bg_header())
            .border_b_1()
            .border_color(AppColors::border())
            // Left side: Filter and Sort
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(rems(1.25)) // 20px
                    // Filter
                    .child(
                        div()
                            .id("filter-display")
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(rems(0.375)) // 6px
                            .px(rems(0.5)) // 8px
                            .py(rems(0.25)) // 4px
                            .rounded(px(4.0)) // Keep border radius as px
                            .cursor_pointer()
                            .hover(|s| s.bg(AppColors::bg_hover()))
                            .child(
                                div()
                                    .text_size(rems(0.75)) // 12px
                                    .text_color(AppColors::text_dim())
                                    .child("Filter:"),
                            )
                            .child(
                                div()
                                    .text_size(rems(0.75)) // 12px
                                    .font_family("monospace")
                                    .text_color(AppColors::text_secondary())
                                    .child(filter_display),
                            ),
                    )
                    // Sort with clear button
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(rems(0.25)) // 4px
                            .child(
                                div()
                                    .id("sort-display")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(rems(0.375)) // 6px
                                    .px(rems(0.5)) // 8px
                                    .py(rems(0.25)) // 4px
                                    .rounded(px(4.0)) // Keep border radius as px
                                    .cursor_pointer()
                                    .hover(|s| s.bg(AppColors::bg_hover()))
                                    .child(
                                        div()
                                            .text_size(rems(0.75)) // 12px
                                            .text_color(AppColors::text_dim())
                                            .child("Sort:"),
                                    )
                                    .child(
                                        div()
                                            .text_size(rems(0.75)) // 12px
                                            .font_family("monospace")
                                            .text_color(if self.sort_field.is_some() {
                                                AppColors::accent()
                                            } else {
                                                AppColors::text_secondary()
                                            })
                                            .child(sort_display),
                                    ),
                            )
                            .when(self.sort_field.is_some(), |el| {
                                el.child(
                                    div()
                                        .id("clear-sort")
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(rems(1.125)) // 18px
                                        .rounded(px(3.0)) // Keep border radius as px
                                        .cursor_pointer()
                                        .hover(|s| s.bg(AppColors::bg_hover()))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.sort_field = None;
                                            this.sort_direction = None;
                                            cx.emit(SortChangeRequested {
                                                field: String::new(),
                                                direction: SortDirection::Ascending,
                                            });
                                            cx.notify();
                                        }))
                                        .child(
                                            svg()
                                                .path("icons/close.svg")
                                                .size(rems(0.625)) // 10px
                                                .text_color(AppColors::text_dim()),
                                        ),
                                )
                            }),
                    ),
            )
            // Right side: View dropdown trigger
            .child(
                div()
                    .id("view-dropdown-trigger")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(rems(0.25)) // 4px
                    .px(rems(0.625)) // 10px
                    .py(rems(0.3125)) // 5px
                    .rounded(px(4.0)) // Keep border radius as px
                    .cursor_pointer()
                    .bg(AppColors::bg_active())
                    .border_1()
                    .border_color(AppColors::border())
                    .hover(|s| s.bg(AppColors::bg_hover()))
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.emit(ViewDropdownToggled {
                            open: true,
                            view_mode: this.view_mode,
                        });
                    }))
                    .child(
                        div()
                            .text_size(rems(0.75)) // 12px
                            .text_color(AppColors::text_secondary())
                            .child(match current_view {
                                ViewMode::Table => "Table",
                                ViewMode::Json => "JSON",
                            }),
                    )
                    .child(
                        svg()
                            .path("icons/chevron-down.svg")
                            .size(rems(0.75)) // 12px
                            .text_color(AppColors::text_dim()),
                    ),
            )
    }

    // ── Pagination ──────────────────────────────────────────────────────

    fn render_pagination(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
            .py(rems(0.5)) // 8px
            .bg(AppColors::bg_secondary())
            .border_t_1()
            .border_color(AppColors::border())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(rems(0.5)) // 8px
                    .px(rems(0.75)) // 12px
                    .py(rems(0.375)) // 6px
                    .rounded(px(6.0)) // Keep border radius as px
                    .bg(AppColors::bg_secondary())
                    .border_1()
                    .border_color(AppColors::border())
                    .shadow_sm()
                    .child(
                        div()
                            .id("prev-page")
                            .flex()
                            .items_center()
                            .justify_center()
                            .px(rems(0.5)) // 8px
                            .py(rems(0.125)) // 2px
                            .rounded(px(3.0)) // Keep border radius as px
                            .cursor(if can_prev {
                                CursorStyle::PointingHand
                            } else {
                                CursorStyle::default()
                            })
                            .when(can_prev, |el| {
                                el.hover(|s| s.bg(AppColors::bg_hover()))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.prev_page(cx);
                                    }))
                            })
                            .child(
                                svg()
                                    .path("icons/chevron-left.svg")
                                    .size(rems(0.875)) // 14px
                                    .text_color(if can_prev {
                                        AppColors::text_secondary()
                                    } else {
                                        AppColors::text_dim()
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(rems(0.375)) // 6px
                            .child(
                                div()
                                    .text_size(rems(0.75)) // 12px
                                    .text_color(AppColors::text_secondary())
                                    .child(format!("{} - {}", start_item, end_item)),
                            )
                            .child(
                                div()
                                    .text_size(rems(0.6875)) // 11px
                                    .text_color(AppColors::text_dim())
                                    .child("of"),
                            )
                            .child(
                                div()
                                    .text_size(rems(0.75)) // 12px
                                    .text_color(AppColors::accent())
                                    .child(format!("{}", total_items)),
                            ),
                    )
                    .child(
                        div()
                            .id("next-page")
                            .flex()
                            .items_center()
                            .justify_center()
                            .px(rems(0.5)) // 8px
                            .py(rems(0.125)) // 2px
                            .rounded(px(3.0)) // Keep border radius as px
                            .cursor(if can_next {
                                CursorStyle::PointingHand
                            } else {
                                CursorStyle::default()
                            })
                            .when(can_next, |el| {
                                el.hover(|s| s.bg(AppColors::bg_hover()))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.next_page(cx);
                                    }))
                            })
                            .child(
                                svg()
                                    .path("icons/chevron-right.svg")
                                    .size(rems(0.875)) // 14px
                                    .text_color(if can_next {
                                        AppColors::text_secondary()
                                    } else {
                                        AppColors::text_dim()
                                    }),
                            ),
                    ),
            )
    }

    // ── JSON view ───────────────────────────────────────────────────────

    fn render_json_view(&self) -> impl IntoElement {
        div()
            .id("json-view")
            .flex_1()
            .min_h_0()
            .bg(AppColors::bg_main())
            .p(rems(1.0)) // 16px
            .overflow_y_scroll()
            .overflow_x_scroll()
            .when_some(self.json_text_area.clone(), |el, text_area| {
                el.child(text_area)
            })
    }
}

impl Default for TableView {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for TableView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view_mode = self.view_mode;

        // Ensure table state is created
        self.ensure_table_state(window, cx);

        // Check for pending context menu requests and emit events
        self.process_pending_interactions(cx);

        div()
            .id("table-view")
            .size_full()
            .bg(AppColors::bg_main())
            .flex()
            .flex_col()
            .overflow_hidden()
            // Toolbar
            .child(self.render_toolbar(cx))
            // Table view
            .when(view_mode == ViewMode::Table, |el| {
                if let Some(table_state) = &self.table_state {
                    el.child(
                        div()
                            .id("table-container")
                            .flex_1()
                            .min_h_0()
                            .overflow_hidden()
                            .bg(AppColors::bg_main())
                            .child(Table::new(table_state).stripe(true).bordered(true)),
                    )
                } else {
                    el
                }
            })
            // JSON view
            .when(view_mode == ViewMode::Json, |el| {
                el.child(self.render_json_view())
            })
            // Pagination (always shown)
            .child(self.render_pagination(cx))
    }
}
