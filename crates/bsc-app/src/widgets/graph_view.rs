//! 状态图（有限自动机）的布局与绘制。
//!
//! 布局沿用教科书的画法：从左往右流动。
//! 1. 从起点深度优先遍历，找出"回边"（指回祖先的边，比如 `*` 产生的循环）；
//! 2. 去掉回边后图没有环，用"最长路径"给每个状态分层——层号就是横坐标；
//! 3. 同一层内按前驱的平均位置排序（重心法），减少交叉。
//!
//! 边：相邻层之间画直线，回边、跨层边和成对的双向边画成弧线，自环画在状态上方。

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2,
    epaint::QuadraticBezierShape,
};

use crate::theme::{Palette, mono};

pub struct Graph {
    pub nodes: Vec<GNode>,
    pub edges: Vec<GEdge>,
    pub start: Option<usize>,
    /// 是否画"开始"箭头（布局总是以 `start` 为起点，不受影响）。
    pub show_start: bool,
}

pub struct GNode {
    pub label: String,
    pub accepting: bool,
    /// 画在状态下方的小字（比如 DFA 状态对应的 NFA 状态集合）。
    pub note: Option<String>,
}

pub struct GEdge {
    pub from: usize,
    pub to: usize,
    pub label: String,
}

#[derive(Clone, Copy)]
pub struct GNodeStyle {
    pub visible: bool,
    pub fill: Color32,
    pub stroke: Color32,
    pub emphasized: bool,
}

#[derive(Clone, Copy)]
pub struct GEdgeStyle {
    pub visible: bool,
    pub color: Color32,
    pub emphasized: bool,
}

impl GNodeStyle {
    pub fn normal(p: &Palette) -> Self {
        Self { visible: true, fill: p.node_fill, stroke: p.node_stroke, emphasized: false }
    }
    pub fn hidden() -> Self {
        Self { visible: false, fill: Color32::TRANSPARENT, stroke: Color32::TRANSPARENT, emphasized: false }
    }
    pub fn focus(p: &Palette) -> Self {
        Self { visible: true, fill: p.focus_bg, stroke: p.accent, emphasized: true }
    }
    pub fn filled(p: &Palette, fill: Color32) -> Self {
        Self { fill, ..Self::normal(p) }
    }
}

impl GEdgeStyle {
    pub fn normal(p: &Palette) -> Self {
        Self { visible: true, color: p.node_stroke, emphasized: false }
    }
    pub fn hidden() -> Self {
        Self { visible: false, color: Color32::TRANSPARENT, emphasized: false }
    }
    pub fn focus(p: &Palette) -> Self {
        Self { visible: true, color: p.accent, emphasized: true }
    }
}

const R: f32 = 19.0; // 状态圆圈半径
const LAYER_GAP: f32 = 96.0;
const ROW_GAP: f32 = 72.0;
const MARGIN: f32 = 44.0;
/// 上下留白：要容得下自环和弯曲的回边。
const V_MARGIN: f32 = 58.0;
const MAX_BEND: f32 = 96.0;

/// 布局结果：每个状态的坐标（以左上角为原点）和整张图的大小。
pub struct GraphLayout {
    pos: Vec<Pos2>,
    layer: Vec<usize>,
    size: Vec2,
}

