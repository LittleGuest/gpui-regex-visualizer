use std::ops::Range;

use gpui_kit::{
    component::{
        button::*,
        checkbox::Checkbox,
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
        scroll::ScrollableElement,
        select::{Select, SelectEvent, SelectState},
        tab::{Tab, TabBar},
        *,
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    ast::*,
    consts::*,
    content::*,
    convert::convert_seq,
    edit::*,
    i18n::*,
    layout::*,
    prim::*,
    syntax_highlight::{self, HighlightPalette},
    tree::*,
};

#[derive(Clone)]
pub(crate) struct RegexMatch {
    pub(crate) index: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) text: String,
    pub(crate) groups: Vec<String>,
}

pub(crate) struct LegendEntry {
    pub(crate) desc: &'static str,
    pub(crate) diagram: Diagram,
}

pub(crate) struct LegendGroup {
    pub(crate) title: &'static str,
    pub(crate) entries: Vec<LegendEntry>,
}

pub struct RegexVisualizer {
    pub(crate) pattern: String,
    pub(crate) test_text: String,
    pub(crate) flags: [bool; 4],
    pub(crate) lang: Lang,
    pub(crate) panel_tab: usize,
    pub(crate) panel_collapsed: bool,
    pub(crate) diagram: Option<Diagram>,
    pub(crate) legend: Vec<LegendGroup>,
    pub(crate) matches: Vec<RegexMatch>,
    pub(crate) error: String,
    pub(crate) mono_family: SharedString,
    pub(crate) pattern_state: Entity<InputState>,
    pub(crate) text_state: Entity<TextareaState>,
    pub(crate) _subscriptions: Vec<Subscription>,

    pub(crate) tree: ERoot,

    pub(crate) tree_source: String,

    pub(crate) selected: Vec<NodeId>,

    pub(crate) next_id: u32,
    pub(crate) undo_stack: Vec<ERoot>,
    pub(crate) redo_stack: Vec<ERoot>,

    pub(crate) edit_inputs: std::collections::HashMap<EditSlot, (Entity<InputState>, Subscription)>,

    #[allow(clippy::type_complexity)]
    edit_selects:
        std::collections::HashMap<EditSlot, (Entity<SelectState<Vec<LabeledItem>>>, Subscription)>,

    pub(crate) show_lookaround: bool,

    pub(crate) marquee: Option<((f32, f32), (f32, f32))>,
}

impl RegexVisualizer {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mono_family: SharedString = cx.theme().mono_font_family.clone();

