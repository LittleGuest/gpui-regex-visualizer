use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, Entity, Point, TestAppContext, WindowBounds,
    WindowOptions, base::Root, px, size,
};
use gpui_regex_visualizer::{Lang, RegexVisualizer};

fn open_window(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<RegexVisualizer>) {
    cx.update(|cx| {
        let bounds = Bounds {
            origin: Point::default(),
            size: size(px(900.0), px(600.0)),
        };
        let (window, content) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            cx,
            |window, cx: &mut App| cx.new(|cx| RegexVisualizer::new(window, cx)),
        )
        .expect("open test window");
        (
            window.downcast::<Root>().expect("Base Root").into(),
            content,
        )
    })
}

fn lang(cx: &TestAppContext, view: &Entity<RegexVisualizer>) -> Lang {
    cx.read(|app| view.read(app).lang())
}

#[gpui_kit::test]
fn visualizer_opens_with_the_default_pattern(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (_, view) = open_window(cx);

    assert_eq!(lang(cx, &view), Lang::Cn);
}

#[gpui_kit::test]
fn language_defaults_to_chinese_and_toggles(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, view) = open_window(cx);

    assert_eq!(lang(cx, &view), Lang::Cn);

    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| view.toggle_lang(window, cx))
    })
    .expect("toggle to English");
    assert_eq!(lang(cx, &view), Lang::En);

    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| view.toggle_lang(window, cx))
    })
    .expect("toggle back to Chinese");
    assert_eq!(lang(cx, &view), Lang::Cn);
}

#[gpui_kit::test]
fn set_lang_reaches_the_requested_language(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, view) = open_window(cx);

    for target in [Lang::En, Lang::En, Lang::Cn, Lang::Cn] {
        cx.update_window(handle, |_, window, cx| {
            view.update(cx, |view, cx| view.set_lang(target, window, cx))
        })
        .expect("set language");
        assert_eq!(lang(cx, &view), target);
    }
}

#[test]
fn lang_labels_are_pure_and_bilingual() {
    assert_eq!(Lang::default(), Lang::Cn);
    assert_eq!(Lang::Cn.of("中文", "English"), "中文");
    assert_eq!(Lang::En.of("中文", "English"), "English");
    assert_eq!(Lang::Cn.toggled(), Lang::En);
    assert_eq!(Lang::En.toggled(), Lang::Cn);
    assert_eq!(Lang::Cn.badge(), "中");
    assert_eq!(Lang::En.badge(), "EN");
    assert_ne!(Lang::Cn.tooltip(), Lang::En.tooltip());
}
