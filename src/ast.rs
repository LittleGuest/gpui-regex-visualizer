pub(crate) type NodeId = u32;

#[derive(Default)]
pub(crate) struct IdGen(pub(crate) u32);

impl IdGen {
    pub(crate) fn next(&mut self) -> NodeId {
        self.0 += 1;
        self.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CharKind {
    String,
    Class,
    Ranges,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum EGroupKind {
    Capturing,
    NonCapturing,
    NamedCapturing,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LookKind {
    Lookahead,
    Lookbehind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BoundaryKind {
    Beginning,
    End,
    Word,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum QuantKind {
    Star,
    Plus,
    Question,
    Custom,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Quantifier {
    pub(crate) kind: QuantKind,
    pub(crate) min: u32,
    pub(crate) max: u32,
    pub(crate) greedy: bool,
}

impl Quantifier {
    pub(crate) const INF: u32 = u32::MAX;

    pub(crate) fn infinite(&self) -> bool {
        self.max == Self::INF
    }
}

#[derive(Clone, Debug)]
pub(crate) enum EKind {
    Character {
        kind: CharKind,
        value: String,
        ranges: Vec<(String, String)>,
        negate: bool,
    },
    BackReference {
        reference: String,
    },
    Boundary {
        kind: BoundaryKind,
        negate: bool,
    },
    Group {
        kind: EGroupKind,
        name: String,
        children: Vec<ENode>,
    },
    LookAround {
        kind: LookKind,
        negate: bool,
        children: Vec<ENode>,
    },
    Choice {
        branches: Vec<Vec<ENode>>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct ENode {
    pub(crate) id: NodeId,
    pub(crate) quantifier: Option<Quantifier>,
    pub(crate) kind: EKind,
}

impl ENode {
    pub(crate) fn new(id: NodeId, kind: EKind) -> Self {
        Self {
            id,
            quantifier: None,
            kind,
        }
    }

    pub(crate) fn accepts_quantifier(&self) -> bool {
        matches!(
            self.kind,
            EKind::Character { .. } | EKind::Group { .. } | EKind::BackReference { .. }
        )
    }

    pub(crate) fn children(&self) -> Option<&Vec<ENode>> {
        match &self.kind {
            EKind::Group { children, .. } | EKind::LookAround { children, .. } => Some(children),
            _ => None,
        }
    }

    pub(crate) fn dashed(&self) -> bool {
        match &self.kind {
            EKind::Character { negate, .. } => *negate,
            EKind::Boundary { kind, negate } => matches!(kind, BoundaryKind::Word) && *negate,
            EKind::LookAround { negate, .. } => *negate,
            _ => false,
        }
    }
}

pub(crate) fn e_string(id: NodeId, value: impl Into<String>) -> ENode {
    ENode::new(
        id,
        EKind::Character {
            kind: CharKind::String,
            value: value.into(),
            ranges: Vec::new(),
            negate: false,
        },
    )
}

pub(crate) fn e_class(id: NodeId, value: impl Into<String>) -> ENode {
    ENode::new(
        id,
        EKind::Character {
            kind: CharKind::Class,
            value: value.into(),
            ranges: Vec::new(),
            negate: false,
        },
    )
}

pub(crate) fn e_ranges(id: NodeId, ranges: Vec<(String, String)>, negate: bool) -> ENode {
    ENode::new(
        id,
        EKind::Character {
            kind: CharKind::Ranges,
            value: String::new(),
            ranges,
            negate,
        },
    )
}

#[allow(dead_code)]
pub(crate) fn e_backref(id: NodeId, reference: impl Into<String>) -> ENode {
    ENode::new(
        id,
        EKind::BackReference {
            reference: reference.into(),
        },
    )
}

pub(crate) fn e_boundary(id: NodeId, kind: BoundaryKind, negate: bool) -> ENode {
    ENode::new(id, EKind::Boundary { kind, negate })
}

pub(crate) fn e_group(
    id: NodeId,
    kind: EGroupKind,
    name: impl Into<String>,
    children: Vec<ENode>,
) -> ENode {
    ENode::new(
        id,
        EKind::Group {
            kind,
            name: name.into(),
            children,
        },
    )
}

pub(crate) fn e_look(id: NodeId, kind: LookKind, negate: bool, children: Vec<ENode>) -> ENode {
    ENode::new(
        id,
        EKind::LookAround {
            kind,
            negate,
            children,
        },
    )
}

pub(crate) fn e_choice(id: NodeId, branches: Vec<Vec<ENode>>) -> ENode {
    ENode::new(id, EKind::Choice { branches })
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ERoot {
    pub(crate) body: Vec<ENode>,
}
