use gpui_kit::{component::ActiveTheme as _, *};

use crate::{
    ast::*,
    consts::*,
    i18n::*,
    prim::*,
    tree::{node_label, node_text},
    view::RegexVisualizer,
};

fn shape_mono(
    text: &str,
    fs: f32,
    color: Hsla,
    window: &mut Window,
    family: &SharedString,
) -> ShapedLine {
    let font = Font {
        family: family.clone(),
        ..window.text_style().font()
    };
    let run = TextRun {
        len: text.len(),
        font,
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(SharedString::from(text.to_string()), px(fs), &[run], None)
}

fn mono_text_width(text: &str, fs: f32, window: &mut Window, family: &SharedString) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    shape_mono(text, fs, Hsla::from(rgb(0x000000)), window, family)
        .width()
        .as_f32()
}

pub(crate) fn quote_pad(text: &str) -> f32 {
    (text.matches('"').count() / 2) as f32 * 4.0
}

fn label_node_w(text: &str, window: &mut Window, family: &SharedString) -> f32 {
    mono_text_width(text, NODE_FS, window, family) + NODE_PAD_X * 2.0
}

pub(crate) fn token_node(
    text: String,
    label: Option<String>,
    dash: bool,
    window: &mut Window,
    family: &SharedString,
) -> GNode {
    let cw = mono_text_width(&text, NODE_FS, window, family) + quote_pad(&text) + NODE_PAD_X * 2.0;
    let cw = cw.max(MIN_W);
    let ch = NODE_H.max(MIN_H);

    let label_w = match &label {
        Some(l) => label_node_w(l, window, family),
        None => 0.0,
    };
    let dh = if label_w > 0.0 { LABEL_BOX * 2.0 } else { 0.0 };
    GNode {
        id: 0,
        kind: GKind::Token { text, label, dash },
        w: cw.max(label_w),
        h: ch + dh,
        cw,
        ch,
    }
}

fn repeat_label(
    min: u32,
    max: Option<u32>,
    window: &mut Window,
    family: &SharedString,
) -> (String, f32, f32, bool) {
    let (text, infinite) = match max {
        Some(m) if m == min => (format!(" {min}"), false),
        Some(m) => (format!(" {min} - {m}"), false),
        None => (format!(" {min} - "), true),
    };
    let text_w = mono_text_width(&text, LABEL_FS, window, family);
    let total = ICON_W + text_w + if infinite { ICON_W } else { 0.0 };
    (text, text_w, total, infinite)
}

