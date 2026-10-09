use crate::ast::*;

pub(crate) const LITERAL_ESCAPES: [char; 14] = [
    '|', '\\', '{', '}', '(', ')', '[', ']', '^', '$', '+', '*', '?', '.',
];

pub(crate) struct Ser {
    pub(crate) out: String,
    pub(crate) head: NodeId,
    pub(crate) tail: NodeId,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl Ser {
    pub(crate) fn plain() -> Self {
        Self {
            out: String::new(),
            head: 0,
            tail: 0,
            start: 0,
            end: 0,
        }
    }

    pub(crate) fn tracking(head: NodeId, tail: NodeId) -> Self {
        Self {
            head,
            tail,
            ..Self::plain()
        }
    }

    pub(crate) fn nodes(&mut self, nodes: &[ENode]) {
        for node in nodes {
            self.node(node);
        }
    }

    pub(crate) fn node(&mut self, node: &ENode) {
        if node.id != 0 && node.id == self.head {
            self.start = self.out.len();
        }
        self.kind(node);
        if node.accepts_quantifier()
            && let Some(q) = &node.quantifier
        {
            self.quantifier(q);
        }
        if node.id != 0 && node.id == self.tail {
            self.end = self.out.len();
        }
    }

    pub(crate) fn kind(&mut self, node: &ENode) {
        match &node.kind {
            EKind::Character {
                kind,
                value,
                ranges,
                negate,
            } => match kind {
                CharKind::String => {
                    for c in value.chars() {
                        if LITERAL_ESCAPES.contains(&c) {
                            self.out.push('\\');
                        }
                        self.out.push(c);
                    }
                }

                CharKind::Class => self.out.push_str(value),
                CharKind::Ranges => {
                    self.out.push_str(if *negate { "[^" } else { "[" });
                    let last = ranges.len().saturating_sub(1);
                    for (i, (from, to)) in ranges.iter().enumerate() {
                        let mut f = escape_range_edge(from);
                        let mut t = escape_range_edge(to);

                        if i != 0 && i != last {
                            if f == "-" {
                                f = "\\-".to_string();
                            }
                            if t == "-" {
                                t = "\\-".to_string();
                            }
                        }
                        if f != t {
                            self.out.push_str(&f);
                            self.out.push('-');
                            self.out.push_str(&t);
                        } else {
                            self.out.push_str(&f);
                        }
                    }
                    self.out.push(']');
                }
            },
            EKind::BackReference { reference } => {
                let numeric =
                    !reference.is_empty() && reference.chars().all(|c| c.is_ascii_digit());
                if numeric {
                    self.out.push('\\');
                    self.out.push_str(reference);
                } else {
                    self.out.push_str("\\k<");
                    self.out.push_str(reference);
                    self.out.push('>');
                }
            }
            EKind::Boundary { kind, negate } => match kind {
                BoundaryKind::Beginning => self.out.push('^'),
                BoundaryKind::End => self.out.push('$'),
                BoundaryKind::Word => self.out.push_str(if *negate { "\\B" } else { "\\b" }),
            },
            EKind::Group {
                kind,
                name,
                children,
            } => {
                match kind {
                    EGroupKind::Capturing => self.out.push('('),
                    EGroupKind::NonCapturing => self.out.push_str("(?:"),
                    EGroupKind::NamedCapturing => {
                        self.out.push_str("(?<");
                        self.out.push_str(name);
                        self.out.push('>');
                    }
                }
                self.nodes(children);
                self.out.push(')');
            }
            EKind::LookAround {
                kind,
                negate,
                children,
            } => {
                self.out.push_str(match (kind, negate) {
                    (LookKind::Lookahead, false) => "(?=",
                    (LookKind::Lookahead, true) => "(?!",
                    (LookKind::Lookbehind, false) => "(?<=",
                    (LookKind::Lookbehind, true) => "(?<!",
                });
                self.nodes(children);
                self.out.push(')');
            }
            EKind::Choice { branches } => {
                for (i, branch) in branches.iter().enumerate() {
                    if i > 0 {
                        self.out.push('|');
                    }
                    self.nodes(branch);
                }
            }
        }
    }

    pub(crate) fn quantifier(&mut self, q: &Quantifier) {
        match q.kind {
            QuantKind::Star => self.out.push('*'),
            QuantKind::Plus => self.out.push('+'),
            QuantKind::Question => self.out.push('?'),
            QuantKind::Custom => {
                if q.min == q.max {
                    self.out.push_str(&format!("{{{}}}", q.min));
                } else if q.infinite() {
                    self.out.push_str(&format!("{{{},}}", q.min));
                } else {
                    self.out.push_str(&format!("{{{},{}}}", q.min, q.max));
                }
            }
        }
        if !q.greedy {
            self.out.push('?');
        }
    }
}

fn escape_range_edge(s: &str) -> String {
    if s == "]" || s == "\\" {
        format!("\\{s}")
    } else {
        s.to_string()
    }
}
