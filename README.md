# gpui-regex-visualizer

A regular expression **railroad diagram visualizer / editor** built on [GPUI Kit](https://crates.io/crates/gpui-kit).

It parses a regex into a syntax tree, then lays it out horizontally as a railroad diagram — capture groups, alternations, quantifiers, and assertions become obvious at a glance. You can also **edit the regex by clicking nodes in the diagram**, no need to hand-write the pattern.

---

## Features

- **Railroad diagram rendering** — converts the `regex-syntax` AST into an internal node tree and hand-draws the diagram from SVG primitives: sequential links, `|` alternations branching up and down, quantifier loops, and boxed groups.
- **Click to edit** — select a node in the diagram and the "Edit" panel in the right column shows a form tailored to that node type: change literals, switch character classes, adjust quantifier bounds, swap group kinds, edit boundary assertions, wrap the selection in a group or look-around, and more. Every change is **serialized back** into the regex string and written into the input field.
- **Undo / redo** — a 100-step history stack (`UNDO_LIMIT`), bound to `Ctrl/Cmd+Z` and `Ctrl/Cmd+Shift+Z`.
- **Live testing** — paste test text and every match is highlighted in real time, along with the position and content of each match.
- **Built-in samples** — 20 common patterns (integers/decimals, URLs, dates and times, phone numbers, ID cards, CJK characters, emails, IPv4, license plates, currency amounts, and more); click one to load it.
- **Legend panel** — explains, by category, what each drawing convention in the diagram maps to in regex syntax.
- **Bilingual UI** — switchable at runtime between Chinese and English; Chinese by default.
- **Dark / light following** — colors are taken from the GPUI theme.

---

## Getting started

```rust
use gpui_kit::*;
use gpui_regex_visualizer::RegexVisualizer;

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            gpui_kit::open_window(
                WindowOptions::default(),
                cx,
                |window, cx| cx.new(|cx| RegexVisualizer::new(window, cx)),
            )
            .expect("failed to open window");
        });
}
```

---

## License

[MIT](LICENSE) © LittleGuest