#[allow(clippy::too_many_arguments)]
fn place(
    node: &GNode,
    x: f32,
    cy: f32,
    depth: u16,
    out: &mut Vec<Prim>,
    hits: &mut Vec<HitBox>,
    selected: &[NodeId],
    inherited_sel: bool,
) -> f32 {
    let cx = x + (node.w - node.cw) / 2.0;
    let top = cy - node.ch / 2.0;

    let is_selected = inherited_sel || (node.id != 0 && selected.contains(&node.id));
    let mark = |hits: &mut Vec<HitBox>, id: NodeId, cx: f32, top: f32, w: f32, h: f32| {
        if id != 0 {
            hits.push(HitBox {
                id,
                x: cx,
                y: top,
                w,
                h,
                depth,
            });
        }
    };

    if node.w > node.cw {
        out.push(Prim::Line {
            x1: x,
            y1: cy,
            x2: cx,
            y2: cy,
            stroke: 0x000000,
        });
        out.push(Prim::Line {
            x1: cx + node.cw,
            y1: cy,
            x2: x + node.w,
            y2: cy,
            stroke: 0x000000,
        });
    }

    match &node.kind {
        GKind::Token { text, label, dash } => {
            if let Some(label) = label {
                out.push(Prim::Text {
                    x,
                    y: top - LABEL_H,
                    w: node.w,
                    text: label.clone(),
                    fs: LABEL_FS,
                    color: GRAPH_FG_SENTINEL,
                    center: true,
                });
            }
            out.push(Prim::RoundRect {
                x: cx,
                y: top,
                w: node.cw,
                h: node.ch,
                r: NODE_R,

                stroke: if *dash { GRAPH_RED_SENTINEL } else { 0x000000 },
                dash: *dash,
            });
            if is_selected {
                out.push(select_highlight(cx, top, node.cw, node.ch));
            }
            mark(hits, node.id, cx, top, node.cw, node.ch);
            out.push(Prim::Text {
                x: cx + NODE_PAD_X,
                y: top + (node.ch - NODE_FS * 1.5) / 2.0,
                w: node.cw - NODE_PAD_X * 2.0,
                text: text.clone(),
                fs: NODE_FS,
                color: GRAPH_FG_SENTINEL,
                center: true,
            });
        }
        GKind::Repeat {
            child,
            label,
            label_text_w,
            label_w,
            infinite,
        } => {
            mark(hits, node.id, cx, top, node.cw, node.ch);

            place(child, cx, cy, depth + 1, out, hits, selected, is_selected);

            let ty = top + node.ch + (LABEL_H - LABEL_FS * 1.5) / 2.0;
            let mut cursor = x + (node.w - label_w) / 2.0;
            let icon_oy = ty + LABEL_FS * 0.75 - ICON_W / 2.0;
            out.push(Prim::Curve {
                cmds: repeat_icon_cmds(cursor, icon_oy, ICON_W / 24.0),
                stroke: 0x000000,
                filled: false,
            });
            cursor += ICON_W;
            out.push(Prim::Text {
                x: cursor,
                y: ty,
                w: *label_text_w,
                text: label.clone(),
                fs: LABEL_FS,
                color: GRAPH_FG_SENTINEL,
                center: false,
            });
            if *infinite {
                out.push(Prim::Curve {
                    cmds: infinity_cmds(
                        cursor + label_text_w + ICON_W / 2.0,
                        ty + LABEL_FS * 0.75,
                        ICON_W,
                    ),
                    stroke: 0x000000,
                    filled: true,
                });
            }
        }
        GKind::Group { child, label } => {
            if let Some(label) = label {
                out.push(Prim::Text {
                    x,
                    y: top - LABEL_H,
                    w: node.w,
                    text: label.clone(),
                    fs: LABEL_FS,
                    color: GRAPH_FG_SENTINEL,
                    center: true,
                });
            }
            out.push(Prim::RoundRect {
                x: cx,
                y: top,
                w: node.cw,
                h: node.ch,
                r: NODE_R,
                stroke: GRAPH_GROUP_SENTINEL,
                dash: false,
            });
            if is_selected {
                out.push(select_highlight(cx, top, node.cw, node.ch));
            }
            mark(hits, node.id, cx, top, node.cw, node.ch);
            let inner = cx + GROUP_PAD_H;
            out.push(Prim::Line {
                x1: cx,
                y1: cy,
                x2: inner,
                y2: cy,
                stroke: 0x000000,
            });
            out.push(Prim::Line {
                x1: inner + child.w,
                y1: cy,
                x2: cx + node.cw,
                y2: cy,
                stroke: 0x000000,
            });
            place(
                child,
                inner,
                cy,
                depth + 1,
                out,
                hits,
                selected,
                is_selected,
            );
        }
        GKind::Concat(items) => {
            let mut cursor = x;
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(Prim::Line {
                        x1: cursor,
                        y1: cy,
                        x2: cursor + LINK,
                        y2: cy,
                        stroke: 0x000000,
                    });
                    cursor += LINK;
                }
                place(
                    item,
                    cursor,
                    cy,
                    depth + 1,
                    out,
                    hits,
                    selected,
                    inherited_sel,
                );
                cursor += item.w;
            }
        }
        GKind::Alternate(rows) => {
            let top = cy - node.h / 2.0;
            let right = x + node.w;
            out.push(Prim::Frame {
                x,
                y: top,
                w: node.w,
                h: node.h,
            });

            if is_selected {
                out.push(select_highlight(x, top, node.w, node.h));
            }
            mark(hits, node.id, x, top, node.w, node.h);
            let mut row_top = top + BRANCH_PAD_V;
            for row in rows {
                let row_x = x + (node.w - row.w) / 2.0;
                let row_cy = row_top + row.h / 2.0;
                out.push(Prim::Curve {
                    cmds: split_cmds(x, cy, row_cy, row_x, BRANCH_R),
                    stroke: 0x000000,
                    filled: false,
                });
                place(
                    row,
                    row_x,
                    row_cy,
                    depth + 1,
                    out,
                    hits,
                    selected,
                    inherited_sel,
                );
                out.push(Prim::Curve {
                    cmds: merge_cmds(row_x + row.w, row_cy, cy, right, BRANCH_R),
                    stroke: 0x000000,
                    filled: false,
                });
                row_top += row.h + ROW_GAP;
            }
        }
    }
    node.w
}