impl GraphLayout {
    pub fn compute(g: &Graph) -> Self {
        let n = g.nodes.len();
        if n == 0 {
            return Self { pos: vec![], layer: vec![], size: Vec2::new(MARGIN * 2.0, MARGIN * 2.0) };
        }
        let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
        for e in &g.edges {
            if e.from != e.to && !out[e.from].contains(&e.to) {
                out[e.from].push(e.to);
            }
        }

        // 1. 深度优先遍历：记录发现顺序，找出回边。
        let mut order = Vec::with_capacity(n);
        let mut state = vec![0u8; n]; // 0 未访问，1 在栈上，2 已完成
        let mut back: Vec<(usize, usize)> = Vec::new();
        let mut finish = Vec::with_capacity(n);
        let roots: Vec<usize> = g.start.into_iter().chain(0..n).collect();
        for root in roots {
            if state[root] != 0 {
                continue;
            }
            // 显式栈：(节点, 下一条要看的出边下标)
            let mut stack = vec![(root, 0usize)];
            state[root] = 1;
            order.push(root);
            while let Some((v, i)) = stack.last_mut() {
                let v = *v;
                if let Some(&w) = out[v].get(*i) {
                    *i += 1;
                    match state[w] {
                        0 => {
                            state[w] = 1;
                            order.push(w);
                            stack.push((w, 0));
                        }
                        1 => back.push((v, w)),
                        _ => {}
                    }
                } else {
                    state[v] = 2;
                    finish.push(v);
                    stack.pop();
                }
            }
        }

        // 2. 最长路径分层（按拓扑序，也就是完成顺序的逆序）。
        let mut layer = vec![0usize; n];
        for &v in finish.iter().rev() {
            for &w in &out[v] {
                if !back.contains(&(v, w)) {
                    layer[w] = layer[w].max(layer[v] + 1);
                }
            }
        }

        // 3. 层内排序：先按发现顺序，再做几轮重心排序。
        let num_layers = layer.iter().max().unwrap() + 1;
        let mut layers: Vec<Vec<usize>> = vec![Vec::new(); num_layers];
        for &v in &order {
            layers[layer[v]].push(v);
        }
        let mut rank = vec![0f32; n];
        let assign = |layers: &Vec<Vec<usize>>, rank: &mut Vec<f32>| {
            for l in layers {
                for (i, &v) in l.iter().enumerate() {
                    rank[v] = i as f32 - (l.len() as f32 - 1.0) / 2.0;
                }
            }
        };
        assign(&layers, &mut rank);
        let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (v, ws) in out.iter().enumerate() {
            for &w in ws {
                if layer[v] < layer[w] {
                    preds[w].push(v);
                }
            }
        }
        for _ in 0..4 {
            for l in layers.iter_mut().skip(1) {
                let key = |v: &usize| {
                    let p = &preds[*v];
                    if p.is_empty() { rank[*v] } else { p.iter().map(|&u| rank[u]).sum::<f32>() / p.len() as f32 }
                };
                l.sort_by(|a, b| key(a).total_cmp(&key(b)));
            }
            assign(&layers, &mut rank);
        }

        // 状态下方的注释（比如 NFA 状态集合）可能很长：层间距至少要放得下最长的注释（按 11 号字每字符约 6 像素估算）。
        let longest_note = g.nodes.iter().filter_map(|n| n.note.as_ref()).map(|s| s.chars().count()).max().unwrap_or(0);
        let gap = LAYER_GAP.max(longest_note as f32 * 6.0 + 14.0);
        // 有自环时同一层的状态要隔得更开：自环和它的标签画在状态上方，会碰到上面那个状态的注释。
        let row_gap = if g.edges.iter().any(|e| e.from == e.to) { ROW_GAP + 26.0 } else { ROW_GAP };
        let max_rows = layers.iter().map(Vec::len).max().unwrap_or(1) as f32;
        let height = (max_rows - 1.0) * row_gap + 2.0 * V_MARGIN + 16.0;
        let pos = (0..n)
            .map(|v| Pos2::new(MARGIN + 16.0 + layer[v] as f32 * gap, height / 2.0 + rank[v] * row_gap))
            .collect();
        let size = Vec2::new(MARGIN * 2.0 + 16.0 + (num_layers as f32 - 1.0) * gap + gap / 2.0, height);
        Self { pos, layer, size }
    }
}

/// 画状态图，返回鼠标悬停的状态。图比可用宽度宽时先缩小（最多到 60%），再不够就横向滚动。
pub fn graph_view(
    ui: &mut Ui,
    id: &str,
    g: &Graph,
    node_style: impl Fn(usize) -> GNodeStyle,
    edge_style: impl Fn(usize) -> GEdgeStyle,
) -> Option<usize> {
    let layout = GraphLayout::compute(g);
    let scale = (ui.available_width() / layout.size.x).clamp(0.6, 1.0);
    egui::ScrollArea::horizontal()
        .id_salt(id)
        .show(ui, |ui| draw(ui, g, &layout, scale, &node_style, &edge_style))
        .inner
}

