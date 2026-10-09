use gpui_kit::{
    component::{button::*, checkbox::Checkbox, *},
    *,
};

use crate::{ast::*, consts::*, i18n::*, tree::*, view::RegexVisualizer};

pub(crate) const UNDO_LIMIT: usize = 100;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum EditSlot {
    ContentType(NodeId),
    ClassKind(NodeId),
    Backref(NodeId),
    QuantKind(NodeId),
    GroupKind(NodeId),
    LookKind(NodeId),
    Value(NodeId),
    RangeFrom(NodeId, usize),
    RangeTo(NodeId, usize),
    QuantMin(NodeId),
    QuantMax(NodeId),
    GroupName(NodeId),
}

#[derive(Clone)]
pub(crate) struct LabeledItem {
    pub(crate) key: String,
    pub(crate) label: SharedString,
}

impl LabeledItem {
    pub(crate) fn new(key: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
        }
    }
}

impl gpui_kit::component::searchable_list::SearchableListItem for LabeledItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.key
    }
}

pub(crate) const CLASS_OPTIONS: [(&str, &str, &str); 22] = [
    (".", "任意字符", "Any character"),
    (r"\d", "任意数字", "Any digit"),
    (r"\D", "任意非数字", "Non-digit"),
    (r"\w", "任意基本拉丁字母数字", "Any alphanumeric"),
    (r"\W", "任意非基本拉丁字母数字", "Non-alphanumeric"),
    (r"\s", "任意空白字符", "White space"),
    (r"\S", "任意非空白字符", "Non-white space"),
    (r"\t", "制表符", "Horizontal tab"),
    (r"\r", "回车符", "Carriage return"),
    (r"\n", "换行符", "Linefeed"),
    (r"\v", "垂直制表符", "Vertical tab"),
    (r"\f", "Form-feed", "Form-feed"),
    (r"[\b]", "退格", "Backspace"),
    (r"\0", "NUL", "NUL"),
    (r"\cH", r"\b 退格", r"\b Backspace"),
    (r"\cI", r"\t 制表符", r"\t Horizontal Tab"),
    (r"\cJ", r"\n 换行符", r"\n Line Feed"),
    (r"\cK", r"\v 垂直制表符", r"\v Vertical Tab"),
    (r"\cL", r"\f Form Feed", r"\f Form Feed"),
    (r"\cM", r"\r 回车符", r"\r Carriage Return"),
    (r"\xhh", "ASCII symbol", "ASCII symbol"),
    (r"\uhhhh", "Unicode symbol", "Unicode symbol"),
];

pub(crate) fn class_kind_key(value: &str) -> String {
    let bytes: Vec<char> = value.chars().collect();
    if bytes.len() == 4
        && bytes[0] == '\\'
        && bytes[1] == 'x'
        && bytes[2].is_ascii_hexdigit()
        && bytes[3].is_ascii_hexdigit()
    {
        return r"\xhh".to_string();
    }
    if bytes.len() == 6
        && bytes[0] == '\\'
        && bytes[1] == 'u'
        && bytes[2..].iter().all(|c| c.is_ascii_hexdigit())
    {
        return r"\uhhhh".to_string();
    }
    value.to_string()
}

pub(crate) fn class_items(lang: Lang) -> Vec<LabeledItem> {
    CLASS_OPTIONS
        .iter()
        .map(|(key, cn, en)| {
            let label = lang.of(cn, en);
            LabeledItem::new(*key, format!("{key}  {label}"))
        })
        .collect()
}

pub(crate) fn set_group_name(tree: &mut ERoot, id: NodeId, name: &str) {
    let Some((path, index)) = tree.find(id) else {
        return;
    };
    let body = &mut tree.body;
    let seq = seq_at_mut(body, &path);
    if let Some(node) = seq.get_mut(index)
        && let EKind::Group {
            kind, name: slot, ..
        } = &mut node.kind
    {
        *kind = EGroupKind::NamedCapturing;
        *slot = if name.is_empty() {
            "name".to_string()
        } else {
            name.to_string()
        };
    }
}

