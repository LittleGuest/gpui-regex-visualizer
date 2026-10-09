use gpui_kit::{Hsla, component::ActiveTheme as _, rgb};

use crate::prim::Prim;

pub(crate) const NODE_H: f32 = 28.0;

pub(crate) const NODE_R: f32 = 5.0;

pub(crate) const NODE_PAD_X: f32 = 10.0;

pub(crate) const NODE_FS: f32 = 16.0;

pub(crate) const LABEL_FS: f32 = 14.0;

pub(crate) const LABEL_H: f32 = 21.0;

pub(crate) const LABEL_BOX: f32 = 16.0;

pub(crate) const ICON_W: f32 = 18.0;

pub(crate) const LINK: f32 = 25.0;

pub(crate) const BRANCH_INDENT: f32 = 25.0;

pub(crate) const BRANCH_R: f32 = 5.0;

pub(crate) const ROW_GAP: f32 = 15.0;

pub(crate) const BRANCH_PAD_V: f32 = 10.0;

pub(crate) const GROUP_PAD_H: f32 = 25.0;

pub(crate) const GROUP_PAD_V: f32 = 15.0;

pub(crate) const ROOT_PAD: f32 = 50.0;

pub(crate) const MIN_W: f32 = 20.0;
pub(crate) const MIN_H: f32 = 26.0;

pub(crate) const INPUT_H: f32 = 36.0;

pub(crate) const TOOLBAR_H: f32 = 28.0;

pub(crate) const INPUT_BLOCK_H: f32 = 88.0;

pub(crate) const MARK_R: f32 = 5.0;

pub(crate) const STROKE: f32 = 1.5;

pub(crate) const ERROR_RED_LIGHT: u32 = 0xdc2626;
pub(crate) const ERROR_RED_DARK: u32 = 0xf87171;

pub(crate) const SELECT_BLUE_LIGHT: u32 = 0x3b82f6;
pub(crate) const SELECT_ALPHA: f32 = 0.3;

pub(crate) const SELECT_ALPHA_DARK: f32 = 0.45;

/// Resolved colors for the SVG-like canvas. The canvas paints through raw
/// `window.paint_path`, which reads no theme of its own, so the palette is
/// resolved once per frame from the live GPUI theme.
pub(crate) struct CanvasTheme {
    stroke: Hsla,
    group: Hsla,
    fg: Hsla,
    select: Hsla,
    select_alpha: f32,
    error: Hsla,
}

impl CanvasTheme {
    pub(crate) fn of(is_dark: bool, cx: &gpui_kit::App) -> Self {
        let t = cx.theme();

        Self {
            stroke: t.border,
            // Group frames sit behind everything and read as furniture, not
            // ink — muted keeps them subordinate to the node strokes.
            group: t.muted_foreground,
            fg: t.foreground,
            select: t.primary,
            select_alpha: if is_dark {
                SELECT_ALPHA_DARK
            } else {
                SELECT_ALPHA
            },
            error: if is_dark {
                Hsla::from(rgb(ERROR_RED_DARK))
            } else {
                Hsla::from(rgb(ERROR_RED_LIGHT))
            },
        }
    }

    /// Node strokes ride the theme border; the neutral sentinel stored in the
    /// layout prims is replaced here so a theme switch repaints correctly.
    pub(crate) fn stroke_color(&self, _sentinel: u32) -> Hsla {
        self.stroke
    }

    pub(crate) fn group_color(&self) -> Hsla {
        self.group
    }

    pub(crate) fn text_color(&self, _sentinel: u32) -> Hsla {
        self.fg
    }

    pub(crate) fn selection_color(&self) -> Hsla {
        Hsla {
            a: self.select_alpha,
            ..self.select
        }
    }

    pub(crate) fn error_color(&self) -> Hsla {
        self.error
    }
}

pub(crate) fn select_highlight(x: f32, y: f32, w: f32, h: f32) -> Prim {
    Prim::Highlight {
        x,
        y,
        w,
        h,
        r: NODE_R,
    }
}

pub(crate) const FLAGS: [(usize, char, &str, &str); 4] = [
    (0, 'g', "全局搜索", "Global search"),
    (1, 'i', "忽略大小写", "Case-insensitive"),
    (2, 'm', "多行", "Multi-line"),
    (3, 's', "允许 . 匹配换行符", "Allows . to match newline"),
];

