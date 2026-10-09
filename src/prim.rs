use crate::ast::*;

#[derive(Clone)]
pub(crate) enum Cmd {
    Move(f32, f32),
    Line(f32, f32),

    Arc {
        r: f32,
        large: bool,
        sweep: bool,
        to: (f32, f32),
    },

    Close,
}

#[derive(Clone)]
pub(crate) enum Prim {
    RoundRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        stroke: u32,
        dash: bool,
    },

    Frame {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },

    Highlight {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        stroke: u32,
    },
    Curve {
        cmds: Vec<Cmd>,
        stroke: u32,

        filled: bool,
    },
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
        stroke: u32,
    },
    Text {
        x: f32,
        y: f32,
        w: f32,
        text: String,
        fs: f32,
        color: u32,
        center: bool,
    },
}

#[derive(Clone, Copy)]
pub(crate) struct HitBox {
    pub(crate) id: NodeId,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,

    pub(crate) depth: u16,
}

impl HitBox {
    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
}

#[derive(Clone, Default)]
pub(crate) struct Diagram {
    pub(crate) prims: Vec<Prim>,

    pub(crate) hits: Vec<HitBox>,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

pub(crate) struct GNode {
    pub(crate) kind: GKind,

    pub(crate) id: NodeId,

    pub(crate) w: f32,

    pub(crate) h: f32,

    pub(crate) cw: f32,

    pub(crate) ch: f32,
}

pub(crate) enum GKind {
    Token {
        text: String,
        label: Option<String>,
        dash: bool,
    },

    Repeat {
        child: Box<GNode>,
        label: String,

        label_text_w: f32,

        label_w: f32,
        infinite: bool,
    },

    Group {
        child: Box<GNode>,
        label: Option<String>,
    },
    Concat(Vec<GNode>),

    Alternate(Vec<GNode>),
}
