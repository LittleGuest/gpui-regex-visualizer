#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Lang {
    #[default]
    Cn,
    En,
}

impl Lang {
    pub fn of(self, cn: &'static str, en: &'static str) -> &'static str {
        match self {
            Lang::Cn => cn,
            Lang::En => en,
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            Lang::Cn => Lang::En,
            Lang::En => Lang::Cn,
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            Lang::Cn => "中",
            Lang::En => "EN",
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Lang::Cn => "切换为 English",
            Lang::En => "切换为中文",
        }
    }
}

pub(crate) const CLASS_LABELS: [(&str, &str, &str); 13] = [
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
    (r"\0", "NUL", "NUL"),
];

pub(crate) fn class_label(raw: &str, lang: Lang) -> Option<&'static str> {
    CLASS_LABELS
        .iter()
        .find(|(key, _, _)| *key == raw)
        .map(|(_, cn, en)| lang.of(cn, en))
}

pub(crate) const LITERAL_LABELS: [(char, &str, &str); 6] = [
    ('\n', "换行符", "Linefeed"),
    ('\r', "回车符", "Carriage return"),
    ('\t', "制表符", "Horizontal tab"),
    ('\u{0b}', "垂直制表符", "Vertical tab"),
    ('\u{0c}', "Form-feed", "Form-feed"),
    ('\0', "NUL", "NUL"),
];

pub(crate) fn group_word(lang: Lang) -> &'static str {
    lang.of("组", "Group")
}

pub(crate) fn boundary_label(lang: Lang, negated: bool) -> &'static str {
    match (lang, negated) {
        (Lang::Cn, false) => "单词边界",
        (Lang::Cn, true) => "非单词边界",
        (Lang::En, false) => "WordBoundary",
        (Lang::En, true) => "NonWordBoundary",
    }
}

pub(crate) fn beginning_label(lang: Lang) -> &'static str {
    lang.of("以...开始", "Begins with")
}

pub(crate) fn ending_label(lang: Lang) -> &'static str {
    lang.of("以...结束", "Ends with")
}

pub(crate) fn lookaround_label(lang: Lang, ahead: bool, negated: bool) -> &'static str {
    match (lang, ahead, negated) {
        (Lang::Cn, true, false) => "接着:",
        (Lang::Cn, true, true) => "不接着:",
        (Lang::Cn, false, false) => "前面是:",
        (Lang::Cn, false, true) => "前面不是:",
        (Lang::En, true, false) => "Followed by:",
        (Lang::En, true, true) => "Not followed by:",
        (Lang::En, false, false) => "Preceded by:",
        (Lang::En, false, true) => "Not preceded by:",
    }
}

pub(crate) fn one_of_label(lang: Lang) -> &'static str {
    lang.of("其一", "One of")
}

pub(crate) fn none_of_label(lang: Lang) -> &'static str {
    lang.of("没有其一", "None of")
}

pub(crate) fn empty_label(lang: Lang) -> &'static str {
    lang.of("空", "Empty")
}