pub(crate) fn split_cmds(x: f32, cy: f32, row_cy: f32, end_x: f32, r: f32) -> Vec<Cmd> {
    if (row_cy - cy).abs() < 0.5 {
        return vec![Cmd::Move(x, cy), Cmd::Line(end_x, row_cy)];
    }
    let down = row_cy > cy;
    let mid = if down { cy + r } else { cy - r };
    let near = if down { row_cy - r } else { row_cy + r };
    vec![
        Cmd::Move(x, cy),
        Cmd::Line(x + 10.0, cy),
        Cmd::Arc {
            r,
            large: false,
            sweep: down,
            to: (x + 15.0, mid),
        },
        Cmd::Line(x + 15.0, near),
        Cmd::Arc {
            r,
            large: false,
            sweep: !down,
            to: (x + 20.0, row_cy),
        },
        Cmd::Line(end_x, row_cy),
    ]
}

pub(crate) fn merge_cmds(
    row_right: f32,
    row_cy: f32,
    cy: f32,
    container_right: f32,
    r: f32,
) -> Vec<Cmd> {
    if (row_cy - cy).abs() < 0.5 {
        return vec![Cmd::Move(row_right, row_cy), Cmd::Line(container_right, cy)];
    }

    let above = cy > row_cy;
    let near = if above { row_cy + r } else { row_cy - r };
    let mid = if above { cy - r } else { cy + r };
    let ax = container_right - 15.0;
    vec![
        Cmd::Move(row_right, row_cy),
        Cmd::Line(container_right - 20.0, row_cy),
        Cmd::Arc {
            r,
            large: false,
            sweep: above,
            to: (ax, near),
        },
        Cmd::Line(ax, mid),
        Cmd::Arc {
            r,
            large: false,
            sweep: !above,
            to: (container_right - 10.0, cy),
        },
        Cmd::Line(container_right, cy),
    ]
}

fn mv(p: (f32, f32)) -> Cmd {
    Cmd::Move(p.0, p.1)
}

fn ln(p: (f32, f32)) -> Cmd {
    Cmd::Line(p.0, p.1)
}

fn repeat_icon_cmds(ox: f32, oy: f32, s: f32) -> Vec<Cmd> {
    let p = |x: f32, y: f32| (ox + x * s, oy + y * s);
    let r = 4.0 * s;
    vec![
        mv(p(17.0, 1.0)),
        ln(p(21.0, 5.0)),
        ln(p(17.0, 9.0)),
        mv(p(3.0, 11.0)),
        ln(p(3.0, 9.0)),
        Cmd::Arc {
            r,
            large: false,
            sweep: true,
            to: p(7.0, 5.0),
        },
        ln(p(21.0, 5.0)),
        mv(p(21.0, 13.0)),
        ln(p(21.0, 15.0)),
        Cmd::Arc {
            r,
            large: false,
            sweep: true,
            to: p(17.0, 19.0),
        },
        ln(p(3.0, 19.0)),
        mv(p(7.0, 23.0)),
        ln(p(3.0, 19.0)),
        ln(p(7.0, 15.0)),
    ]
}