        let pattern_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("输入正则表达式，例如 (?P<word>\\w+)")
                .default_value("".to_string())
        });
        let text_state = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("输入测试文本...")
                .default_value("".to_string())
        });

        let _subscriptions = vec![
            cx.subscribe_in(&pattern_state, window, {
                let pattern_state = pattern_state.clone();
                move |this, _, ev: &InputEvent, window, cx| {
                    if let InputEvent::Change = ev {
                        this.pattern = pattern_state.read(cx).value().to_string();

                        this.evaluate(window, cx);
                        cx.notify();
                    }
                }
            }),
            cx.subscribe_in(&text_state, window, {
                let text_state = text_state.clone();
                move |this, _, ev: &InputEvent, _, cx| {
                    if let InputEvent::Change = ev {
                        this.test_text = text_state.read(cx).value().to_string();
                        this.re_match();
                        cx.notify();
                    }
                }
            }),
        ];

        let mut this = Self {
            pattern: "".to_string(),
            test_text: "".to_string(),
            flags: [false; 4],
            lang: Lang::Cn,
            panel_tab: TAB_LEGEND,
            panel_collapsed: false,
            diagram: None,
            legend: Vec::new(),
            matches: Vec::new(),
            error: String::new(),
            mono_family,
            pattern_state,
            text_state,
            _subscriptions,
            tree: ERoot::default(),
            tree_source: String::new(),
            selected: Vec::new(),
            next_id: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            edit_inputs: std::collections::HashMap::new(),
            edit_selects: std::collections::HashMap::new(),
            show_lookaround: false,
            marquee: None,
        };
        this.legend = build_legend(window, &this.mono_family, this.lang);
        this.evaluate(window, cx);
        this
    }

    pub(crate) fn evaluate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let parsed_ok = if self.tree_source != self.pattern {
            match regex_syntax::ast::parse::Parser::new().parse(&self.pattern) {
                Ok(ast) => {
                    let mut id_gen = IdGen(self.next_id);
                    let body = convert_seq(&ast, &self.pattern, &mut id_gen);
                    self.next_id = id_gen.0;
                    self.tree = ERoot { body };
                    true
                }
                Err(_) => {
                    self.tree = ERoot::default();
                    false
                }
            }
        } else {
            regex_syntax::ast::parse::Parser::new()
                .parse(&self.pattern)
                .is_ok()
        };
        self.tree_source = self.pattern.clone();
        if parsed_ok {
        } else {
            self.undo_stack.clear();
            self.redo_stack.clear();
        }

        refresh_capture_index(&self.tree);

        self.selected.retain(|id| self.tree.node(*id).is_some());

        self.diagram = if parsed_ok {
            let root = gnode_seq(&self.tree.body, self.lang, window, &self.mono_family);
            Some(layout_diagram_selected(&root, &self.selected))
        } else {
            None
        };

        self.re_match();
        self.sync_edit_widgets(window, cx);
    }

    pub(crate) fn sync_pattern_from_tree(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pattern = self.tree.to_pattern();
        self.tree_source = self.pattern.clone();
        let value = self.pattern.clone();
        self.pattern_state
            .update(cx, |state, cx| state.set_value(value, window, cx));
        self.evaluate(window, cx);
        cx.notify();
    }

    pub(crate) fn edit<F>(&mut self, window: &mut Window, cx: &mut Context<Self>, f: F)
    where
        F: FnOnce(&mut ERoot, &mut IdGen) -> Vec<NodeId>,
    {
        let before = self.tree.clone();
        let mut id_gen = IdGen(self.next_id);
        let selection = f(&mut self.tree, &mut id_gen);
        self.next_id = id_gen.0;
        self.undo_stack.push(before);
        if self.undo_stack.len() > UNDO_LIMIT {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
        if !selection.is_empty() {
            self.selected = selection;
        }
        self.sync_pattern_from_tree(window, cx);
    }

    pub(crate) fn select_node(&mut self, id: NodeId, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected.len() == 1 && self.selected[0] == id {
            self.selected.clear();
        } else {
            self.selected = vec![id];
        }
        self.show_lookaround = false;
        self.relayout(window, cx);
    }

    pub(crate) fn delete_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            return;
        }
        let ids = self.selected.clone();
        self.edit(window, cx, move |tree, _| {
            tree.remove_nodes(&ids);
            Vec::new()
        });
    }

    pub(crate) fn set_selection(
        &mut self,
        ids: Vec<NodeId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected == ids {
            self.relayout(window, cx);
            return;
        }
        self.selected = ids;
        self.show_lookaround = false;
        self.relayout(window, cx);
    }

    pub(crate) fn clear_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            return;
        }
        self.selected.clear();
        self.show_lookaround = false;
        self.relayout(window, cx);
    }

    pub(crate) fn relayout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected.is_empty() || self.diagram.is_some() {
            let root = gnode_seq(&self.tree.body, self.lang, window, &self.mono_family);
            self.diagram = Some(layout_diagram_selected(&root, &self.selected));
        }
        self.sync_edit_widgets(window, cx);
        cx.notify();
    }

    pub(crate) fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(previous) = self.undo_stack.pop() {
            self.redo_stack.push(self.tree.clone());
            self.tree = previous;
            self.selected.clear();

            self.edit_inputs.clear();
            self.edit_selects.clear();
            self.sync_pattern_from_tree(window, cx);
        }
    }

    pub(crate) fn redo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.tree.clone());
            self.tree = next;
            self.selected.clear();
            self.edit_inputs.clear();
            self.edit_selects.clear();
            self.sync_pattern_from_tree(window, cx);
        }
    }

    pub(crate) fn content_of(&self, id: NodeId) -> Option<ContentSpec> {
        self.tree.node(id).and_then(ContentSpec::from_node)
    }

    pub(crate) fn selection(&self) -> SelectionInfo {
        self.tree.selection(&self.selected)
    }

    pub(crate) fn sync_edit_widgets(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (inputs, selects) = self.edit_field_plan();

        for (slot, value) in inputs {
            if self.edit_inputs.contains_key(&slot) {
                continue;
            }
            let state: Entity<InputState> =
                cx.new(|cx| InputState::new(window, cx).default_value(value));
            let sub = cx.subscribe_in(&state, window, {
                let slot = slot.clone();
                move |this: &mut RegexVisualizer, input, ev: &InputEvent, window, cx| {
                    if let InputEvent::Change = ev {
                        let value = input.read(cx).value().to_string();
                        this.on_edit_input(&slot, &value, window, cx);
                    }
                }
            });
            self.edit_inputs.insert(slot, (state, sub));
        }

        for (slot, items, selected) in selects {
            if self.edit_selects.contains_key(&slot) {
                continue;
            }
            let state: Entity<SelectState<Vec<LabeledItem>>> = cx.new(|cx| {
                let mut state = SelectState::new(items, None, window, cx);
                state.set_selected_value(&selected, window, cx);
                state
            });
            let sub = cx.subscribe_in(&state, window, {
                let slot = slot.clone();
                move |this: &mut RegexVisualizer,
                      _,
                      ev: &SelectEvent<Vec<LabeledItem>>,
                      window,
                      cx| {
                    if let SelectEvent::Confirm(Some(value)) = ev {
                        let value = value.clone();
                        this.on_edit_select(&slot, &value, window, cx);
                    }
                }
            });
            self.edit_selects.insert(slot, (state, sub));
        }

        let (wanted_inputs, wanted_selects) = self.edit_field_plan();
        let input_keys: Vec<EditSlot> = wanted_inputs.into_iter().map(|(s, _)| s).collect();
        let select_keys: Vec<EditSlot> = wanted_selects.into_iter().map(|(s, _, _)| s).collect();
        self.edit_inputs.retain(|k, _| input_keys.contains(k));
        self.edit_selects.retain(|k, _| select_keys.contains(k));
    }

    #[allow(clippy::type_complexity)]
    pub(crate) fn edit_field_plan(
        &self,
    ) -> (
        Vec<(EditSlot, String)>,
        Vec<(EditSlot, Vec<LabeledItem>, String)>,
    ) {
        let mut inputs: Vec<(EditSlot, String)> = Vec::new();
        let mut selects: Vec<(EditSlot, Vec<LabeledItem>, String)> = Vec::new();
        if self.panel_tab != TAB_EDIT || self.selected.is_empty() {
            return (inputs, selects);
        }
        let info = self.selection();
        let id = info.id;

        if let Some(content) = &info.content {
            let keys = content_type_keys(&info, self.tree.capture_names().len());
            let items: Vec<LabeledItem> = keys
                .iter()
                .filter_map(|key| {
                    CONTENT_TYPES
                        .iter()
                        .find(|(k, _, _)| k == key)
                        .map(|(k, cn, en)| LabeledItem::new(*k, self.lang.of(cn, en)))
                })
                .collect();
            selects.push((EditSlot::ContentType(id), items, content.key().to_string()));
            match content {
                ContentSpec::String { value } => {
                    inputs.push((EditSlot::Value(id), value.clone()));
                }
                ContentSpec::Class { value } => {
                    let kind = class_kind_key(value);
                    selects.push((
                        EditSlot::ClassKind(id),
                        class_items(self.lang),
                        kind.clone(),
                    ));
                    if kind != *value {
                        inputs.push((EditSlot::Value(id), value.clone()));
                    }
                }
                ContentSpec::Ranges { ranges, .. } => {
                    for (i, (from, to)) in ranges.iter().enumerate() {
                        inputs.push((EditSlot::RangeFrom(id, i), from.clone()));
                        inputs.push((EditSlot::RangeTo(id, i), to.clone()));
                    }
                }
                ContentSpec::BackReference { reference } => {
                    let mut names = self.tree.capture_names();
                    if !names.contains(reference) {
                        names.insert(0, reference.clone());
                    }
                    let items: Vec<LabeledItem> = names
                        .iter()
                        .map(|name| {
                            LabeledItem::new(
                                name.clone(),
                                format!("{} #{name}", group_word(self.lang)),
                            )
                        })
                        .collect();
                    selects.push((EditSlot::Backref(id), items, reference.clone()));
                }
                _ => {}
            }
        }

        if info.has_quantifier {
            let items: Vec<LabeledItem> = QUANT_OPTIONS
                .iter()
                .map(|(key, cn, en, _)| LabeledItem::new(*key, self.lang.of(cn, en)))
                .collect();
            selects.push((
                EditSlot::QuantKind(id),
                items,
                quant_key(info.quantifier.as_ref()).to_string(),
            ));
            if let Some(q) = &info.quantifier
                && q.kind == QuantKind::Custom
            {
                inputs.push((EditSlot::QuantMin(id), q.min.to_string()));
                inputs.push((
                    EditSlot::QuantMax(id),
                    if q.infinite() {
                        String::new()
                    } else {
                        q.max.to_string()
                    },
                ));
            }
        }

        if let Some((EGroupKind::NamedCapturing, name)) = &info.group {
            inputs.push((EditSlot::GroupName(id), name.clone()));
        }

        if let Some((kind, _)) = &info.group {
            let items = vec![
                LabeledItem::new("capturing", self.lang.of("捕获组", "Capturing group")),
                LabeledItem::new(
                    "nonCapturing",
                    self.lang.of("非捕获组", "Non-capturing group"),
                ),
                LabeledItem::new(
                    "namedCapturing",
                    self.lang.of("具名捕获组", "Named capturing group"),
                ),
            ];
            selects.push((
                EditSlot::GroupKind(id),
                items,
                group_kind_key(*kind).to_string(),
            ));
        }

        if let Some((kind, _)) = info.lookaround {
            let items = vec![
                LabeledItem::new("lookahead", self.lang.of("向前断言", "Lookahead assertion")),
                LabeledItem::new(
                    "lookbehind",
                    self.lang.of("向后断言", "Lookbehind assertion"),
                ),
            ];
            selects.push((
                EditSlot::LookKind(id),
                items,
                look_kind_key(kind).to_string(),
            ));
        }

        (inputs, selects)
    }

    pub(crate) fn on_edit_input(
        &mut self,
        slot: &EditSlot,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match slot.clone() {
            EditSlot::Value(id) => {
                let next = match self.content_of(id) {
                    Some(ContentSpec::String { .. }) => ContentSpec::String {
                        value: value.to_string(),
                    },
                    Some(ContentSpec::Class { .. }) => ContentSpec::Class {
                        value: value.to_string(),
                    },
                    _ => return,
                };
                if self.content_of(id).as_ref() == Some(&next) {
                    return;
                }
                self.edit(window, cx, |tree, id_gen| {
                    vec![tree.set_content(id, &next, id_gen)]
                });
            }
            EditSlot::RangeFrom(id, i) => self.update_range(id, i, true, value, window, cx),
            EditSlot::RangeTo(id, i) => self.update_range(id, i, false, value, window, cx),
            EditSlot::QuantMin(id) | EditSlot::QuantMax(id) => {
                let is_min = matches!(slot, EditSlot::QuantMin(_));
                let Some(Some(q)) = self.content_of(id).map(|_| self.quantifier_of(id)) else {
                    return;
                };
                let parsed = match value.trim() {
                    "" => {
                        if is_min {
                            0
                        } else {
                            Quantifier::INF
                        }
                    }
                    "Infinity" | "∞" => Quantifier::INF,
                    other => match other.parse::<u32>() {
                        Ok(n) => n,
                        Err(_) => return,
                    },
                };
                let (min, max) = if is_min {
                    (parsed, q.max)
                } else {
                    (q.min, parsed)
                };
                if min > max {
                    return;
                }
                let next = Quantifier {
                    kind: QuantKind::Custom,
                    min,
                    max,
                    greedy: q.greedy,
                };
                if q.kind == QuantKind::Custom && q.min == next.min && q.max == next.max {
                    return;
                }
                self.edit(window, cx, |tree, id_gen| {
                    vec![tree.set_quantifier(id, Some(next), id_gen)]
                });
            }
            EditSlot::GroupName(id) => {
                if self
                    .tree
                    .node(id)
                    .is_some_and(|n| matches!(&n.kind, EKind::Group { name, .. } if name == value))
                {
                    return;
                }
                let name = value.to_string();
                self.edit(window, cx, |tree, id_gen| {
                    let gen_ref = &mut *id_gen;
                    set_group_name(tree, id, &name);
                    let _ = gen_ref;
                    Vec::new()
                });
            }
            _ => {}
        }
    }

    pub(crate) fn quantifier_of(&self, id: NodeId) -> Option<Quantifier> {
        self.tree.node(id).and_then(|n| n.quantifier)
    }

    pub(crate) fn update_range(
        &mut self,
        id: NodeId,
        index: usize,
        is_from: bool,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ContentSpec::Ranges { ranges, negate }) = self.content_of(id) else {
            return;
        };
        let mut next = ranges.clone();
        let Some(slot) = next.get_mut(index) else {
            return;
        };
        if is_from {
            slot.0 = value.to_string();
        } else {
            slot.1 = value.to_string();
        }
        if next == ranges {
            return;
        }
        let spec = ContentSpec::Ranges {
            ranges: next,
            negate,
        };
        self.edit(window, cx, |tree, id_gen| {
            vec![tree.set_content(id, &spec, id_gen)]
        });
    }

    pub(crate) fn on_edit_select(
        &mut self,
        slot: &EditSlot,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match slot.clone() {
            EditSlot::ContentType(id) => {
                if self.content_of(id).is_some_and(|c| c.key() == value) {
                    return;
                }
                let spec = ContentSpec::for_key(value);
                self.edit(window, cx, |tree, id_gen| {
                    vec![tree.set_content(id, &spec, id_gen)]
                });
            }
            EditSlot::ClassKind(id) => {
                let raw = match value {
                    "\\xhh" => "\\x00".to_string(),
                    "\\uhhhh" => "\\u0000".to_string(),
                    other => other.to_string(),
                };
                let spec = ContentSpec::Class { value: raw };
                if self.content_of(id).as_ref() == Some(&spec) {
                    return;
                }
                self.edit(window, cx, |tree, id_gen| {
                    vec![tree.set_content(id, &spec, id_gen)]
                });
            }
            EditSlot::Backref(id) => {
                let spec = ContentSpec::BackReference {
                    reference: value.to_string(),
                };
                if self.content_of(id).as_ref() == Some(&spec) {
                    return;
                }
                self.edit(window, cx, |tree, id_gen| {
                    vec![tree.set_content(id, &spec, id_gen)]
                });
            }
            EditSlot::QuantKind(id) => {
                let greedy = self.quantifier_of(id).map(|q| q.greedy).unwrap_or(true);
                let next = quant_for_key(value, greedy);
                self.edit(window, cx, |tree, id_gen| {
                    vec![tree.set_quantifier(id, next, id_gen)]
                });
            }
            EditSlot::GroupKind(id) => {
                let next = match value {
                    "capturing" => EGroupKind::Capturing,
                    "nonCapturing" => EGroupKind::NonCapturing,
                    _ => EGroupKind::NamedCapturing,
                };
                if self
                    .tree
                    .node(id)
                    .is_some_and(|n| matches!(&n.kind, EKind::Group { kind, .. } if *kind == next))
                {
                    return;
                }
                self.edit(window, cx, move |tree, _| {
                    tree.set_group_kind(id, Some(next))
                });
            }
            EditSlot::LookKind(id) => {
                let negate = self
                    .tree
                    .node(id)
                    .and_then(|n| match &n.kind {
                        EKind::LookAround { negate, .. } => Some(*negate),
                        _ => None,
                    })
                    .unwrap_or(false);
                let kind = if value == "lookbehind" {
                    LookKind::Lookbehind
                } else {
                    LookKind::Lookahead
                };
                self.edit(window, cx, move |tree, _| {
                    tree.set_lookaround(id, Some((kind, negate)))
                });
            }
            _ => {}
        }
    }

    pub fn lang(&self) -> Lang {
        self.lang
    }

    pub fn set_lang(&mut self, lang: Lang, window: &mut Window, cx: &mut Context<Self>) {
        if self.lang == lang {
            return;
        }
        self.lang = lang;
        self.legend = build_legend(window, &self.mono_family, self.lang);
        self.evaluate(window, cx);
        cx.notify();
    }

    pub fn toggle_lang(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_lang(self.lang.toggled(), window, cx);
    }

    pub(crate) fn re_match(&mut self) {
        self.matches.clear();
        self.error.clear();
        if self.pattern.is_empty() {
            return;
        }
        let regex = match regex::RegexBuilder::new(&self.pattern)
            .case_insensitive(self.flags[1])
            .multi_line(self.flags[2])
            .dot_matches_new_line(self.flags[3])
            .build()
        {
            Ok(regex) => regex,
            Err(err) => {
                self.error = err.to_string();
                return;
            }
        };

        for (index, captures) in regex.captures_iter(&self.test_text).enumerate() {
            if let Some(full) = captures.get(0) {
                let groups = captures
                    .iter()
                    .enumerate()
                    .skip(1)
                    .map(|(group_index, value)| match value {
                        Some(value) => format!("#{group_index}: {}", value.as_str()),
                        None => {
                            if self.lang == Lang::Cn {
                                format!("#{group_index}: <未匹配>")
                            } else {
                                format!("#{group_index}: <no match>")
                            }
                        }
                    })
                    .collect();
                self.matches.push(RegexMatch {
                    index: index + 1,
                    start: full.start(),
                    end: full.end(),
                    text: full.as_str().to_string(),
                    groups,
                });
            }
        }
    }

    pub(crate) fn toggle_flag(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(slot) = self.flags.get_mut(index) {
            *slot = !*slot;
        }
        self.re_match();
        cx.notify();
    }

    pub(crate) fn flags_string(&self) -> String {
        FLAGS
            .iter()
            .filter(|(index, _, _, _)| self.flags[*index])
            .map(|(_, ch, _, _)| ch.to_string())
            .collect()
    }

    pub(crate) fn apply_sample(
        &mut self,
        pattern: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pattern = pattern.to_string();
        self.pattern_state.update(cx, |state, cx| {
            state.set_value(pattern.to_string(), window, cx);
        });
        self.evaluate(window, cx);
        self.panel_tab = TAB_TEST;
        self.panel_collapsed = false;
        cx.notify();
    }

    pub(crate) fn set_panel_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.panel_tab = index;
        if index == TAB_EDIT {
            self.sync_edit_widgets(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn edit_panel(&self, cx: &Context<Self>) -> AnyElement {
        let lang = self.lang;
        let mut root = div().flex().flex_col().gap_6();

        if self.selected.is_empty() {
            return root
                .child(hint(
                    lang.of(
                        "在轨道图里点击一个节点，即可编辑它的内容、分组与量词",
                        "Click a node in the graph to edit its content, group and quantifier",
                    ),
                    cx,
                ))
                .into_any_element();
        }

        let info = self.selection();
        let ids = info.ids.clone();
        let id = info.id;

        root = root.child(self.insert_section(&info, &ids, cx));
        root = root.child(panel_section(
            lang.of("表达式", "Expression"),
            None,
            expression_body(&info.pattern, info.start, info.end, &self.mono_family),
            cx,
        ));

        if info.content.is_some() {
            root = root.child(self.content_section(&info, cx));
        }
        if let Some((kind, _name)) = info.group.clone() {
            root = root.child(self.group_section(id, kind, cx));
        }
        if info.has_quantifier {
            root = root.child(self.quantifier_section(&info, cx));
        }
        if let Some((kind, negate)) = info.lookaround {
            root = root.child(self.lookaround_section(id, kind, negate, cx));
        }

        root.into_any_element()
    }

    pub(crate) fn insert_section(
        &self,
        info: &SelectionInfo,
        ids: &[NodeId],
        cx: &Context<Self>,
    ) -> Div {
        let lang = self.lang;
        let mut block = div().flex().flex_col().gap_6();

        let ids_owned = ids.to_vec();
        let first_is_begin = self.tree.node(ids[0]).is_some_and(|n| {
            matches!(
                &n.kind,
                EKind::Boundary {
                    kind: BoundaryKind::Beginning,
                    ..
                }
            )
        });
        let last_is_end = self
            .tree
            .node(*ids.last().unwrap_or(&ids[0]))
            .is_some_and(|n| {
                matches!(
                    &n.kind,
                    EKind::Boundary {
                        kind: BoundaryKind::End,
                        ..
                    }
                )
            });

        let mut insert_buttons: Vec<AnyElement> = Vec::new();
        if !first_is_begin {
            insert_buttons.push(insert_button(
                "regex-insert-prev",
                lang.of("向前插入", "Before"),
                InsertMode::Before,
                &ids_owned,
                cx,
            ));
        }
        insert_buttons.push(insert_button(
            "regex-insert-parallel",
            lang.of("插入或", "Parallel"),
            InsertMode::Parallel,
            &ids_owned,
            cx,
        ));
        if !last_is_end {
            insert_buttons.push(insert_button(
                "regex-insert-next",
                lang.of("向后插入", "After"),
                InsertMode::After,
                &ids_owned,
                cx,
            ));
        }
        block = block.child(panel_section(
            lang.of("插入节点", "Insert around"),
            None,
            button_row(insert_buttons).into_any_element(),
            cx,
        ));

        if !(info.single && info.group.is_some()) {
            let mut buttons: Vec<AnyElement> = Vec::new();
            for (index, (kind, cn, en)) in [
                (EGroupKind::Capturing, "捕获组", "Capturing"),
                (EGroupKind::NonCapturing, "非捕获组", "Non-cap"),
                (EGroupKind::NamedCapturing, "具名捕获组", "Named cap"),
            ]
            .into_iter()
            .enumerate()
            {
                buttons.push(
                    Button::new(("regex-wrap-group", index))
                        .label(lang.of(cn, en))
                        .outline()
                        .compact()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let ids = this.selected.clone();
                            this.edit(window, cx, |tree, id_gen| {
                                tree.wrap(&ids, WrapKind::Group(kind), id_gen)
                            });
                        }))
                        .into_any_element(),
                );
            }
            block = block.child(panel_section(
                lang.of("分组", "Group selection"),
                None,
                button_row(buttons).into_any_element(),
                cx,
            ));
        }

        if !(info.single && info.lookaround.is_some()) {
            if self.show_lookaround {
                let mut buttons: Vec<AnyElement> = Vec::new();
                for (index, (kind, cn, en)) in [
                    (LookKind::Lookahead, "向前断言", "Lookahead"),
                    (LookKind::Lookbehind, "向后断言", "Lookbehind"),
                ]
                .into_iter()
                .enumerate()
                {
                    buttons.push(
                        Button::new(("regex-wrap-look", index))
                            .label(lang.of(cn, en))
                            .outline()
                            .compact()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                let ids = this.selected.clone();
                                this.edit(window, cx, |tree, id_gen| {
                                    tree.wrap(&ids, WrapKind::LookAround(kind), id_gen)
                                });
                            }))
                            .into_any_element(),
                    );
                }
                block = block.child(panel_section(
                    lang.of("向前/向后断言", "Lookaround assertion"),
                    None,
                    button_row(buttons).into_any_element(),
                    cx,
                ));
            }
            block = block.child(
                div().flex().justify_center().child(
                    div()
                        .id("regex-lookaround-toggle")
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py(px(2.0))
                        .rounded_full()
                        .border_1()
                        .border_color(cx.theme().border)
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .cursor_pointer()
                        .hover(|style| style.bg(cx.theme().muted))
                        .child(lang.of("显示更多", "show more"))
                        .child(
                            Icon::new(IconName::ChevronDown)
                                .size(px(12.0))
                                .text_color(cx.theme().muted_foreground),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_lookaround = true;
                            cx.notify();
                        })),
                ),
            );
        }

        block
    }

    pub(crate) fn content_section(&self, info: &SelectionInfo, cx: &Context<Self>) -> Div {
        let lang = self.lang;
        let id = info.id;
        let Some(content) = info.content.clone() else {
            return div();
        };

        let mut body = div().flex().flex_col().gap_6();

        let mut type_row = div().flex().items_center().gap_2();
        if let Some((state, _)) = self.edit_selects.get(&EditSlot::ContentType(id)) {
            type_row = type_row.child(div().w(px(208.0)).child(Select::new(state)));
        }
        body = body.child(panel_item(lang.of("类型", "Type"), type_row));

        match &content {
            ContentSpec::String { .. } => {
                let mut column = div().flex().flex_col().gap_2();

                column = column.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .p_2()
                        .rounded(px(6.0))
                        .bg(cx.theme().muted)
                        .child(
                            Icon::new(IconName::Info)
                                .size(px(16.0))
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(cx.theme().muted_foreground)
                                .child(lang.of(
                                    "输入将会被自动转义",
                                    "The input will be escaped automatically.",
                                )),
                        ),
                );
                if let Some((state, _)) = self.edit_inputs.get(&EditSlot::Value(id)) {
                    column = column.child(Input::new(state));
                }
                body = body.child(panel_item(" 值", column));
            }
            ContentSpec::Class { value } => {
                let mut row = div().flex().items_center().gap_2();
                if let Some((state, _)) = self.edit_selects.get(&EditSlot::ClassKind(id)) {
                    row = row.child(div().w(px(208.0)).child(Select::new(state)));
                }

                if class_kind_key(value) != *value
                    && let Some((state, _)) = self.edit_inputs.get(&EditSlot::Value(id))
                {
                    row = row.child(div().w(px(208.0)).child(Input::new(state)));
                }
                body = body.child(panel_item(lang.of("类", "Class"), row));
            }
            ContentSpec::Ranges { ranges, negate } => {
                body = body.child(panel_item(
                    lang.of("范围", "Ranges"),
                    self.ranges_body(id, ranges, *negate, cx),
                ));
            }
            ContentSpec::BackReference { .. } => {
                let mut row = div().flex().items_center().gap_2();
                if let Some((state, _)) = self.edit_selects.get(&EditSlot::Backref(id)) {
                    row = row.child(div().w(px(208.0)).child(Select::new(state)));
                }
                body = body.child(panel_item(lang.of("反向引用", "Back Reference"), row));
            }
            ContentSpec::WordBoundary { negate } => {
                body = body.child(panel_item(
                    lang.of("否定", "Negate"),
                    negate_row(
                        "regex-negate-word",
                        *negate,
                        cx,
                        move |view, value, window, cx| {
                            view.set_negate(id, value, window, cx);
                        },
                    ),
                ));
            }
            _ => {}
        }

        panel_section(
            lang.of("内容", "Content"),
            None,
            body.into_any_element(),
            cx,
        )
    }

    pub(crate) fn ranges_body(
        &self,
        id: NodeId,
        ranges: &[(String, String)],
        negate: bool,
        cx: &Context<Self>,
    ) -> Div {
        let lang = self.lang;
        let mut body = div().flex().flex_col().gap_4();

        let mut rows = div().flex().flex_col().gap(px(10.0));
        for index in 0..ranges.len() {
            let mut row = div().flex().items_center().gap_2();
            if let Some((state, _)) = self.edit_inputs.get(&EditSlot::RangeFrom(id, index)) {
                row = row.child(div().flex_1().child(Input::new(state)));
            }
            row = row.child(div().text_sm().child("-"));
            if let Some((state, _)) = self.edit_inputs.get(&EditSlot::RangeTo(id, index)) {
                row = row.child(div().flex_1().child(Input::new(state)));
            }
            row = row.child(
                Button::new(("regex-range-remove", index))
                    .icon(Icon::new(IconName::Close))
                    .ghost()
                    .compact()
                    .tooltip(lang.of("删除该范围", "Remove this range"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.remove_range(id, index, window, cx);
                    })),
            );
            rows = rows.child(row);
        }
        body = body.child(rows);

        let mut actions = div().flex().flex_wrap().items_center().gap_2();
        actions = actions.child(
            Button::new("regex-range-add")
                .icon(Icon::new(IconName::Plus))
                .label("An Empty Range")
                .outline()
                .compact()
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.add_range(id, "", "", window, cx);
                })),
        );

        for (index, (from, to)) in [("0", "9"), ("a", "z"), ("A", "Z")].into_iter().enumerate() {
            actions = actions.child(
                Button::new(("regex-range-preset", index))
                    .label(format!("{from} - {to}"))
                    .outline()
                    .compact()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.add_range(id, from, to, window, cx);
                    })),
            );
        }
        body = body.child(actions);

        body.child(negate_row(
            "regex-negate-ranges",
            negate,
            cx,
            move |view, value, window, cx| {
                view.set_negate(id, value, window, cx);
            },
        ))
    }

    pub(crate) fn group_section(&self, id: NodeId, kind: EGroupKind, cx: &Context<Self>) -> Div {
        let lang = self.lang;
        let mut body = div().flex().flex_col().gap_2();

        let mut row = div().flex().items_center().gap_2();
        if let Some((state, _)) = self.edit_selects.get(&EditSlot::GroupKind(id)) {
            row = row.child(div().w(px(192.0)).child(Select::new(state)));
        }
        body = body.child(row);

        if kind == EGroupKind::NamedCapturing {
            let mut name_row = div().flex().items_center();
            name_row = name_row.child(
                div()
                    .h(px(32.0))
                    .flex()
                    .items_center()
                    .px_2()
                    .text_size(px(12.0))
                    .text_color(cx.theme().muted_foreground)
                    .bg(cx.theme().muted)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_l(px(6.0))
                    .child(lang.of("组名", "Group's name")),
            );
            if let Some((state, _)) = self.edit_inputs.get(&EditSlot::GroupName(id)) {
                name_row = name_row.child(div().flex_1().min_w_0().child(Input::new(state)));
            }
            body = body.child(name_row);
        }

        panel_section(
            lang.of("组", "Group"),
            Some(
                Button::new("regex-ungroup")
                    .icon(Icon::new(IconName::Close))
                    .ghost()
                    .compact()
                    .tooltip(lang.of("取消组", "UnGroup"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit(window, cx, |tree, _| tree.set_group_kind(id, None));
                    }))
                    .into_any_element(),
            ),
            body.into_any_element(),
            cx,
        )
    }

    pub(crate) fn quantifier_section(&self, info: &SelectionInfo, cx: &Context<Self>) -> Div {
        let lang = self.lang;
        let id = info.id;
        let mut body = div().flex().flex_col().gap(px(10.0));

        let mut row = div().flex().items_center().gap_2();
        if let Some((state, _)) = self.edit_selects.get(&EditSlot::QuantKind(id)) {
            row = row.child(div().w(px(208.0)).child(Select::new(state)));
        }
        body = body.child(row);

        if info.quantifier.is_some_and(|q| q.kind == QuantKind::Custom) {
            let mut custom = div().flex().items_center().gap_2();
            if let Some((state, _)) = self.edit_inputs.get(&EditSlot::QuantMin(id)) {
                custom = custom.child(div().flex_1().child(Input::new(state)));
            }
            custom = custom.child(div().text_sm().child("-"));
            if let Some((state, _)) = self.edit_inputs.get(&EditSlot::QuantMax(id)) {
                custom = custom.child(div().flex_1().child(Input::new(state)));
            }
            body = body.child(custom);
        }

        if let Some(quantifier) = info.quantifier {
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(13.0))
                    .text_color(cx.theme().muted_foreground)
                    .child("Greedy")
                    .child(
                        Checkbox::new("regex-greedy")
                            .checked(quantifier.greedy)
                            .on_click(cx.listener(move |this, value: &bool, window, cx| {
                                this.set_greedy(id, *value, window, cx);
                            })),
                    ),
            );
        }

        panel_section(
            lang.of("量词", "Quantifier"),
            None,
            panel_section(lang.of("次数", "times"), None, body.into_any_element(), cx)
                .into_any_element(),
            cx,
        )
    }

    pub(crate) fn lookaround_section(
        &self,
        id: NodeId,
        kind: LookKind,
        negate: bool,
        cx: &Context<Self>,
    ) -> Div {
        let lang = self.lang;
        let mut body = div().flex().flex_col().gap_4();

        let mut row = div().flex().items_center().gap_2();
        if let Some((state, _)) = self.edit_selects.get(&EditSlot::LookKind(id)) {
            row = row.child(div().w(px(208.0)).child(Select::new(state)));
        }
        body = body.child(row);
        body = body.child(negate_row(
            "regex-negate-look",
            negate,
            cx,
            move |view, value, window, cx| {
                view.set_lookaround_negate(id, value, window, cx);
            },
        ));
        let _ = kind;

        panel_section(
            lang.of("向前/向后断言", "Lookaround assertion"),
            Some(
                Button::new("regex-cancel-look")
                    .icon(Icon::new(IconName::Close))
                    .ghost()
                    .compact()
                    .tooltip(lang.of("取消断言", "Cancel assertion"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit(window, cx, |tree, _| tree.set_lookaround(id, None));
                    }))
                    .into_any_element(),
            ),
            body.into_any_element(),
            cx,
        )
    }

    pub(crate) fn add_range(
        &mut self,
        id: NodeId,
        from: &str,
        to: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ContentSpec::Ranges { mut ranges, negate }) = self.content_of(id) else {
            return;
        };
        ranges.push((from.to_string(), to.to_string()));
        let spec = ContentSpec::Ranges { ranges, negate };
        self.edit(window, cx, move |tree, id_gen| {
            vec![tree.set_content(id, &spec, id_gen)]
        });
    }

    pub(crate) fn remove_range(
        &mut self,
        id: NodeId,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ContentSpec::Ranges { mut ranges, negate }) = self.content_of(id) else {
            return;
        };
        if index >= ranges.len() {
            return;
        }
        ranges.remove(index);
        let spec = ContentSpec::Ranges { ranges, negate };
        self.edit(window, cx, move |tree, id_gen| {
            vec![tree.set_content(id, &spec, id_gen)]
        });
    }

    pub(crate) fn set_negate(
        &mut self,
        id: NodeId,
        negate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(content) = self.content_of(id) else {
            return;
        };
        let spec = match content {
            ContentSpec::Ranges { ranges, .. } => ContentSpec::Ranges { ranges, negate },
            ContentSpec::WordBoundary { .. } => ContentSpec::WordBoundary { negate },
            _ => return,
        };
        self.edit(window, cx, move |tree, id_gen| {
            vec![tree.set_content(id, &spec, id_gen)]
        });
    }

    pub(crate) fn set_lookaround_negate(
        &mut self,
        id: NodeId,
        negate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(kind) = self.tree.node(id).and_then(|n| match &n.kind {
            EKind::LookAround { kind, .. } => Some(*kind),
            _ => None,
        }) else {
            return;
        };
        self.edit(window, cx, move |tree, _| {
            tree.set_lookaround(id, Some((kind, negate)))
        });
    }

    pub(crate) fn set_greedy(
        &mut self,
        id: NodeId,
        greedy: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(mut quantifier) = self.quantifier_of(id) else {
            return;
        };
        if quantifier.greedy == greedy {
            return;
        }
        quantifier.greedy = greedy;
        self.edit(window, cx, move |tree, id_gen| {
            vec![tree.set_quantifier(id, Some(quantifier), id_gen)]
        });
    }

    pub(crate) fn paste_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(item) = cx.read_from_clipboard()
            && let Some(text) = item.text()
        {
            self.test_text = text.to_string();
            self.text_state.update(cx, |state, cx| {
                state.set_value(self.test_text.clone(), window, cx);
            });
            self.re_match();
        }
    }

    pub(crate) fn copy_pattern(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(self.pattern.clone()));
    }

    pub(crate) fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pattern.clear();
        self.test_text.clear();
        self.diagram = None;
        self.matches.clear();
        self.error.clear();
        self.tree = ERoot::default();
        self.tree_source.clear();
        self.selected.clear();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.edit_inputs.clear();
        self.edit_selects.clear();
        self.marquee = None;
        self.pattern_state.update(cx, |state, cx| {
            state.set_value(String::new(), window, cx);
        });
        self.text_state.update(cx, |state, cx| {
            state.set_value(String::new(), window, cx);
        });
    }
}

