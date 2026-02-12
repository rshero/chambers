use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;

use gpui::{App, AppContext, Application, AssetSource, SharedString, WindowDecorations, px, rgb};

mod db;
mod ui;

use ui::connection_modal::register_connection_modal_bindings;
use ui::selectable_text::register_selectable_text_bindings;
use ui::text_input::register_text_input_bindings;
use ui::workspace::ChambersWorkspace;

/// Asset source for loading icons and other resources
struct Assets {
    base: PathBuf,
}

impl Assets {
    /// Load all fonts from the fonts directory
    fn load_fonts(&self, cx: &App) -> anyhow::Result<()> {
        let fonts_dir = self.base.join("fonts");
        let mut fonts = Vec::new();

        if let Ok(entries) = fs::read_dir(&fonts_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path
                    .extension()
                    .is_some_and(|ext| ext == "ttf" || ext == "otf")
                {
                    if let Ok(data) = fs::read(&path) {
                        fonts.push(Cow::Owned(data));
                    }
                }
            }
        }

        if !fonts.is_empty() {
            cx.text_system().add_fonts(fonts)?;
        }

        Ok(())
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        fs::read(self.base.join(path))
            .map(|data| Some(Cow::Owned(data)))
            .map_err(|err| err.into())
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        fs::read_dir(self.base.join(path))
            .map(|entries| {
                entries
                    .filter_map(|entry| {
                        entry
                            .ok()
                            .and_then(|entry| entry.file_name().into_string().ok())
                            .map(SharedString::from)
                    })
                    .collect()
            })
            .map_err(|err| err.into())
    }
}

fn main() {
    // Set up asset path - in release, this would be embedded
    let assets_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let assets = Assets { base: assets_path };

    Application::new().with_assets(assets).run(|cx: &mut App| {
        // Initialize gpui-component (must be called before using any GPUI Component features)
        gpui_component::init(cx);

        // Set dark theme for gpui-component (the app uses dark colors)
        gpui_component::theme::Theme::change(gpui_component::theme::ThemeMode::Dark, None, cx);

        // Customize theme colors to match app's dark gray background (#1a1a1a)
        // instead of gpui-component's default pure black (#0a0a0a)
        {
            let theme = gpui_component::theme::Theme::global_mut(cx);
            let bg_main = rgb(0x1a1a1a).into();
            let bg_header = rgb(0x252525).into();
            let bg_secondary = rgb(0x1e1e1e).into();
            let border = rgb(0x3a3a3a).into();
            let bg_hover = rgb(0x3a3a3a).into();

            // Table colors
            theme.table = bg_main;
            theme.table_head = bg_header;
            theme.table_even = bg_secondary;
            theme.table_hover = rgb(0x2a2a2a).into();
            theme.table_row_border = border;
            // Disable row selection highlight (we use cell-level highlighting instead)
            theme.table_active = gpui::transparent_black();
            theme.table_active_border = gpui::transparent_black();

            // General background colors for consistency
            theme.background = bg_main;
            theme.popover = bg_main;
            theme.list = bg_main;
            theme.list_head = bg_header;
            theme.list_even = bg_secondary;

            // Context menu styling: rectangular corners and visible hover
            theme.radius = px(0.);
            theme.accent = bg_hover;
            theme.accent_foreground = rgb(0xe0e0e0).into();

            // Font size for menus to match sidebar text (13px sidebar items, 12px for menus)
            theme.font_size = px(12.);
        }

        // Load custom fonts
        let assets_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
        let assets = Assets { base: assets_path };
        if let Err(e) = assets.load_fonts(cx) {
            eprintln!("Failed to load fonts: {}", e);
        }

        // Register text input key bindings
        register_text_input_bindings(cx);

        // Register selectable text key bindings
        register_selectable_text_bindings(cx);

        // Register connection modal key bindings (Tab navigation)
        register_connection_modal_bindings(cx);

        let bounds =
            gpui::Bounds::centered(None, gpui::size(gpui::px(1200.0), gpui::px(800.0)), cx);
        cx.open_window(
            gpui::WindowOptions {
                window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
                // Use client-side decorations (hides system title bar)
                window_decorations: Some(WindowDecorations::Client),
                // Transparent titlebar allows custom title bar rendering
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some(gpui::SharedString::new_static("Chambers")),
                    appears_transparent: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(ChambersWorkspace::new),
        )
        .unwrap();
        cx.activate(true);
    });
}
