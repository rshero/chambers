use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;

use gpui::{App, AppContext, Application, AssetSource, SharedString, WindowDecorations};

mod db;
mod ui;

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
                    .map_or(false, |ext| ext == "ttf" || ext == "otf")
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
        // Load custom fonts
        let assets_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
        let assets = Assets { base: assets_path };
        if let Err(e) = assets.load_fonts(cx) {
            eprintln!("Failed to load fonts: {}", e);
        }

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
            |_, cx| cx.new(|cx| ChambersWorkspace::new(cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}