fn infinity_cmds(cx: f32, cy: f32, size: f32) -> Vec<Cmd> {
    let s = size / 256.0;

    let ox = cx - 128.0 * s;
    let oy = cy - 128.0 * s;
    let p = |x: f32, y: f32| (ox + x * s, oy + y * s);
    let r = |v: f32| v * s;
    vec![
        mv(p(248.0, 128.0)),
        Cmd::Arc {
            r: r(56.0),
            large: false,
            sweep: true,
            to: p(152.4, 167.6),
        },
        ln(p(152.07, 167.25)),
        ln(p(92.12, 99.55)),
        Cmd::Arc {
            r: r(40.0),
            large: true,
            sweep: false,
            to: p(92.12, 156.45),
        },
        ln(p(100.64, 146.83)),
        Cmd::Arc {
            r: r(8.0),
            large: true,
            sweep: true,
            to: p(112.64, 157.44),
        },
        ln(p(103.95, 167.25)),
        ln(p(103.62, 167.6)),
        Cmd::Arc {
            r: r(56.0),
            large: true,
            sweep: true,
            to: p(103.62, 88.4),
        },
        ln(p(103.95, 88.75)),
        ln(p(163.9, 156.45)),
        Cmd::Arc {
            r: r(40.0),
            large: true,
            sweep: false,
            to: p(163.9, 99.55),
        },
        ln(p(155.38, 109.17)),
        Cmd::Arc {
            r: r(8.0),
            large: true,
            sweep: true,
            to: p(143.38, 98.56),
        },
        ln(p(152.07, 88.75)),
        ln(p(152.4, 88.4)),
        Cmd::Arc {
            r: r(56.0),
            large: false,
            sweep: true,
            to: p(248.0, 128.0),
        },
        Cmd::Close,
    ]
}

pub(crate) fn gnode_seq(
    nodes: &[ENode],
    lang: Lang,
    window: &mut Window,
    family: &SharedString,
) -> GNode {
    match nodes.len() {
        0 => token_node(empty_label(lang).to_string(), None, false, window, family),
        1 => gnode(&nodes[0], lang, window, family),
        _ => build_concat(
            nodes
                .iter()
                .map(|n| gnode(n, lang, window, family))
                .collect(),
        ),
    }
}

fn gnode(node: &ENode, lang: Lang, window: &mut Window, family: &SharedString) -> GNode {
    let mut layout = match &node.kind {
        EKind::Group { children, .. } | EKind::LookAround { children, .. } => {
            let child = gnode_seq(children, lang, window, family);
            group_node(node_label(node, lang), child, window, family)
        }
        EKind::Choice { branches } => build_alternate(
            branches
                .iter()
                .map(|b| gnode_seq(b, lang, window, family))
                .collect(),
        ),
        _ => token_node(
            node_text(node, lang),
            node_label(node, lang),
            node.dashed(),
            window,
            family,
        ),
    };

    match node.quantifier {
        Some(q) if node.accepts_quantifier() => {
            let (min, max) = match q.kind {
                QuantKind::Star => (0u32, None),
                QuantKind::Plus => (1u32, None),
                QuantKind::Question => (0u32, Some(1u32)),
                QuantKind::Custom => (q.min, if q.infinite() { None } else { Some(q.max) }),
            };
            let mut outer = repeat_node(layout, min, max, window, family);
            outer.id = node.id;
            outer
        }
        _ => {
            layout.id = node.id;
            layout
        }
    }
}

pub(crate) fn build_concat(items: Vec<GNode>) -> GNode {
    let w = items.iter().map(|i| i.w).sum::<f32>()
        + if items.is_empty() {
            0.0
        } else {
            LINK * (items.len() as f32 - 1.0)
        };
    let h = items.iter().map(|i| i.h).fold(0.0f32, f32::max);

    let w = w.max(MIN_W);
    let h = h.max(MIN_H);
    GNode {
        id: 0,
        kind: GKind::Concat(items),
        w,
        h,
        cw: w,
        ch: h,
    }
}

pub(crate) fn build_alternate(rows: Vec<GNode>) -> GNode {
    let max_w = rows.iter().map(|r| r.w).fold(0.0f32, f32::max);
    let h = rows.iter().map(|r| r.h).sum::<f32>()
        + if rows.is_empty() {
            0.0
        } else {
            ROW_GAP * (rows.len() as f32 - 1.0)
        }
        + BRANCH_PAD_V * 2.0;

    let w = (max_w + BRANCH_INDENT * 2.0).max(MIN_W);
    let h = h.max(MIN_H);
    GNode {
        id: 0,
        kind: GKind::Alternate(rows),
        w,
        h,
        cw: w,
        ch: h,
    }
}

