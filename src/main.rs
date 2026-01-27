use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;

use gpui::{App, AppContext, Application, AssetSource, SharedString, WindowDecorations};

mod ui;

use ui::workspace::ChambersWorkspace;

/// Asset source for loading icons and other resources
struct Assets {
    base: PathBuf,
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

    Application::new()
        .with_assets(Assets { base: assets_path })
        .run(|cx: &mut App| {
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