#[allow(clippy::type_complexity)]
pub(crate) const LEGEND_TEXT: [(&str, &str, &[(&str, &str)]); 8] = [
    (
        "字符",
        "Characters",
        &[("直接匹配字符串", "Direct match characters")],
    ),
    (
        "字符类",
        "Character classes",
        &[(
            "区分不同类型的字符",
            "Distinguish different types of characters",
        )],
    ),
    (
        "范围",
        "Ranges",
        &[
            (
                "匹配任何一个包含的字符",
                "Matches any one of the enclosed characters",
            ),
            (
                "匹配任何没有包含在括号中的字符",
                "Matches anything that is not enclosed in the brackets",
            ),
        ],
    ),
    (
        "或",
        "Choice",
        &[("匹配 “x” 或者 “y”", "Matches either \"x\" or \"y\"")],
    ),
    (
        "量词",
        "Quantifier",
        &[(
            "表示要匹配的字符或表达式的数量",
            "Indicate numbers of characters or expressions to match",
        )],
    ),
    (
        "组",
        "Group",
        &[
            ("匹配x并记住匹配项", "Matches x and remembers the match"),
            (
                "匹配 “x”，但不记得匹配",
                "Matches \"x\" but does not remember the match",
            ),
            (
                "匹配 “x” 并将其存储在返回的匹配项的groups属性中，该属性位于 <Name> 指定的名称下",
                "Matches \"x\" and stores it on the groups property of the returned matches under the name specified by <Name>",
            ),
        ],
    ),
    (
        "反向引用",
        "Back reference",
        &[
            ("匹配组 #1 的反向引用", "A back reference to match group #1"),
            (
                "匹配组 #Name 的反向引用",
                "A back reference to match group #Name",
            ),
        ],
    ),
    (
        "断言",
        "Assertion",
        &[
            ("匹配输入的开头", "Matches the beginning of input"),
            (
                "x 被 y 跟随时匹配 x",
                "Matches \"x\" only if \"x\" is followed by \"y\"",
            ),
        ],
    ),
];

