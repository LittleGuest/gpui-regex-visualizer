use crate::ast::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ContentSpec {
    String {
        value: String,
    },
    Class {
        value: String,
    },
    Ranges {
        ranges: Vec<(String, String)>,
        negate: bool,
    },
    BackReference {
        reference: String,
    },
    WordBoundary {
        negate: bool,
    },
    Beginning,
    End,
}

pub(crate) const CONTENT_TYPES: [(&str, &str, &str); 7] = [
    ("string", "简单字符串", "Simple string"),
    ("class", "字符类", "Character class"),
    ("ranges", "字符范围", "Character range"),
    ("backReference", "反向引用", "Back reference"),
    ("beginningAssertion", "开始断言", "Beginning Assertion"),
    ("endAssertion", "结束断言", "End Assertion"),
    (
        "wordBoundaryAssertion",
        "单词边界断言",
        "Word Boundary Assertion",
    ),
];

impl ContentSpec {
    pub(crate) fn key(&self) -> &'static str {
        match self {
            ContentSpec::String { .. } => "string",
            ContentSpec::Class { .. } => "class",
            ContentSpec::Ranges { .. } => "ranges",
            ContentSpec::BackReference { .. } => "backReference",
            ContentSpec::WordBoundary { .. } => "wordBoundaryAssertion",
            ContentSpec::Beginning => "beginningAssertion",
            ContentSpec::End => "endAssertion",
        }
    }

    pub(crate) fn for_key(key: &str) -> ContentSpec {
        match key {
            "string" => ContentSpec::String {
                value: String::new(),
            },
            "class" => ContentSpec::Class {
                value: String::new(),
            },
            "ranges" => ContentSpec::Ranges {
                ranges: vec![(String::new(), String::new())],
                negate: false,
            },
            "backReference" => ContentSpec::BackReference {
                reference: "1".to_string(),
            },
            "beginningAssertion" => ContentSpec::Beginning,
            "endAssertion" => ContentSpec::End,
            _ => ContentSpec::WordBoundary { negate: false },
        }
    }

    pub(crate) fn to_kind(&self) -> EKind {
        match self {
            ContentSpec::String { value } => EKind::Character {
                kind: CharKind::String,
                value: value.clone(),
                ranges: Vec::new(),
                negate: false,
            },
            ContentSpec::Class { value } => EKind::Character {
                kind: CharKind::Class,
                value: value.clone(),
                ranges: Vec::new(),
                negate: false,
            },
            ContentSpec::Ranges { ranges, negate } => EKind::Character {
                kind: CharKind::Ranges,
                value: String::new(),
                ranges: ranges.clone(),
                negate: *negate,
            },
            ContentSpec::BackReference { reference } => EKind::BackReference {
                reference: reference.clone(),
            },
            ContentSpec::WordBoundary { negate } => EKind::Boundary {
                kind: BoundaryKind::Word,
                negate: *negate,
            },
            ContentSpec::Beginning => EKind::Boundary {
                kind: BoundaryKind::Beginning,
                negate: false,
            },
            ContentSpec::End => EKind::Boundary {
                kind: BoundaryKind::End,
                negate: false,
            },
        }
    }

    pub(crate) fn from_node(node: &ENode) -> Option<ContentSpec> {
        match &node.kind {
            EKind::Character {
                kind,
                value,
                ranges,
                negate,
            } => Some(match kind {
                CharKind::String => ContentSpec::String {
                    value: value.clone(),
                },
                CharKind::Class => ContentSpec::Class {
                    value: value.clone(),
                },
                CharKind::Ranges => ContentSpec::Ranges {
                    ranges: ranges.clone(),
                    negate: *negate,
                },
            }),
            EKind::BackReference { reference } => Some(ContentSpec::BackReference {
                reference: reference.clone(),
            }),
            EKind::Boundary { kind, negate } => Some(match kind {
                BoundaryKind::Word => ContentSpec::WordBoundary { negate: *negate },
                BoundaryKind::Beginning => ContentSpec::Beginning,
                BoundaryKind::End => ContentSpec::End,
            }),
            _ => None,
        }
    }
}

