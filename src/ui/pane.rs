use gpui::{prelude::*, rems, *};
use std::collections::HashMap;

use crate::ui::collection_view::CollectionView;
use crate::ui::tab::TabData;
use crate::ui::tab_bar::TabBar;

/// Unique identifier for a tab (database.collection)
pub type TabId = SharedString;

/// Information about an open tab
#[derive(Clone)]
pub struct TabInfo {
    pub id: TabId,
    pub collection_name: String,
    pub database_name: String,
    #[allow(dead_code)] // May be used for reconnection or refresh
    pub connection_string: String,
    pub is_loading: bool,
}

impl TabInfo {
    pub fn new(collection_name: String, database_name: String, connection_string: String) -> Self {
        let id = SharedString::from(format!("{}.{}", database_name, collection_name));
        Self {
            id,
            collection_name,
            database_name,
            connection_string,
            is_loading: true,
        }
    }

    pub fn title(&self) -> String {
        self.collection_name.clone()
    }

    pub fn subtitle(&self) -> String {
        self.database_name.clone()
    }

    pub fn to_tab_data(&self, is_active: bool) -> TabData {
        TabData::new(self.id.clone(), self.title())
            .subtitle(self.subtitle())
            .active(is_active)
            .loading(self.is_loading)
    }
}

/// Event emitted when all tabs are closed
#[derive(Clone)]
pub struct AllTabsClosed;

impl EventEmitter<AllTabsClosed> for Pane {}

/// The Pane manages a collection of tabs and their content
pub struct Pane {
    tab_bar: TabBar,
    tabs: Vec<TabInfo>,
    active_tab_index: Option<usize>,
    /// Collection views keyed by tab ID
    collection_views: HashMap<TabId, Entity<CollectionView>>,
}

impl Pane {
    pub fn new() -> Self {
        Self {
            tab_bar: TabBar::new(),
            tabs: Vec::new(),
            active_tab_index: None,
            collection_views: HashMap::new(),
        }
    }

    /// Check if pane has any tabs
    #[allow(dead_code)] // API method for future use
    pub fn has_tabs(&self) -> bool {
        !self.tabs.is_empty()
    }

    /// Open a new tab or activate existing one for the given collection
    pub fn open_collection(
        &mut self,
        collection_name: String,
        database_name: String,
        connection_string: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tab_id = SharedString::from(format!("{}.{}", database_name, collection_name));

        // Check if tab already exists
        if let Some(index) = self.tabs.iter().position(|t| t.id == tab_id) {
            self.active_tab_index = Some(index);
            cx.notify();
            return;
        }

        // Create new tab
        let tab_info = TabInfo::new(
            collection_name.clone(),
            database_name.clone(),
            connection_string.clone(),
        );
        let tab_id_clone = tab_info.id.clone();

        // Create collection view
        let view =
            cx.new(|cx| CollectionView::new(collection_name, database_name, connection_string, cx));

        // Subscribe to view events to update loading state
        cx.subscribe_in(&view, window, {
            let tab_id = tab_id_clone.clone();
            move |pane, _, event: &crate::ui::collection_view::LoadingStateChanged, _, cx| {
                if let Some(tab) = pane.tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.is_loading = event.0;
                    cx.notify();
                }
            }
        })
        .detach();

        self.collection_views.insert(tab_id_clone, view);
        self.tabs.push(tab_info);
        self.active_tab_index = Some(self.tabs.len() - 1);

        cx.notify();
    }

    /// Close a tab by ID
    pub fn close_tab(&mut self, tab_id: &TabId, cx: &mut Context<Self>) {
        if let Some(index) = self.tabs.iter().position(|t| &t.id == tab_id) {
            self.tabs.remove(index);
            self.collection_views.remove(tab_id);

            // Adjust active index
            if self.tabs.is_empty() {
                self.active_tab_index = None;
                cx.emit(AllTabsClosed);
            } else if let Some(active) = self.active_tab_index {
                if active >= self.tabs.len() {
                    self.active_tab_index = Some(self.tabs.len() - 1);
                } else if active > index {
                    self.active_tab_index = Some(active - 1);
                }
            }

            cx.notify();
        }
    }

    /// Select a tab by ID
    pub fn select_tab(&mut self, tab_id: &TabId, cx: &mut Context<Self>) {
        if let Some(index) = self.tabs.iter().position(|t| &t.id == tab_id) {
            self.active_tab_index = Some(index);
            cx.notify();
        }
    }

    /// Get the currently active tab info
    pub fn active_tab(&self) -> Option<&TabInfo> {
        self.active_tab_index.and_then(|i| self.tabs.get(i))
    }

    fn get_tab_data(&self) -> Vec<TabData> {
        let active_id = self.active_tab().map(|t| t.id.clone());

        self.tabs
            .iter()
            .map(|tab_info| {
                let is_active = active_id.as_ref() == Some(&tab_info.id);
                tab_info.to_tab_data(is_active)
            })
            .collect()
    }
}

impl Default for Pane {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for Pane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg_color = rgb(0x1a1a1a);
        let text_muted = rgb(0x808080);

        // No tabs - show empty state
        if self.tabs.is_empty() {
            return div()
                .id("pane-empty")
                .flex()
                .flex_col()
                .size_full()
                .bg(bg_color)
                .items_center()
                .justify_center()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(rems(0.75)) // 12px
                        .child(
                            svg()
                                .path("icons/collection.svg")
                                .size(rems(3.0)) // 48px
                                .text_color(rgb(0x3a3a3a)),
                        )
                        .child(
                            div()
                                .text_size(rems(0.875)) // 14px
                                .text_color(text_muted)
                                .child("Select a collection to view data"),
                        )
                        .child(
                            div()
                                .text_size(rems(0.75)) // 12px
                                .text_color(rgb(0x606060))
                                .child("Browse databases in the sidebar"),
                        ),
                )
                .into_any_element();
        }

        // Get tab data
        let tabs_data = self.get_tab_data();

        // Get active content view
        let active_view = self
            .active_tab()
            .and_then(|tab| self.collection_views.get(&tab.id))
            .cloned();

        // Create entity handle for callbacks
        let entity = cx.entity().downgrade();
        let entity_for_close = entity.clone();

        div()
            .id("pane")
            .flex()
            .flex_col()
            .size_full()
            .bg(bg_color)
            // Tab bar
            .child(self.tab_bar.render_bar(
                tabs_data,
                // On select callback
                move |tab_id: &SharedString, _window, cx| {
                    let tab_id = tab_id.clone();
                    if let Some(entity) = entity.upgrade() {
                        entity.update(cx, |pane, cx| {
                            pane.select_tab(&tab_id, cx);
                        });
                    }
                },
                // On close callback
                move |tab_id: &SharedString, _window, cx| {
                    let tab_id = tab_id.clone();
                    if let Some(entity) = entity_for_close.upgrade() {
                        entity.update(cx, |pane, cx| {
                            pane.close_tab(&tab_id, cx);
                        });
                    }
                },
            ))
            // Content area
            .child(
                div()
                    .id("pane-content")
                    .flex_1()
                    .min_w_0() // Critical: allow shrinking below content width for horizontal scroll
                    .w_full()
                    .overflow_hidden()
                    .when_some(active_view, |el, view| el.child(view)),
            )
            .into_any_element()
    }
}
