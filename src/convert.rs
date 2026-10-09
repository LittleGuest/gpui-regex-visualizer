use crate::{ast::*, i18n::LITERAL_LABELS};

pub(crate) fn is_bare_literal(c: char) -> bool {
    !LITERAL_LABELS.iter().any(|(ch, _, _)| *ch == c)
}

pub(crate) fn convert_ast(ast: &regex_syntax::ast::Ast, src: &str, id_gen: &mut IdGen) -> ENode {
    use regex_syntax::ast::Ast;

    match ast {
        Ast::Empty(_) => e_string(id_gen.next(), ""),
        Ast::Flags(_) => e_class(id_gen.next(), ast_text(src, ast_span(ast))),
        Ast::Literal(lit) => {
            if is_bare_literal(lit.c) {
                e_string(id_gen.next(), lit.c.to_string())
            } else {
                e_class(id_gen.next(), ast_text(src, &lit.span))
            }
        }
        Ast::Dot(_) => e_class(id_gen.next(), "."),
        Ast::Assertion(a) => {
            use regex_syntax::ast::AssertionKind as K;
            let (kind, negate) = match a.kind {
                K::StartLine | K::StartText => (BoundaryKind::Beginning, false),
                K::EndLine | K::EndText => (BoundaryKind::End, false),
                K::WordBoundary | K::WordBoundaryStart | K::WordBoundaryStartAngle => {
                    (BoundaryKind::Word, false)
                }
                K::NotWordBoundary | K::WordBoundaryEnd | K::WordBoundaryEndAngle => {
                    (BoundaryKind::Word, true)
                }
                _ => (BoundaryKind::Word, false),
            };
            e_boundary(id_gen.next(), kind, negate)
        }
        Ast::ClassUnicode(c) => e_class(id_gen.next(), ast_text(src, &c.span)),
        Ast::ClassPerl(c) => e_class(id_gen.next(), ast_text(src, &c.span)),
        Ast::ClassBracketed(c) => {
            let mut ranges: Vec<(String, String)> = Vec::new();
            collect_ranges(&c.kind, &mut ranges);
            e_ranges(id_gen.next(), ranges, c.negated)
        }
        Ast::Repetition(rep) => {
            let mut node = convert_ast(&rep.ast, src, id_gen);

            let greedy = rep.greedy;
            node.quantifier = Some(match &rep.op.kind {
                regex_syntax::ast::RepetitionKind::ZeroOrMore => Quantifier {
                    kind: QuantKind::Star,
                    min: 0,
                    max: Quantifier::INF,
                    greedy,
                },
                regex_syntax::ast::RepetitionKind::OneOrMore => Quantifier {
                    kind: QuantKind::Plus,
                    min: 1,
                    max: Quantifier::INF,
                    greedy,
                },
                regex_syntax::ast::RepetitionKind::ZeroOrOne => Quantifier {
                    kind: QuantKind::Question,
                    min: 0,
                    max: 1,
                    greedy,
                },
                regex_syntax::ast::RepetitionKind::Range(range) => {
                    let (min, max) = match range {
                        regex_syntax::ast::RepetitionRange::Exactly(n) => (*n, *n),
                        regex_syntax::ast::RepetitionRange::AtLeast(n) => (*n, Quantifier::INF),
                        regex_syntax::ast::RepetitionRange::Bounded(a, b) => (*a, *b),
                    };
                    Quantifier {
                        kind: QuantKind::Custom,
                        min,
                        max,
                        greedy,
                    }
                }
            });
            node
        }
        Ast::Group(g) => {
            let children = convert_seq(&g.ast, src, id_gen);
            match &g.kind {
                regex_syntax::ast::GroupKind::CaptureIndex(_) => {
                    e_group(id_gen.next(), EGroupKind::Capturing, "", children)
                }

                regex_syntax::ast::GroupKind::CaptureName { name, .. } => e_group(
                    id_gen.next(),
                    EGroupKind::NamedCapturing,
                    &name.name,
                    children,
                ),
                regex_syntax::ast::GroupKind::NonCapturing(_) => {
                    e_group(id_gen.next(), EGroupKind::NonCapturing, "", children)
                }
            }
        }
        Ast::Alternation(a) => {
            let branches = a.asts.iter().map(|c| convert_seq(c, src, id_gen)).collect();
            e_choice(id_gen.next(), branches)
        }

        Ast::Concat(c) => {
            let children = if c.asts.len() == 1 {
                convert_seq(&c.asts[0], src, id_gen)
            } else {
                let mut all = Vec::new();
                for child in &c.asts {
                    all.extend(convert_seq(child, src, id_gen));
                }
                all
            };
            e_group(id_gen.next(), EGroupKind::NonCapturing, "", children)
        }
    }
}