pub(crate) fn build_legend(
    window: &mut Window,
    family: &SharedString,
    lang: Lang,
) -> Vec<LegendGroup> {
    let t =
        |text: &str, window: &mut Window| token_node(text.to_string(), None, false, window, family);
    let range = |label: &'static str, negated: bool, window: &mut Window| {
        token_node(
            "\"a\" - \"z\"".to_string(),
            Some(label.to_string()),
            negated,
            window,
            family,
        )
    };
    let class_text = class_label(r"\d", lang).unwrap_or(r"\d").to_string();
    let backref = lang.of("反向引用", "Back Reference");

    let diagrams: Vec<Vec<Diagram>> = vec![
        vec![layout_content(&t("\"abc\"", window))],
        vec![layout_content(&t(&class_text, window))],
        vec![
            layout_content(&range(one_of_label(lang), false, window)),
            layout_content(&range(none_of_label(lang), true, window)),
        ],
        vec![layout_content(&build_alternate(vec![
            t("\"x\"", window),
            t("\"y\"", window),
        ]))],
        vec![layout_content(&repeat_node(
            t("\"a\"", window),
            0,
            None,
            window,
            family,
        ))],
        vec![
            layout_content(&group_node(
                Some(format!("{} #1", group_word(lang))),
                t("\"x\"", window),
                window,
                family,
            )),
            layout_content(&group_node(None, t("\"x\"", window), window, family)),
            layout_content(&group_node(
                Some(format!("{} #Name", group_word(lang))),
                t("\"x\"", window),
                window,
                family,
            )),
        ],
        vec![
            layout_content(&t(&format!("{backref} #1"), window)),
            layout_content(&t(&format!("{backref} #Name"), window)),
        ],
        vec![
            layout_content(&t(beginning_label(lang), window)),
            layout_content(&build_concat(vec![
                t("\"x\"", window),
                group_node(
                    Some(lookaround_label(lang, true, false).to_string()),
                    t("\"y\"", window),
                    window,
                    family,
                ),
            ])),
        ],
    ];

    LEGEND_TEXT
        .iter()
        .zip(diagrams)
        .map(
            |((cn_title, en_title, entries), group_diagrams)| LegendGroup {
                title: lang.of(cn_title, en_title),
                entries: entries
                    .iter()
                    .zip(group_diagrams)
                    .map(|((cn_desc, en_desc), diagram)| LegendEntry {
                        desc: lang.of(cn_desc, en_desc),
                        diagram,
                    })
                    .collect(),
            },
        )
        .collect()
}

