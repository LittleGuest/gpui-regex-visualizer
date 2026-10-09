use gpui_kit::*;
use gpui_regex_visualizer::RegexVisualizer;

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);

            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(1180.0), px(760.0)),
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Regex Visualizer".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| RegexVisualizer::new(window, cx)),
            )
            .expect("failed to open window");
        });
}