pub(crate) fn group_kind_key(kind: EGroupKind) -> &'static str {
    match kind {
        EGroupKind::Capturing => "capturing",
        EGroupKind::NonCapturing => "nonCapturing",
        EGroupKind::NamedCapturing => "namedCapturing",
    }
}

pub(crate) fn look_kind_key(kind: LookKind) -> &'static str {
    match kind {
        LookKind::Lookahead => "lookahead",
        LookKind::Lookbehind => "lookbehind",
    }
}

pub(crate) const OK_GREEN: u32 = 0x18a058;

pub(crate) fn editor_label(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(12.0))
        .font_semibold()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

pub(crate) fn hint(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(12.5))
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

pub(crate) fn pill(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .px_2()
        .py(px(2.0))
        .rounded_full()
        .bg(cx.theme().primary.opacity(0.1))
        .text_size(px(12.0))
        .font_semibold()
        .text_color(cx.theme().primary)
        .child(text.into())
}

pub(crate) fn panel_section(
    label: &'static str,
    action: Option<AnyElement>,
    body: AnyElement,
    cx: &App,
) -> Div {
    let mut head = div().flex().items_center().justify_between();
    head = head.child(editor_label(label.to_string(), cx));
    if let Some(action) = action {
        head = head.child(action);
    }
    div().flex().flex_col().gap_2().child(head).child(body)
}

pub(crate) fn panel_item(label: &'static str, body: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_size(px(13.0)).child(label))
        .child(body)
}

pub(crate) fn button_row(buttons: Vec<AnyElement>) -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_1()
        .children(buttons)
}

pub(crate) fn insert_button(
    id: &'static str,
    label: &'static str,
    mode: InsertMode,
    ids: &[NodeId],
    cx: &Context<RegexVisualizer>,
) -> AnyElement {
    let ids = ids.to_vec();
    Button::new(id)
        .label(label)
        .outline()
        .compact()
        .on_click(cx.listener(move |this, _, window, cx| {
            let ids = ids.clone();
            this.edit(window, cx, move |tree, id_gen| {
                tree.insert_around(&ids, mode, id_gen);
                Vec::new()
            });
        }))
        .into_any_element()
}

pub(crate) fn negate_row<F>(
    id: &'static str,
    checked: bool,
    cx: &Context<RegexVisualizer>,
    handler: F,
) -> Div
where
    F: Fn(&mut RegexVisualizer, bool, &mut Window, &mut Context<RegexVisualizer>) + 'static,
{
    div()
        .flex()
        .items_center()
        .gap_2()
        .text_size(px(13.0))
        .text_color(cx.theme().muted_foreground)
        .child("Negate")
        .child(Checkbox::new(id).checked(checked).on_click(cx.listener(
            move |this, value: &bool, window, cx| {
                handler(this, *value, window, cx);
            },
        )))
}

pub(crate) fn expression_body(
    pattern: &str,
    start: usize,
    end: usize,
    family: &SharedString,
) -> AnyElement {
    let (head, mid, tail) = if start <= end
        && end <= pattern.len()
        && pattern.is_char_boundary(start)
        && pattern.is_char_boundary(end)
    {
        (&pattern[..start], &pattern[start..end], &pattern[end..])
    } else {
        (pattern, "", "")
    };
    let highlight = Hsla {
        a: 0.5,
        ..Hsla::from(rgb(SELECT_BLUE_LIGHT))
    };
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .font_family(family.clone())
        .text_sm()
        .child(head.to_string())
        .child(
            div()
                .rounded(px(4.0))
                .py(px(2.0))
                .bg(highlight)
                .child(mid.to_string()),
        )
        .child(tail.to_string())
        .into_any_element()
}