pub(crate) fn content_type_keys(info: &SelectionInfo, capture_count: usize) -> Vec<&'static str> {
    let current = info.content.as_ref().map(ContentSpec::key);
    let mut keys: Vec<&'static str> = vec!["string", "class", "ranges", "wordBoundaryAssertion"];
    if capture_count > 0 || current == Some("backReference") {
        keys.push("backReference");
    }
    if info.first || current == Some("beginningAssertion") {
        keys.push("beginningAssertion");
    }
    if info.last || current == Some("endAssertion") {
        keys.push("endAssertion");
    }
    keys
}

pub(crate) const QUANT_OPTIONS: [(&str, &str, &str, &str); 5] = [
    ("non", "1 (默认)", "1 (default)", ""),
    ("?", "0 or 1", "0 or 1", "?"),
    ("*", "0 or more", "0 or more", "*"),
    ("+", "1 or more", "1 or more", "+"),
    ("custom", "自定义", "custom", "{min,max}"),
];

pub(crate) fn quant_key(q: Option<&Quantifier>) -> &'static str {
    match q.map(|q| q.kind) {
        None => "non",
        Some(QuantKind::Question) => "?",
        Some(QuantKind::Star) => "*",
        Some(QuantKind::Plus) => "+",
        Some(QuantKind::Custom) => "custom",
    }
}

pub(crate) fn quant_for_key(key: &str, greedy: bool) -> Option<Quantifier> {
    Some(match key {
        "?" => Quantifier {
            kind: QuantKind::Question,
            min: 0,
            max: 1,
            greedy,
        },
        "*" => Quantifier {
            kind: QuantKind::Star,
            min: 0,
            max: Quantifier::INF,
            greedy,
        },
        "+" => Quantifier {
            kind: QuantKind::Plus,
            min: 1,
            max: Quantifier::INF,
            greedy,
        },
        "custom" => Quantifier {
            kind: QuantKind::Custom,
            min: 1,
            max: 1,
            greedy,
        },
        _ => return None,
    })
}

#[derive(Clone, Debug)]
pub(crate) struct SelectionInfo {
    pub(crate) ids: Vec<NodeId>,
    pub(crate) id: NodeId,

    pub(crate) pattern: String,

    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) content: Option<ContentSpec>,
    pub(crate) group: Option<(EGroupKind, String)>,

    pub(crate) has_quantifier: bool,
    pub(crate) quantifier: Option<Quantifier>,
    pub(crate) lookaround: Option<(LookKind, bool)>,

    pub(crate) first: bool,
    pub(crate) last: bool,
    pub(crate) single: bool,
}

impl ERoot {
    pub(crate) fn selection(&self, ids: &[NodeId]) -> SelectionInfo {
        let id = ids.first().copied().unwrap_or(0);
        let (pattern, start, end) = match (ids.first(), ids.last()) {
            (Some(head), Some(tail)) => self.pattern_with_span(*head, *tail),
            _ => (self.to_pattern(), 0, 0),
        };
        let single = ids.len() == 1;
        let node = if single { self.node(id) } else { None };
        let group = node.and_then(|n| match &n.kind {
            EKind::Group { kind, name, .. } => Some((*kind, name.clone())),
            _ => None,
        });
        let lookaround = node.and_then(|n| match &n.kind {
            EKind::LookAround { kind, negate, .. } => Some((*kind, *negate)),
            _ => None,
        });
        SelectionInfo {
            ids: ids.to_vec(),
            id,
            pattern,
            start,
            end,
            content: node.and_then(ContentSpec::from_node),
            group,
            has_quantifier: node.is_some_and(|n| n.accepts_quantifier()),
            quantifier: node.and_then(|n| n.quantifier),
            lookaround,
            first: self.is_first(id),
            last: self.is_last(id),
            single,
        }
    }
}
