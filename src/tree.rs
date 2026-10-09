use crate::{ast::*, content::ContentSpec, i18n::*, ser::*};

impl ERoot {
    pub(crate) fn to_pattern(&self) -> String {
        let mut ser = Ser::plain();
        ser.nodes(&self.body);
        ser.out
    }

    pub(crate) fn pattern_with_span(&self, head: NodeId, tail: NodeId) -> (String, usize, usize) {
        let mut ser = Ser::tracking(head, tail);
        ser.nodes(&self.body);
        (ser.out, ser.start, ser.end)
    }

    pub(crate) fn find(&self, id: NodeId) -> Option<(Vec<Step>, usize)> {
        find_path(&self.body, id)
    }

    pub(crate) fn node(&self, id: NodeId) -> Option<&ENode> {
        self.find(id)
            .map(|(path, index)| &seq_at(&self.body, &path)[index])
    }

    #[allow(dead_code)]
    pub(crate) fn slice(&self, ids: &[NodeId]) -> Option<(&[ENode], usize, usize)> {
        if ids.is_empty() {
            return None;
        }
        let (path, index) = self.find(ids[0])?;
        let seq = seq_at(&self.body, &path);
        let len = ids.len().min(seq.len().saturating_sub(index));
        Some((seq, index, len))
    }

    pub(crate) fn is_first(&self, id: NodeId) -> bool {
        self.body.first().is_some_and(|n| n.id == id)
    }

    pub(crate) fn is_last(&self, id: NodeId) -> bool {
        self.body.last().is_some_and(|n| n.id == id)
    }

    pub(crate) fn insert_around(&mut self, ids: &[NodeId], mode: InsertMode, id_gen: &mut IdGen) {
        if ids.is_empty() {
            return;
        }
        let Some((path, index)) = self.find(ids[0]) else {
            return;
        };
        let fresh = e_string(id_gen.next(), "");
        let body = &mut self.body;
        let seq = seq_at_mut(body, &path);
        let len = ids.len().min(seq.len().saturating_sub(index));

        match mode {
            InsertMode::Before => seq.insert(index, fresh),
            InsertMode::After => seq.insert(index + len, fresh),
            InsertMode::Parallel => {
                let single_choice = len == 1 && matches!(seq[index].kind, EKind::Choice { .. });
                let whole_branch =
                    index == 0 && len == seq.len() && matches!(path.last(), Some(Step::Branch(..)));
                if single_choice {
                    if let EKind::Choice { branches } = &mut seq[index].kind {
                        branches.push(vec![fresh]);
                    }
                } else if whole_branch {
                    let parent_path = &path[..path.len() - 1];
                    let ci = match path[path.len() - 1] {
                        Step::Branch(ci, _) => ci,
                        Step::Children(_) => unreachable!(),
                    };
                    let parent = seq_at_mut(&mut self.body, parent_path);
                    if let EKind::Choice { branches } = &mut parent[ci].kind {
                        branches.push(vec![fresh]);
                    }
                } else {
                    let taken: Vec<ENode> = seq.splice(index..index + len, []).collect();
                    let choice = e_choice(id_gen.next(), vec![taken, vec![fresh]]);
                    seq.insert(index, choice);
                }
            }
        }
    }

    pub(crate) fn set_content(
        &mut self,
        id: NodeId,
        spec: &ContentSpec,
        id_gen: &mut IdGen,
    ) -> NodeId {
        let Some((path, index)) = self.find(id) else {
            return id;
        };
        let fresh = e_string(id_gen.next(), "");
        let body = &mut self.body;
        let seq = seq_at_mut(body, &path);
        let quantifier = seq[index].quantifier;
        let new_kind = spec.to_kind();

        if let ContentSpec::String { value } = spec
            && value.chars().count() > 1
            && quantifier.is_some()
        {
            let inner = ENode {
                id,
                quantifier: None,
                kind: new_kind,
            };
            let group_id = id_gen.next();
            seq[index] = ENode {
                id: group_id,
                quantifier,
                kind: EKind::Group {
                    kind: EGroupKind::NonCapturing,
                    name: String::new(),
                    children: vec![inner],
                },
            };
            return group_id;
        }
        let _ = fresh;
        seq[index].kind = new_kind;
        id
    }