fn draw(
    ui: &mut Ui,
    g: &Graph,
    layout: &GraphLayout,
    scale: f32,
    node_style: &dyn Fn(usize) -> GNodeStyle,
    edge_style: &dyn Fn(usize) -> GEdgeStyle,
) -> Option<usize> {
    let p = Palette::of(ui);
    let (resp, painter) = ui.allocate_painter(layout.size * scale, Sense::hover());
    let origin = resp.rect.min;
    let at = |v: usize| origin + (layout.pos[v].to_vec2()) * scale;
    let r = R * scale;
    let font = FontId::proportional(13.0 * scale.max(0.8));
    let styles: Vec<GNodeStyle> = (0..g.nodes.len()).map(node_style).collect();

    // 合并同一对状态之间的平行边：标签用逗号连接，样式取"最显眼"的那条。
    let mut merged: Vec<(usize, usize, Vec<String>, GEdgeStyle)> = Vec::new();
    for (i, e) in g.edges.iter().enumerate() {
        let s = edge_style(i);
        if !s.visible || !styles[e.from].visible || !styles[e.to].visible {
            continue;
        }
        match merged.iter_mut().find(|m| m.0 == e.from && m.1 == e.to) {
            Some(m) => {
                if !m.2.contains(&e.label) {
                    m.2.push(e.label.clone());
                }
                if s.emphasized && !m.3.emphasized {
                    m.3 = s;
                }
            }
            None => merged.push((e.from, e.to, vec![e.label.clone()], s)),
        }
    }

    // 起始箭头
    if let Some(s) = g.start
        && g.show_start
        && styles[s].visible
    {
        let c = at(s);
        let from = c - Vec2::new(r + 30.0 * scale, 0.0);
        arrow_line(&painter, from, c - Vec2::new(r, 0.0), Stroke::new(1.8, p.text));
        let mid = from + (c - from) * 0.4;
        painter.text(mid - Vec2::new(0.0, 4.0), Align2::CENTER_BOTTOM, "开始", FontId::proportional(12.0), p.muted);
    }

    let mut labels: Vec<(Pos2, String, GEdgeStyle)> = Vec::new();
    for (from, to, texts, s) in &merged {
        let stroke = Stroke::new(if s.emphasized { 2.6 } else { 1.4 }, s.color);
        let text = texts.join(", ");
        if from == to {
            // 自环：画在状态上方的一段贝塞尔曲线
            let c = at(*from);
            let a = c + Vec2::new(-r * 0.6, -r * 0.8);
            let b = c + Vec2::new(r * 0.6, -r * 0.8);
            let ctrl1 = c + Vec2::new(-r * 1.6, -r * 3.0);
            let ctrl2 = c + Vec2::new(r * 1.6, -r * 3.0);
            painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [a, ctrl1, ctrl2, b],
                false,
                Color32::TRANSPARENT,
                stroke,
            ));
            arrow_head(&painter, b, (b - ctrl2).normalized(), stroke);
            labels.push((c + Vec2::new(0.0, -r * 2.6), text, *s));
            continue;
        }
        let (a, b) = (at(*from), at(*to));
        let (la, lb) = (layout.layer[*from], layout.layer[*to]);
        let reverse_exists = merged.iter().any(|m| m.0 == *to && m.1 == *from);
        let d = b - a;
        let normal = Vec2::new(-d.y, d.x).normalized();
        // 弯曲程度：相邻层的正向边画直线；双向边各弯一点；回边和跨层边弯得多，避免穿过中间的状态。
        let bend = if lb == la + 1 && !reverse_exists {
            0.0
        } else if lb > la {
            if reverse_exists { 0.18 } else { 0.22 }
        } else {
            0.32
        } * d.length();
        // 弧线离弦的最大距离是 bend/2，限制在上下留白之内，避免画出界。
        let bend = bend.min(MAX_BEND * scale);
        let ctrl = a + d / 2.0 + normal * bend;
        let start = a + (ctrl - a).normalized() * r;
        let end = b + (ctrl - b).normalized() * r;
        if bend == 0.0 {
            arrow_line(&painter, start, end, stroke);
        } else {
            painter.add(QuadraticBezierShape::from_points_stroke(
                [start, ctrl, end],
                false,
                Color32::TRANSPARENT,
                stroke,
            ));
            arrow_head(&painter, end, (end - ctrl).normalized(), stroke);
        }
        let mid = start * 0.25 + ctrl.to_vec2() * 0.5 + end.to_vec2() * 0.25;
        labels.push((mid, text, *s));
    }

    // 状态
    let hover = resp.hover_pos();
    let mut hovered = None;
    for (v, node) in g.nodes.iter().enumerate() {
        let s = styles[v];
        if !s.visible {
            continue;
        }
        let c = at(v);
        painter.circle_filled(c, r, s.fill);
        painter.circle_stroke(c, r, Stroke::new(if s.emphasized { 3.0 } else { 1.6 }, s.stroke));
        if node.accepting {
            painter.circle_stroke(c, r - 4.0 * scale, Stroke::new(1.4, s.stroke));
        }
        painter.text(c, Align2::CENTER_CENTER, &node.label, mono(14.0 * scale.max(0.8)), p.text);
        if let Some(note) = &node.note {
            painter.text(c + Vec2::new(0.0, r + 4.0), Align2::CENTER_TOP, note, FontId::proportional(11.0), p.muted);
        }
        if hover.is_some_and(|h| h.distance(c) <= r) {
            hovered = Some(v);
        }
    }

    // 边上的字最后画，带底色，保证压在线上也看得清。
    for (pos, text, s) in labels {
        // 被淡化的边（颜色半透明），标签也跟着淡化
        let color = if s.emphasized || s.color.a() < 200 { s.color } else { p.text };
        let galley = painter.layout_no_wrap(text, font.clone(), color);
        let rect = Rect::from_center_size(pos, galley.size() + Vec2::new(6.0, 2.0));
        painter.rect_filled(rect, CornerRadius::same(3), p.card_bg.gamma_multiply(0.92));
        painter.galley(rect.center() - galley.size() / 2.0, galley, color);
    }
    hovered
}

fn arrow_line(painter: &egui::Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    painter.line_segment([from, to], stroke);
    arrow_head(painter, to, (to - from).normalized(), stroke);
}

fn arrow_head(painter: &egui::Painter, tip: Pos2, dir: Vec2, stroke: Stroke) {
    let size = 9.0;
    let back = tip - dir * size;
    let normal = Vec2::new(-dir.y, dir.x) * (size * 0.45);
    painter.add(Shape::convex_polygon(vec![tip, back + normal, back - normal], stroke.color, Stroke::NONE));
}
