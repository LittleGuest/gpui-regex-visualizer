use std::ops::Range;

use gpui_kit::{HighlightStyle, Hsla, StyledText};

pub struct HighlightPalette {
    pub boolean: Hsla,
}

impl HighlightPalette {
    pub fn default_light() -> Self {
        Self {
            boolean: gpui_kit::hsla(0.0, 0.65, 0.5, 1.0),
        }
    }
}

pub struct HighlightRange {
    pub range: Range<usize>,
    pub color: Hsla,
}

fn to_highlight_styles(ranges: &[HighlightRange]) -> Vec<(Range<usize>, HighlightStyle)> {
    ranges
        .iter()
        .map(|r| {
            (
                r.range.clone(),
                HighlightStyle {
                    color: Some(r.color),
                    ..Default::default()
                },
            )
        })
        .collect()
}

pub fn styled_text(text: &str, ranges: Vec<HighlightRange>) -> StyledText {
    let highlights = to_highlight_styles(&ranges);
    StyledText::new(text.to_string()).with_highlights(highlights)
}