fn error_one_line(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for word in raw.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

impl Render for RegexVisualizer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let matches = self.matches.clone();
        let match_count = self.matches.len();
        let error = self.error.clone();
        let has_error = !error.is_empty();
        let error_text = error_one_line(&error);
        let pattern_empty = self.pattern.is_empty();
        let flags_str = self.flags_string();
        let panel_tab = self.panel_tab;
        let lang = self.lang;
        let flags_state = self.flags;
        let mono_family = self.mono_family.clone();
        let show_hint = self.diagram.is_some() && error.is_empty();
        let can_undo = !self.undo_stack.is_empty();
        let can_redo = !self.redo_stack.is_empty();

        let diagram_area = div()
            .flex_1()
            .min_h_0()
            .relative()
            .overflow_scrollbar()
            .bg(cx.theme().background)
            .child(
                div()
                    .min_h_full()
                    .min_w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_8()
                    .children(match self.diagram.as_ref() {
                        Some(diagram) => {
                            vec![
                                interactive_diagram_canvas(diagram, self.marquee, cx)
                                    .into_any_element(),
                            ]
                        }

                        None if has_error => Vec::new(),
                        None => {
                            vec![
                                hint(
                                    lang.of(
                                        "输入有效正则后显示轨道图",
                                        "Enter a valid regex to show the graph",
                                    ),
                                    cx,
                                )
                                .into_any_element(),
                            ]
                        }
                    }),
            )
            .when(show_hint, |area| {
                area.child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .bg(cx.theme().background)
                                .py_1()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(lang.of(
                                    "↑ 可以通过点击或拖拽选中节点",
                                    "↑ You can select nodes by dragging or clicking on the graph",
                                )),
                        ),
                )
            })
            .when(has_error, |area| {
                area.child(
                    div()
                        .absolute()
                        .top_3()
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .px_4()
                        .child(
                            div()
                                .max_w(px(560.0))
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_3()
                                .py_2()
                                .rounded(px(8.0))
                                .border_1()
                                .border_color(cx.theme().danger)
                                .bg(cx.theme().background)
                                .child(
                                    Icon::new(IconName::TriangleAlert)
                                        .size(px(14.0))
                                        .flex_shrink_0()
                                        .text_color(cx.theme().danger),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .text_size(px(12.5))
                                        .text_color(cx.theme().danger)
                                        .child(error_text.clone()),
                                ),
                        ),
                )
            })
            .child(
                div()
                    .absolute()
                    .top_2()
                    .right_2()
                    .h(px(TOOLBAR_H))
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("regex-undo")
                            .icon(Icon::new(IconName::Undo2))
                            .ghost()
                            .compact()
                            .disabled(!can_undo)
                            .tooltip(lang.of("撤销", "Undo"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.undo(window, cx);
                            })),
                    )
                    .child(
                        Button::new("regex-redo")
                            .icon(Icon::new(IconName::Redo2))
                            .ghost()
                            .compact()
                            .disabled(!can_redo)
                            .tooltip(lang.of("重做", "Redo"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.redo(window, cx);
                            })),
                    )
                    .child(
                        Button::new("regex-lang")
                            .label(lang.badge())
                            .ghost()
                            .compact()
                            .tooltip(lang.tooltip())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_lang(window, cx);
                            })),
                    )
                    .child(
                        Button::new("regex-panel-toggle")
                            .icon(Icon::new(if self.panel_collapsed {
                                IconName::PanelLeftOpen
                            } else {
                                IconName::PanelLeftClose
                            }))
                            .ghost()
                            .compact()
                            .tooltip(lang.of("显示/隐藏右侧面板", "Show / hide the side panel"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.panel_collapsed = !this.panel_collapsed;
                                cx.notify();
                            })),
                    ),
            );

        let mut input_block = div()
            .max_w(px(896.0))
            .w_full()
            .h(px(INPUT_BLOCK_H))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.pattern_state).h(px(INPUT_H))),
                    )
                    .when(!pattern_empty && !flags_str.is_empty(), |row| {
                        row.child(
                            div()
                                .ml_2()
                                .h(px(INPUT_H - 8.0))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .px_3()
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().muted)
                                .font_family(mono_family.clone())
                                .text_sm()
                                .child(flags_str.clone()),
                        )
                    })
                    .child(
                        div()
                            .ml_3()
                            .h(px(INPUT_H))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("regex-copy")
                                    .icon(Icon::new(IconName::Copy))
                                    .outline()
                                    .h(px(INPUT_H))
                                    .w(px(INPUT_H))
                                    .rounded(px(8.0))
                                    .tooltip(lang.of("复制正则", "Copy regex"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.copy_pattern(window, cx);
                                    })),
                            ),
                    ),
            );

        if !pattern_empty {
            let mut flag_row = div()
                .flex()
                .items_center()
                .gap_3()
                .child(div().mr_2().text_sm().child(lang.of("标志: ", "Flags: ")));
            for &(flag_index, _, cn, en) in FLAGS.iter() {
                let label = lang.of(cn, en);
                let active = flags_state[flag_index];
                flag_row = flag_row.child(
                    div()
                        .id(("regex-flag", flag_index))
                        .flex()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_flag(flag_index, cx);
                        }))
                        .child(
                            div()
                                .size(px(16.0))
                                .flex_shrink_0()
                                .rounded(px(4.0))
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(if active {
                                    cx.theme().border
                                } else {
                                    cx.theme().background
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .when(active, |box_el| {
                                    box_el.child(
                                        Icon::new(IconName::Check)
                                            .size(px(11.0))
                                            .text_color(cx.theme().background),
                                    )
                                }),
                        )
                        .child(div().text_sm().child(label)),
                );
            }
            input_block = input_block.child(flag_row);
        }

        let left_column = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(diagram_area)
            .child(
                div()
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .px_4()
                    .py_8()
                    .flex()
                    .justify_center()
                    .child(input_block),
            );

        let mut tab_bar = TabBar::new("regex-panel-tabs")
            .segmented()
            .w_full()
            .selected_index(panel_tab)
            .on_click(cx.listener(move |this, index: &usize, window, cx| {
                this.set_panel_tab(*index, window, cx);
            }));
        for (cn, en) in PANEL_TABS.iter() {
            tab_bar = tab_bar.child(Tab::new().flex_1().label(self.lang.of(cn, en)));
        }

        let panel_body: AnyElement = match panel_tab {
            TAB_LEGEND => {
                let mut list = div().flex().flex_col();
                for (gi, group) in self.legend.iter().enumerate() {
                    list = list
                        .when(gi > 0, |l| {
                            l.child(div().my_4().h(px(1.0)).w_full().bg(cx.theme().border))
                        })
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_semibold()
                                .mb_2()
                                .child(group.title),
                        );
                    for entry in group.entries.iter() {
                        list = list
                            .child(div().mb_3().child(diagram_canvas(&entry.diagram, cx)))
                            .child(
                                div()
                                    .mb_2()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(entry.desc),
                            );
                    }
                }
                list.into_any_element()
            }
            TAB_EDIT => self.edit_panel(cx),
            TAB_TEST => {
                let (status_icon, status_color, status_text) = if !error.is_empty() {
                    (IconName::CircleX, cx.theme().danger, error_text.clone())
                } else if pattern_empty {
                    (
                        IconName::CircleCheck,
                        cx.theme().muted_foreground,
                        lang.of("输入正则后测试", "Enter a regex to test")
                            .to_string(),
                    )
                } else if match_count > 0 {
                    (
                        IconName::CircleCheck,
                        Hsla::from(rgb(OK_GREEN)),
                        match lang {
                            Lang::Cn => format!("{match_count} 个匹配"),
                            Lang::En => format!("{match_count} matches"),
                        },
                    )
                } else {
                    (
                        IconName::CircleX,
                        cx.theme().muted_foreground,
                        lang.of("无匹配", "No match").to_string(),
                    )
                };

                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(editor_label(lang.of("测试文本", "Test text"), cx))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        Button::new("regex-paste")
                                            .icon(Icon::new(IconName::File))
                                            .ghost()
                                            .compact()
                                            .tooltip(lang.of("粘贴测试文本", "Paste test text"))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.paste_text(window, cx);
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("regex-clear")
                                            .icon(Icon::new(IconName::Delete))
                                            .ghost()
                                            .compact()
                                            .tooltip(lang.of("清空全部", "Clear all"))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.clear(window, cx);
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(Textarea::new(&self.text_state).h(px(96.0)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .pl_3()
                            .pr_2()
                            .py_1p5()
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded(px(8.0))
                            .bg(cx.theme().muted)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .min_w_0()
                                    .child(
                                        Icon::new(status_icon)
                                            .size(px(14.0))
                                            .flex_shrink_0()
                                            .text_color(status_color),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .min_w_0()
                                            .truncate()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(status_text),
                                    ),
                            )
                            .child(pill(format!("{match_count}"), cx)),
                    )
                    .child(highlight_preview_panel(&self.test_text, &matches, cx, lang))
                    .child(match_panel(matches, cx, lang))
                    .into_any_element()
            }
            _ => {
                let mut list = div().flex().flex_col().gap_1p5();
                for (i, (cn, en, sample)) in SAMPLES.iter().enumerate() {
                    let name = lang.of(cn, en);
                    let sample: &'static str = sample;
                    list = list.child(
                        div()
                            .id(("regex-sample", i))
                            .rounded(px(6.0))
                            .border_1()
                            .border_color(cx.theme().border)
                            .p_3()
                            .cursor_pointer()
                            .hover(|style| style.bg(cx.theme().accent))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.apply_sample(sample, window, cx);
                            }))
                            .child(div().text_sm().font_medium().child(name.to_string()))
                            .child(
                                div()
                                    .mt_1()
                                    .font_family(mono_family.clone())
                                    .text_size(px(12.0))
                                    .text_color(Hsla::from(rgb(0x2dd4bf)))
                                    .child(sample.to_string()),
                            ),
                    );
                }
                list.into_any_element()
            }
        };

        let right_panel = div()
            .w(px(305.0))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .py_4()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .child(div().mx_4().mb_6().child(tab_bar))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(div().px_4().pb_4().child(panel_body)),
            );

        div()
            .h_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .rounded(px(8.0))
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                let modifiers = &ev.keystroke.modifiers;
                match ev.keystroke.key.as_str() {
                    "z" if modifiers.control || modifiers.platform => {
                        if modifiers.shift {
                            this.redo(window, cx);
                        } else {
                            this.undo(window, cx);
                        }
                        cx.stop_propagation();
                    }
                    "backspace" | "delete" if !this.selected.is_empty() => {
                        this.delete_selected(window, cx);
                        cx.stop_propagation();
                    }
                    "escape" if !this.selected.is_empty() => {
                        this.clear_selection(window, cx);
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(left_column)
                    .when(!self.panel_collapsed, |row| row.child(right_panel)),
            )
    }
}