pub(crate) fn convert_seq(
    ast: &regex_syntax::ast::Ast,
    src: &str,
    id_gen: &mut IdGen,
) -> Vec<ENode> {
    use regex_syntax::ast::Ast;

    if let Ast::Concat(c) = ast {
        let mut out: Vec<ENode> = Vec::new();
        let mut pending = String::new();
        for child in &c.asts {
            match child {
                Ast::Empty(_) => {}
                Ast::Literal(lit) if is_bare_literal(lit.c) => pending.push(lit.c),
                _ => {
                    if !pending.is_empty() {
                        out.push(e_string(id_gen.next(), std::mem::take(&mut pending)));
                    }
                    out.push(convert_ast(child, src, id_gen));
                }
            }
        }
        if !pending.is_empty() {
            out.push(e_string(id_gen.next(), pending));
        }
        return out;
    }

    vec![convert_ast(ast, src, id_gen)]
}

fn ast_span(ast: &regex_syntax::ast::Ast) -> &regex_syntax::ast::Span {
    use regex_syntax::ast::Ast;
    match ast {
        Ast::Empty(s) | Ast::Dot(s) => s,
        Ast::Flags(f) => &f.span,
        Ast::Literal(l) => &l.span,
        Ast::Assertion(l) => &l.span,
        Ast::ClassUnicode(l) => &l.span,
        Ast::ClassPerl(l) => &l.span,
        Ast::ClassBracketed(l) => &l.span,
        Ast::Repetition(r) => &r.span,
        Ast::Group(g) => &g.span,
        Ast::Alternation(a) => &a.span,
        Ast::Concat(c) => &c.span,
    }
}

fn collect_ranges(node: &regex_syntax::ast::ClassSet, out: &mut Vec<(String, String)>) {
    use regex_syntax::ast::ClassSet;
    match node {
        ClassSet::Item(item) => collect_ranges_item(item, out),
        ClassSet::BinaryOp(op) => {
            collect_ranges(&op.lhs, out);
            collect_ranges(&op.rhs, out);
        }
    }
}

fn collect_ranges_item(item: &regex_syntax::ast::ClassSetItem, out: &mut Vec<(String, String)>) {
    use regex_syntax::ast::ClassSetItem;
    match item {
        ClassSetItem::Literal(lit) => out.push((lit.c.to_string(), lit.c.to_string())),
        ClassSetItem::Range(r) => {
            out.push((r.start.c.to_string(), r.end.c.to_string()));
        }
        ClassSetItem::Bracketed(b) => collect_ranges(&b.kind, out),
        ClassSetItem::Perl(p) => {
            use regex_syntax::ast::ClassPerlKind as K;
            let raw = match p.kind {
                K::Digit => r"\d",
                K::Space => r"\s",
                K::Word => r"\w",
            };
            out.push((raw.to_string(), raw.to_string()));
        }
        ClassSetItem::Unicode(u) => {
            let name = format!("\\p{}", unicode_class_name(&u.kind));
            out.push((name.clone(), name));
        }
        ClassSetItem::Ascii(a) => {
            use regex_syntax::ast::ClassAsciiKind as K;
            let raw = match a.kind {
                K::Alnum | K::Word => r"\w",
                K::Digit => r"\d",
                K::Space => r"\s",
                _ => "class",
            };
            out.push((raw.to_string(), raw.to_string()));
        }
        ClassSetItem::Union(u) => {
            for i in &u.items {
                collect_ranges_item(i, out);
            }
        }
        ClassSetItem::Empty(_) => {}
    }
}

pub(crate) fn ast_text(src: &str, span: &regex_syntax::ast::Span) -> String {
    src.get(span.start.offset..span.end.offset)
        .unwrap_or("")
        .to_string()
}

pub(crate) fn unicode_class_name(kind: &regex_syntax::ast::ClassUnicodeKind) -> String {
    use regex_syntax::ast::ClassUnicodeKind;
    match kind {
        ClassUnicodeKind::OneLetter(c) => c.to_string(),
        ClassUnicodeKind::Named(n) => n.to_string(),
        ClassUnicodeKind::NamedValue { name, value, .. } => format!("{name}={value}"),
    }
}