pub(crate) const SAMPLES: [(&str, &str, &str); 20] = [
    ("1. 整数", "1. Whole Numbers", r"^\d+$"),
    ("2. 小数", "2. Decimal Numbers", r"^\d*\.\d+$"),
    (
        "3. 整数 + 小数",
        "3. Whole + Decimal Numbers",
        r"^\d*(\.\d+)?$",
    ),
    (
        "4. 正负 整数 + 小数",
        "4. Negative, Positive Whole + Decimal Numbers",
        r"^-?\d*(\.\d+)?$",
    ),
    (
        "5. Url",
        "5. Url",
        r"^https?:\/\/(www\.)?[-a-zA-Z0-9@:%._\+~#=]{2,256}\.[a-z]{2,6}\b([-a-zA-Z0-9@:%_\+.~#()?&//=]*)$",
    ),
    (
        "6. 日期格式 YYYY-MM-dd",
        "6. Date Format YYYY-MM-dd",
        r"^[12]\d{3}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$",
    ),
    (
        "7. 时间格式 HH:mm:ss",
        "7. Time Format HH:mm:ss",
        r"^([01]\d|2[0-3]):[0-5]\d:[0-5]\d$",
    ),
    (
        "8. 日期 + 时间 YYYY-MM-dd HH:mm:ss",
        "8. Date + Time YYYY-MM-dd HH:mm:ss",
        r"^[12]\d{3}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])\s([01]\d|2[0-3]):[0-5]\d:[0-5]\d$",
    ),
    (
        "9. 手机号（中国大陆）",
        "9. Mobile Number (Chinese Mainland)",
        r"^1[3-9]\d{9}$",
    ),
    (
        "10. 身份证号（18 位）",
        "10. ID Card Number (18 digits)",
        r"^[1-9]\d{5}(18|19|20)\d{2}(0[1-9]|1[0-2])(0[1-9]|[12]\d|3[01])\d{3}[\dXx]$",
    ),
    ("11. 邮政编码", "11. Postal Code", r"^[1-9]\d{5}$"),
    (
        "12. 中文汉字",
        "12. Chinese Characters",
        r"^[\u4e00-\u9fa5]+$",
    ),
    (
        "13. 中文姓名",
        "13. Chinese Name",
        r"^[\u4e00-\u9fa5]{2,4}$",
    ),
    (
        "14. 用户名",
        "14. Username",
        r"^[a-zA-Z][a-zA-Z0-9_]{3,15}$",
    ),
    (
        "15. 密码强度（含大小写、数字、8 位以上）",
        "15. Strong Password (upper, lower, digit, 8+)",
        r"^[a-zA-Z]\w{7,15}$",
    ),
    (
        "16. 电子邮箱",
        "16. Email Address",
        r"^[\w.+-]+@[\w-]+(\.[\w-]+)+$",
    ),
    (
        "17. IPv4 地址",
        "17. IPv4 Address",
        r"^((25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)\.){3}(25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)$",
    ),
    (
        "18. 车牌号（含新能源）",
        "18. License Plate (incl. new energy)",
        r"^[京津冀晋蒙辽吉黑沪苏浙皖闽赣鲁豫鄂湘粤桂琼渝川贵云藏陕甘青宁新][A-HJ-NP-Z][A-HJ-NP-Z0-9]{5,6}$",
    ),
    (
        "19. 金额（两位小数）",
        "19. Amount (two decimals)",
        r"^\d+(\.\d{1,2})?$",
    ),
    (
        "20. 中英文混合文本",
        "20. Mixed Chinese + Latin Text",
        r"^[\u4e00-\u9fa5a-zA-Z0-9]+$",
    ),
];

pub(crate) const PANEL_TABS: [(&str, &str); 4] = [
    ("图例", "Legends"),
    ("编辑", "Edit"),
    ("测试", "Test"),
    ("样例", "Samples"),
];
pub(crate) const TAB_LEGEND: usize = 0;
pub(crate) const TAB_EDIT: usize = 1;
pub(crate) const TAB_TEST: usize = 2;