    pub(crate) fn set_quantifier(
        &mut self,
        id: NodeId,
        quantifier: Option<Quantifier>,
        id_gen: &mut IdGen,
    ) -> NodeId {
        let Some((path, index)) = self.find(id) else {
            return id;
        };
        let body = &mut self.body;
        let seq = seq_at_mut(body, &path);
        let multi = matches!(
            &seq[index].kind,
            EKind::Character { kind: CharKind::String, value, .. } if value.chars().count() > 1
        );
        if multi {
            let child = seq[index].clone();
            let group_id = id_gen.next();
            seq[index] = ENode {
                id: group_id,
                quantifier,
                kind: EKind::Group {
                    kind: EGroupKind::NonCapturing,
                    name: String::new(),
                    children: vec![child],
                },
            };
            group_id
        } else if seq[index].accepts_quantifier() {
            seq[index].quantifier = quantifier;
            id
        } else {
            id
        }
    }

    pub(crate) fn set_group_kind(&mut self, id: NodeId, kind: Option<EGroupKind>) -> Vec<NodeId> {
        let Some((path, index)) = self.find(id) else {
            return vec![id];
        };
        let body = &mut self.body;
        let seq = seq_at_mut(body, &path);
        if !matches!(seq[index].kind, EKind::Group { .. }) {
            return vec![id];
        }
        match kind {
            None => {
                let children = seq[index].children().cloned().unwrap_or_default();
                let ids = children.iter().map(|c| c.id).collect();
                seq.splice(index..index + 1, children);
                ids
            }
            Some(next) => {
                if let EKind::Group { kind, name, .. } = &mut seq[index].kind {
                    match next {
                        EGroupKind::Capturing => {
                            *kind = next;
                            name.clear();
                        }
                        EGroupKind::NamedCapturing => {
                            *kind = next;
                            if name.is_empty() {
                                *name = "name".to_string();
                            }
                        }
                        EGroupKind::NonCapturing => *kind = next,
                    }
                }
                vec![id]
            }
        }
    }

    pub(crate) fn set_lookaround(
        &mut self,
        id: NodeId,
        spec: Option<(LookKind, bool)>,
    ) -> Vec<NodeId> {
        let Some((path, index)) = self.find(id) else {
            return vec![id];
        };
        let body = &mut self.body;
        let seq = seq_at_mut(body, &path);
        if !matches!(seq[index].kind, EKind::LookAround { .. }) {
            return vec![id];
        }
        match spec {
            Some((kind, negate)) => {
                if let EKind::LookAround {
                    kind: k, negate: n, ..
                } = &mut seq[index].kind
                {
                    *k = kind;
                    *n = negate;
                }
                vec![id]
            }
            None => {
                let children = seq[index].children().cloned().unwrap_or_default();
                let ids = children.iter().map(|c| c.id).collect();
                seq.splice(index..index + 1, children);
                ids
            }
        }
    }