pub(crate) fn group_node(
    label: Option<String>,
    child: GNode,
    window: &mut Window,
    family: &SharedString,
) -> GNode {
    let label_w = match &label {
        Some(l) => label_node_w(l, window, family),
        None => 0.0,
    };
    let cw = child.w + GROUP_PAD_H * 2.0;
    let ch = child.h + GROUP_PAD_V * 2.0;
    let dh = if label_w > 0.0 { LABEL_BOX * 2.0 } else { 0.0 };
    GNode {
        id: 0,
        kind: GKind::Group {
            child: Box::new(child),
            label,
        },
        w: cw.max(label_w),
        h: ch + dh,
        cw,
        ch,
    }
}

pub(crate) fn repeat_node(
    child: GNode,
    min: u32,
    max: Option<u32>,
    window: &mut Window,
    family: &SharedString,
) -> GNode {
    let (label, label_text_w, label_w, infinite) = repeat_label(min, max, window, family);
    let (cw, ch) = (child.cw, child.ch);
    let child_w = child.w;
    GNode {
        id: 0,
        kind: GKind::Repeat {
            child: Box::new(child),
            label,
            label_text_w,
            label_w,
            infinite,
        },
        w: child_w.max(label_w),
        h: ch + LABEL_BOX * 2.0,
        cw,
        ch,
    }
}

#[allow(dead_code)]
pub(crate) fn layout_diagram(root: &GNode) -> Diagram {
    layout_diagram_selected(root, &[])
}

pub(crate) fn layout_diagram_selected(root: &GNode, selected: &[NodeId]) -> Diagram {
    let left = ROOT_PAD + MARK_R * 2.0 + LINK;
    let width = root.w + left * 2.0;
    let height = root.h + ROOT_PAD * 2.0;
    let cy = height / 2.0;
    let mut prims = Vec::new();
    let mut hits = Vec::new();
    prims.push(Prim::Circle {
        cx: ROOT_PAD + MARK_R,
        cy,
        r: MARK_R,
        stroke: 0x000000,
    });
    prims.push(Prim::Line {
        x1: ROOT_PAD + MARK_R * 2.0,
        y1: cy,
        x2: left,
        y2: cy,
        stroke: 0x000000,
    });
    place(root, left, cy, 0, &mut prims, &mut hits, selected, false);
    prims.push(Prim::Line {
        x1: width - left,
        y1: cy,
        x2: width - ROOT_PAD - MARK_R * 2.0,
        y2: cy,
        stroke: 0x000000,
    });
    prims.push(Prim::Circle {
        cx: width - ROOT_PAD - MARK_R,
        cy,
        r: MARK_R,
        stroke: 0x000000,
    });
    Diagram {
        prims,
        hits,
        width,
        height,
    }
}

fn prims_bbox(prims: &[Prim]) -> (f32, f32, f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for prim in prims {
        let (x0, y0, x1, y1) = match prim {
            Prim::RoundRect { x, y, w, h, .. } => (*x, *y, x + w, y + h),
            Prim::Frame { x, y, w, h } => (*x, *y, x + w, y + h),
            Prim::Highlight { x, y, w, h, .. } => (*x, *y, x + w, y + h),
            Prim::Line { x1, y1, x2, y2, .. } => {
                (x1.min(*x2), y1.min(*y2), x1.max(*x2), y1.max(*y2))
            }
            Prim::Curve { cmds, .. } => {
                let mut b = (
                    f32::INFINITY,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    f32::NEG_INFINITY,
                );
                for c in cmds {
                    let (cx, cy) = match c {
                        Cmd::Move(x, y) | Cmd::Line(x, y) => (*x, *y),
                        Cmd::Arc { to, .. } => *to,
                        Cmd::Close => continue,
                    };
                    b.0 = b.0.min(cx);
                    b.1 = b.1.min(cy);
                    b.2 = b.2.max(cx);
                    b.3 = b.3.max(cy);
                }
                b
            }
            Prim::Circle { cx, cy, r, .. } => (cx - r, cy - r, cx + r, cy + r),
            Prim::Text { x, y, w, fs, .. } => (*x, *y, x + w, y + fs * 1.5),
        };
        min_x = min_x.min(x0);
        min_y = min_y.min(y0);
        max_x = max_x.max(x1);
        max_y = max_y.max(y1);
    }
    (min_x, min_y, max_x, max_y)
}