pub(crate) fn match_panel(
    matches: Vec<RegexMatch>,
    cx: &mut Context<RegexVisualizer>,
    lang: Lang,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(editor_label(lang.of("匹配结果", "Matches"), cx))
        .child(
            div()
                .max_h(px(220.0))
                .overflow_y_scrollbar()
                .children(if matches.is_empty() {
                    vec![hint(lang.of("暂无匹配", "No matches yet"), cx)]
                } else {
                    matches
                        .into_iter()
                        .map(|item| {
                            div()
                                .border_1()
                                .border_color(cx.theme().border)
                                .rounded(px(8.0))
                                .px_2()
                                .py_1p5()
                                .mb_1p5()
                                .flex()
                                .items_baseline()
                                .gap_2()
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_family("monospace")
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!(
                                            "#{} [{}..{}]",
                                            item.index, item.start, item.end
                                        )),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_family("monospace")
                                        .font_semibold()
                                        .child(item.text),
                                )
                                .children(item.groups.into_iter().map(|group| {
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(group)
                                }))
                        })
                        .collect::<Vec<_>>()
                }),
        )
}

pub(crate) fn highlight_preview_panel(
    text: &str,
    matches: &[RegexMatch],
    cx: &mut Context<RegexVisualizer>,
    lang: Lang,
) -> Div {
    let palette = HighlightPalette::default_light();
    let highlight_color = palette.boolean;
    let mut ranges: Vec<Range<usize>> = matches.iter().map(|m| m.start..m.end).collect();
    ranges.sort_by_key(|r| r.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for r in ranges {
        if let Some(last) = merged.last_mut()
            && r.start <= last.end
        {
            last.end = last.end.max(r.end);
            continue;
        }
        merged.push(r);
    }

    let highlight_ranges: Vec<syntax_highlight::HighlightRange> = merged
        .into_iter()
        .map(|r| syntax_highlight::HighlightRange {
            range: r,
            color: highlight_color,
        })
        .collect();

    let styled = syntax_highlight::styled_text(text, highlight_ranges);

    div()
        .flex()
        .flex_col()
        .gap_1p5()
        .child(editor_label("匹配高亮", cx))
        .child(
            div()
                .max_h(px(140.0))
                .overflow_y_scrollbar()
                .rounded(px(8.0))
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .p_2()
                .text_sm()
                .font_family("monospace")
                .when(text.is_empty(), |this| {
                    this.text_color(cx.theme().muted_foreground)
                        .child(lang.of("暂无测试文本", "No test text"))
                })
                .when(!text.is_empty(), |this| this.child(styled)),
        )
}