    pub(crate) fn remove_nodes(&mut self, ids: &[NodeId]) {
        if ids.is_empty() {
            return;
        }

        let mut paths: Vec<Vec<Step>> = Vec::new();
        for id in ids {
            if let Some((path, _)) = self.find(*id) {
                seq_at_mut(&mut self.body, &path).retain(|n| n.id != *id);
                paths.push(path);
            }
        }

        let rounds = paths.iter().map(Vec::len).max().unwrap_or(0) + 1;
        for _ in 0..rounds {
            let mut changed = false;
            for path in &paths {
                if self.cleanup_along(path) {
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    pub(crate) fn cleanup_along(&mut self, path: &[Step]) -> bool {
        let mut changed = false;
        for depth in (0..path.len()).rev() {
            let (in_children, index) = match path[depth] {
                Step::Children(i) => (true, i),
                Step::Branch(i, _) => (false, i),
            };
            let Some(seq) = try_seq_at_mut(&mut self.body, &path[..depth]) else {
                break;
            };
            if index >= seq.len() {
                continue;
            }
            if in_children
                && matches!(
                    &seq[index].kind,
                    EKind::Group { children, .. } | EKind::LookAround { children, .. }
                        if children.is_empty()
                )
            {
                seq.remove(index);
                changed = true;
                continue;
            }
            let mut flatten: Option<Vec<ENode>> = None;
            if let EKind::Choice { branches } = &mut seq[index].kind {
                let before = branches.len();
                branches.retain(|b| !b.is_empty());
                if branches.len() != before {
                    changed = true;
                }
                if branches.len() == 1 {
                    flatten = Some(std::mem::take(&mut branches[0]));
                }
            }
            if let Some(single) = flatten {
                seq.splice(index..index + 1, single);
                changed = true;
            }
        }
        changed
    }

    pub(crate) fn wrap(
        &mut self,
        ids: &[NodeId],
        kind: WrapKind,
        id_gen: &mut IdGen,
    ) -> Vec<NodeId> {
        if ids.is_empty() {
            return Vec::new();
        }
        let Some((path, index)) = self.find(ids[0]) else {
            return Vec::new();
        };
        let new_id = id_gen.next();
        let body = &mut self.body;
        let seq = seq_at_mut(body, &path);
        let len = ids.len().min(seq.len().saturating_sub(index));
        let taken: Vec<ENode> = seq.splice(index..index + len, []).collect();
        let node = match kind {
            WrapKind::Group(k) => e_group(
                new_id,
                k,
                if matches!(k, EGroupKind::NamedCapturing) {
                    "name"
                } else {
                    ""
                },
                taken,
            ),
            WrapKind::LookAround(k) => e_look(new_id, k, false, taken),
        };
        seq.insert(index, node);
        vec![new_id]
    }

    pub(crate) fn capture_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut index = 0usize;
        fn walk(nodes: &[ENode], index: &mut usize, out: &mut Vec<String>) {
            for node in nodes {
                match &node.kind {
                    EKind::Group {
                        kind,
                        name,
                        children,
                    } => {
                        match kind {
                            EGroupKind::Capturing => {
                                *index += 1;
                                out.push(index.to_string());
                            }
                            EGroupKind::NamedCapturing => {
                                *index += 1;
                                out.push(name.clone());
                            }
                            EGroupKind::NonCapturing => {}
                        }
                        walk(children, index, out);
                    }
                    EKind::LookAround { children, .. } => walk(children, index, out),
                    EKind::Choice { branches } => {
                        for branch in branches {
                            walk(branch, index, out);
                        }
                    }
                    _ => {}
                }
            }
        }
        walk(&self.body, &mut index, &mut out);
        out
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum InsertMode {
    Before,
    Parallel,
    After,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum WrapKind {
    Group(EGroupKind),
    LookAround(LookKind),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Step {
    Children(usize),

    Branch(usize, usize),
}

pub(crate) fn find_path(nodes: &[ENode], id: NodeId) -> Option<(Vec<Step>, usize)> {
    for (i, node) in nodes.iter().enumerate() {
        if node.id == id {
            return Some((Vec::new(), i));
        }
        match &node.kind {
            EKind::Group { children, .. } | EKind::LookAround { children, .. } => {
                if let Some((mut path, index)) = find_path(children, id) {
                    path.insert(0, Step::Children(i));
                    return Some((path, index));
                }
            }
            EKind::Choice { branches } => {
                for (j, branch) in branches.iter().enumerate() {
                    if let Some((mut path, index)) = find_path(branch, id) {
                        path.insert(0, Step::Branch(i, j));
                        return Some((path, index));
                    }
                }
            }
            EKind::Character { .. } | EKind::BackReference { .. } | EKind::Boundary { .. } => {}
        }
    }
    None
}

pub(crate) fn seq_at<'a>(nodes: &'a [ENode], path: &[Step]) -> &'a [ENode] {
    match path.split_first() {
        None => nodes,
        Some((step, rest)) => {
            if rest.is_empty() {
                match step {
                    Step::Children(i) => match &nodes[*i].kind {
                        EKind::Group { children, .. } | EKind::LookAround { children, .. } => {
                            children
                        }
                        _ => unreachable!(),
                    },
                    Step::Branch(i, j) => match &nodes[*i].kind {
                        EKind::Choice { branches } => &branches[*j],
                        _ => unreachable!(),
                    },
                }
            } else {
                match step {
                    Step::Children(i) => match &nodes[*i].kind {
                        EKind::Group { children, .. } | EKind::LookAround { children, .. } => {
                            seq_at(children, rest)
                        }
                        _ => unreachable!(),
                    },
                    Step::Branch(i, j) => match &nodes[*i].kind {
                        EKind::Choice { branches } => seq_at(&branches[*j], rest),
                        _ => unreachable!(),
                    },
                }
            }
        }
    }
}

pub(crate) fn seq_at_mut<'a>(nodes: &'a mut Vec<ENode>, path: &[Step]) -> &'a mut Vec<ENode> {
    match path.split_first() {
        None => nodes,
        Some((step, rest)) => {
            let next = match step {
                Step::Children(i) => match &mut nodes[*i].kind {
                    EKind::Group { children, .. } | EKind::LookAround { children, .. } => children,
                    _ => unreachable!(),
                },
                Step::Branch(i, j) => match &mut nodes[*i].kind {
                    EKind::Choice { branches } => &mut branches[*j],
                    _ => unreachable!(),
                },
            };
            seq_at_mut(next, rest)
        }
    }
}

pub(crate) fn try_seq_at_mut<'a>(
    nodes: &'a mut Vec<ENode>,
    path: &[Step],
) -> Option<&'a mut Vec<ENode>> {
    match path.split_first() {
        None => Some(nodes),
        Some((step, rest)) => {
            let next = match step {
                Step::Children(i) => match nodes.get_mut(*i).map(|n| &mut n.kind) {
                    Some(EKind::Group { children, .. } | EKind::LookAround { children, .. }) => {
                        children
                    }
                    _ => return None,
                },
                Step::Branch(i, j) => match nodes.get_mut(*i).map(|n| &mut n.kind) {
                    Some(EKind::Choice { branches }) => branches.get_mut(*j)?,
                    _ => return None,
                },
            };
            try_seq_at_mut(next, rest)
        }
    }
}

fn backref_label(lang: Lang) -> &'static str {
    lang.of("反向引用", "Back reference")
}