fn translate_prim(prim: &mut Prim, dx: f32, dy: f32) {
    match prim {
        Prim::RoundRect { x, y, .. } => {
            *x += dx;
            *y += dy;
        }
        Prim::Frame { x, y, .. } => {
            *x += dx;
            *y += dy;
        }
        Prim::Highlight { x, y, .. } => {
            *x += dx;
            *y += dy;
        }
        Prim::Line { x1, y1, x2, y2, .. } => {
            *x1 += dx;
            *y1 += dy;
            *x2 += dx;
            *y2 += dy;
        }
        Prim::Curve { cmds, .. } => {
            for c in cmds.iter_mut() {
                match c {
                    Cmd::Move(x, y) | Cmd::Line(x, y) => {
                        *x += dx;
                        *y += dy;
                    }
                    Cmd::Arc { to, .. } => {
                        to.0 += dx;
                        to.1 += dy;
                    }
                    Cmd::Close => {}
                }
            }
        }
        Prim::Circle { cx, cy, .. } => {
            *cx += dx;
            *cy += dy;
        }
        Prim::Text { x, y, .. } => {
            *x += dx;
            *y += dy;
        }
    }
}

pub(crate) fn layout_content(root: &GNode) -> Diagram {
    let mut prims = Vec::new();
    let mut hits = Vec::new();
    place(root, 0.0, 0.0, 0, &mut prims, &mut hits, &[], false);
    let (min_x, min_y, max_x, max_y) = prims_bbox(&prims);
    let dx = 10.0 - min_x;
    let dy = 10.0 - min_y;
    for prim in prims.iter_mut() {
        translate_prim(prim, dx, dy);
    }
    for hit in hits.iter_mut() {
        hit.x += dx;
        hit.y += dy;
    }
    Diagram {
        prims,
        hits,
        width: (max_x - min_x) + 20.0,
        height: (max_y - min_y) + 20.0,
    }
}

fn build_path(
    cmds: &[Cmd],
    origin: Point<Pixels>,
    dash: bool,
    filled: bool,
) -> Option<Path<Pixels>> {
    let mut builder = if filled {
        PathBuilder::fill()
    } else {
        PathBuilder::stroke(px(STROKE))
    };
    if dash {
        builder = builder.dash_array(&[px(4.0), px(2.0)]);
    }
    for cmd in cmds {
        match cmd {
            Cmd::Move(x, y) => builder.move_to(point(origin.x + px(*x), origin.y + px(*y))),
            Cmd::Line(x, y) => builder.line_to(point(origin.x + px(*x), origin.y + px(*y))),
            Cmd::Arc {
                r,
                large,
                sweep,
                to,
            } => builder.arc_to(
                point(px(*r), px(*r)),
                px(0.0),
                *large,
                *sweep,
                point(origin.x + px(to.0), origin.y + px(to.1)),
            ),
            Cmd::Close => builder.close(),
        }
    }
    builder.build().ok()
}