pub(crate) fn ranges_text(ranges: &[(String, String)], lang: Lang) -> String {
    let mut merged: Vec<char> = Vec::new();
    let mut parts: Vec<String> = Vec::new();
    for (from, to) in ranges {
        if from == to {
            let chars: Vec<char> = from.chars().collect();
            if chars.len() == 1 {
                if !merged.contains(&chars[0]) {
                    merged.push(chars[0]);
                }
            } else {
                parts.push(from.clone());
            }
        } else if from.chars().count() == 1 {
            parts.push(format!("\"{from}\" - \"{to}\""));
        } else {
            parts.push(format!("{from} - {to}"));
        }
    }
    if !merged.is_empty() {
        parts.push(format!("\"{}\"", merged.iter().collect::<String>()));
    }
    if parts.is_empty() {
        empty_label(lang).to_string()
    } else {
        parts.join(", ")
    }
}

pub(crate) fn node_text(node: &ENode, lang: Lang) -> String {
    match &node.kind {
        EKind::Character {
            kind,
            value,
            ranges,
            ..
        } => match kind {
            CharKind::String => {
                if value.is_empty() {
                    empty_label(lang).to_string()
                } else {
                    format!("\"{value}\"")
                }
            }
            CharKind::Class => class_label(value, lang)
                .map(str::to_string)
                .unwrap_or_else(|| value.clone()),
            CharKind::Ranges => ranges_text(ranges, lang),
        },
        EKind::BackReference { reference } => {
            format!("{} #{reference}", backref_label(lang))
        }
        EKind::Boundary { kind, negate } => match kind {
            BoundaryKind::Beginning => beginning_label(lang).to_string(),
            BoundaryKind::End => ending_label(lang).to_string(),
            BoundaryKind::Word => boundary_label(lang, *negate).to_string(),
        },
        EKind::Group { .. } | EKind::LookAround { .. } | EKind::Choice { .. } => String::new(),
    }
}

pub(crate) fn node_label(node: &ENode, lang: Lang) -> Option<String> {
    match &node.kind {
        EKind::Character {
            kind: CharKind::Ranges,
            negate,
            ..
        } => Some(
            if *negate {
                none_of_label(lang)
            } else {
                one_of_label(lang)
            }
            .to_string(),
        ),
        EKind::Group {
            kind: EGroupKind::Capturing,
            ..
        } => Some(format!(
            "{} #{}",
            group_word(lang),
            capture_index_of(node, lang)
        )),
        EKind::Group {
            kind: EGroupKind::NamedCapturing,
            name,
            ..
        } => Some(format!("{} #{name}", group_word(lang))),
        EKind::LookAround { kind, negate, .. } => {
            Some(lookaround_label(lang, matches!(kind, LookKind::Lookahead), *negate).to_string())
        }
        _ => None,
    }
}

pub(crate) fn capture_index_of(target: &ENode, _lang: Lang) -> usize {
    CAPTURE_INDEX.with(|cell| {
        let map = cell.borrow();
        map.get(&target.id).copied().unwrap_or(0)
    })
}

thread_local! {

    static CAPTURE_INDEX: std::cell::RefCell<std::collections::HashMap<NodeId, usize>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

pub(crate) fn index_captures(
    nodes: &[ENode],
    counter: &mut usize,
    out: &mut std::collections::HashMap<NodeId, usize>,
) {
    for node in nodes {
        match &node.kind {
            EKind::Group { kind, children, .. } => {
                if !matches!(kind, EGroupKind::NonCapturing) {
                    *counter += 1;
                    out.insert(node.id, *counter);
                }
                index_captures(children, counter, out);
            }
            EKind::LookAround { children, .. } => index_captures(children, counter, out),
            EKind::Choice { branches } => {
                for branch in branches {
                    index_captures(branch, counter, out);
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn refresh_capture_index(root: &ERoot) {
    let mut counter = 0usize;
    let mut map = std::collections::HashMap::new();
    index_captures(&root.body, &mut counter, &mut map);
    CAPTURE_INDEX.with(|cell| *cell.borrow_mut() = map);
}