fn rounded_rect_cmds(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<Cmd> {
    let r = r.min(w / 2.0).min(h / 2.0);
    vec![
        Cmd::Move(x + r, y),
        Cmd::Line(x + w - r, y),
        Cmd::Arc {
            r,
            large: false,
            sweep: true,
            to: (x + w, y + r),
        },
        Cmd::Line(x + w, y + h - r),
        Cmd::Arc {
            r,
            large: false,
            sweep: true,
            to: (x + w - r, y + h),
        },
        Cmd::Line(x + r, y + h),
        Cmd::Arc {
            r,
            large: false,
            sweep: true,
            to: (x, y + h - r),
        },
        Cmd::Line(x, y + r),
        Cmd::Arc {
            r,
            large: false,
            sweep: true,
            to: (x + r, y),
        },
    ]
}

fn circle_cmds(cx: f32, cy: f32, r: f32) -> Vec<Cmd> {
    vec![
        Cmd::Move(cx - r, cy),
        Cmd::Arc {
            r,
            large: false,
            sweep: true,
            to: (cx + r, cy),
        },
        Cmd::Arc {
            r,
            large: false,
            sweep: true,
            to: (cx - r, cy),
        },
    ]
}

fn paint_diagram(
    prims: &[Prim],
    origin: Point<Pixels>,
    canvas_theme: &CanvasTheme,
    window: &mut Window,
    cx: &mut App,
    family: &SharedString,
) {
    for prim in prims {
        match prim {
            Prim::RoundRect {
                x,
                y,
                w,
                h,
                r,
                stroke,
                dash,
            } => {
                let cmds = rounded_rect_cmds(*x, *y, *w, *h, *r);
                if let Some(path) = build_path(&cmds, origin, *dash, false) {
                    let color = if *stroke == GRAPH_GROUP_SENTINEL {
                        canvas_theme.group_color()
                    } else {
                        canvas_theme.stroke_color(*stroke)
                    };
                    let color = if *stroke == GRAPH_RED_SENTINEL {
                        canvas_theme.error_color()
                    } else {
                        color
                    };
                    window.paint_path(path, color);
                }
            }

            Prim::Frame { .. } => {}
            Prim::Highlight { x, y, w, h, r } => {
                let cmds = rounded_rect_cmds(*x, *y, *w, *h, *r);
                if let Some(path) = build_path(&cmds, origin, false, true) {
                    let color = canvas_theme.selection_color();
                    window.paint_path(path, color);
                }
            }
            Prim::Line {
                x1,
                y1,
                x2,
                y2,
                stroke,
            } => {
                let cmds = [Cmd::Move(*x1, *y1), Cmd::Line(*x2, *y2)];
                if let Some(path) = build_path(&cmds, origin, false, false) {
                    window.paint_path(path, canvas_theme.stroke_color(*stroke));
                }
            }
            Prim::Curve {
                cmds,
                stroke,
                filled,
            } => {
                if let Some(path) = build_path(cmds, origin, false, *filled) {
                    window.paint_path(path, canvas_theme.stroke_color(*stroke));
                }
            }
            Prim::Circle {
                cx: ccx,
                cy: ccy,
                r,
                stroke,
            } => {
                if let Some(path) = build_path(&circle_cmds(*ccx, *ccy, *r), origin, false, false) {
                    window.paint_path(path, canvas_theme.stroke_color(*stroke));
                }
            }
            Prim::Text {
                x,
                y,
                w,
                text,
                fs,
                color,
                center,
            } => {
                if text.is_empty() {
                    continue;
                }
                let line = shape_mono(text, *fs, canvas_theme.text_color(*color), window, family);
                let width = line.width().as_f32();
                let ox = if *center {
                    origin.x + px(*x + (*w - width) / 2.0)
                } else {
                    origin.x + px(*x)
                };
                let _ = line.paint(
                    point(ox, origin.y + px(*y)),
                    px(*fs),
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
        }
    }
}

/// Sentinels stored in the layout prims. The canvas resolves each one to a
/// live theme color at paint time, so a theme switch immediately repaints.
const GRAPH_RED_SENTINEL: u32 = 0xef4444;
const GRAPH_GROUP_SENTINEL: u32 = 0xa1a1aa;
const GRAPH_FG_SENTINEL: u32 = 0x990000;

pub(crate) fn diagram_canvas(diagram: &Diagram, cx: &App) -> Div {
    let prims = diagram.prims.clone();
    let width = diagram.width;
    let height = diagram.height;
    let canvas_theme = CanvasTheme::of(cx.theme().is_dark(), cx);
    div()
        .w(px(width))
        .h(px(height))
        .flex_shrink_0()
        .rounded(px(8.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .child(
            canvas(
                move |_, _, _| (),
                move |bounds: Bounds<Pixels>, _, window: &mut Window, cx: &mut App| {
                    let family: SharedString = cx.theme().mono_font_family.clone();
                    paint_diagram(&prims, bounds.origin, &canvas_theme, window, cx, &family);
                },
            )
            .size_full(),
        )
}

pub(crate) fn interactive_diagram_canvas(
    diagram: &Diagram,
    marquee: Option<((f32, f32), (f32, f32))>,
    cx: &Context<RegexVisualizer>,
) -> Div {
    let prims = diagram.prims.clone();
    let width = diagram.width;
    let height = diagram.height;
    let canvas_theme = CanvasTheme::of(cx.theme().is_dark(), cx);

    let origin: std::rc::Rc<std::cell::Cell<(f32, f32)>> =
        std::rc::Rc::new(std::cell::Cell::new((0.0, 0.0)));
    let paint_origin = origin.clone();
    let down_origin = origin.clone();
    let move_origin = origin.clone();
    let up_origin = origin.clone();

    let mut wrapper = div()
        .w(px(width))
        .h(px(height))
        .flex_shrink_0()
        .relative()
        .rounded(px(8.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                let (ox, oy) = down_origin.get();
                let p = (f32::from(ev.position.x) - ox, f32::from(ev.position.y) - oy);

                match deepest_hit(&this.diagram, p.0, p.1) {
                    Some(id) => {
                        this.marquee = None;
                        this.select_node(id, window, cx);
                    }
                    None => {
                        this.marquee = Some((p, p));
                        cx.notify();
                    }
                }
            }),
        )
        .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| {
            if let Some((start, _)) = this.marquee {
                let (ox, oy) = move_origin.get();
                let p = (f32::from(ev.position.x) - ox, f32::from(ev.position.y) - oy);
                this.marquee = Some((start, p));
                cx.notify();
            }
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(move |this, ev: &MouseUpEvent, window, cx| {
                let Some((start, _)) = this.marquee.take() else {
                    return;
                };
                let (ox, oy) = up_origin.get();
                let end = (f32::from(ev.position.x) - ox, f32::from(ev.position.y) - oy);
                let (x0, x1) = (start.0.min(end.0), start.0.max(end.0));
                let (y0, y1) = (start.1.min(end.1), start.1.max(end.1));

                if x1 - x0 < 3.0 && y1 - y0 < 3.0 {
                    this.clear_selection(window, cx);
                    return;
                }
                let ids: Vec<NodeId> = this
                    .diagram
                    .as_ref()
                    .map(|d| {
                        d.hits
                            .iter()
                            .filter(|h| h.x < x1 && h.x + h.w > x0 && h.y < y1 && h.y + h.h > y0)
                            .map(|h| h.id)
                            .collect()
                    })
                    .unwrap_or_default();
                this.set_selection(ids, window, cx);
            }),
        )
        .child(
            canvas(
                move |_, _, _| (),
                move |bounds: Bounds<Pixels>, _, window: &mut Window, cx: &mut App| {
                    paint_origin.set((f32::from(bounds.origin.x), f32::from(bounds.origin.y)));
                    let family: SharedString = cx.theme().mono_font_family.clone();
                    paint_diagram(&prims, bounds.origin, &canvas_theme, window, cx, &family);
                },
            )
            .size_full(),
        );

    if let Some((start, current)) = marquee {
        let x = start.0.min(current.0);
        let y = start.1.min(current.1);
        let w = (current.0 - start.0).abs();
        let h = (current.1 - start.1).abs();
        wrapper = wrapper.child(
            div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(w))
                .h(px(h))
                .border_1()
                .border_color(cx.theme().primary)
                .bg(cx.theme().primary.opacity(0.15)),
        );
    }

    wrapper
}

pub(crate) fn deepest_hit(diagram: &Option<Diagram>, x: f32, y: f32) -> Option<NodeId> {
    diagram
        .as_ref()?
        .hits
        .iter()
        .filter(|h| h.contains(x, y))
        .max_by_key(|h| h.depth)
        .map(|h| h.id)
}
