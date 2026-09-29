//! 树的绘制：语法树、调用树等都用它。
//!
//! 布局算法很朴素：叶子节点从左到右依次占一个"槽位"，父节点放在第一个和最后
//! 一个孩子的正中间，纵坐标由深度决定。节点可以是一片森林（语法分析进行到一半时，
//! 已经建好的是若干棵小树），按顺序并排摆放。

use eframe::egui::{self, Color32, CornerRadius, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::theme::{Palette, mono};

pub struct TreeNode {
    pub label: String,
    pub children: Vec<usize>,
}

#[derive(Clone, Copy)]
pub struct NodeStyle {
    pub visible: bool,
    pub fill: Color32,
    pub stroke: Color32,
    /// 粗边框，用于"当前步骤涉及的节点"。
    pub emphasized: bool,
}

impl NodeStyle {
    pub fn normal(p: &Palette) -> Self {
        Self { visible: true, fill: p.node_fill, stroke: p.node_stroke, emphasized: false }
    }

    pub fn hidden() -> Self {
        Self { visible: false, fill: Color32::TRANSPARENT, stroke: Color32::TRANSPARENT, emphasized: false }
    }
}

const SLOT_W: f32 = 52.0;
const LEVEL_H: f32 = 62.0;
const MARGIN: f32 = 16.0;

/// 画一棵树（或森林），返回鼠标悬停的节点。
pub fn tree_view(ui: &mut Ui, nodes: &[TreeNode], style: impl Fn(usize) -> NodeStyle) -> Option<usize> {
    let layout = Layout::compute(nodes);
    let size = Vec2::new(
        (layout.slots as f32 * SLOT_W).max(SLOT_W) + 2.0 * MARGIN,
        (layout.max_depth as f32 + 1.0) * LEVEL_H + MARGIN,
    );
    let (response, painter) = ui.allocate_painter(size, Sense::hover());
    let origin = response.rect.min;
    let center = |i: usize| {
        origin + Vec2::new(MARGIN + (layout.x[i] + 0.5) * SLOT_W, MARGIN + 14.0 + layout.depth[i] as f32 * LEVEL_H)
    };

    let p = Palette::of(ui);
    let styles: Vec<NodeStyle> = (0..nodes.len()).map(&style).collect();

    for (i, n) in nodes.iter().enumerate() {
        for &c in &n.children {
            if styles[i].visible && styles[c].visible {
                painter.line_segment([center(i), center(c)], Stroke::new(1.5, p.node_stroke));
            }
        }
    }

    let hover_pos = response.hover_pos();
    let mut hovered = None;
    for (i, n) in nodes.iter().enumerate() {
        let s = styles[i];
        if !s.visible {
            continue;
        }
        let galley = painter.layout_no_wrap(n.label.clone(), mono(16.0), p.text);
        let rect =
            Rect::from_center_size(center(i), (galley.size() + Vec2::new(18.0, 10.0)).max(Vec2::new(34.0, 28.0)));
        painter.rect_filled(rect, CornerRadius::same(8), s.fill);
        let stroke = if s.emphasized { Stroke::new(3.0, s.stroke) } else { Stroke::new(1.5, s.stroke) };
        painter.rect_stroke(rect, CornerRadius::same(8), stroke, StrokeKind::Inside);
        painter.galley(rect.center() - galley.size() / 2.0, galley, p.text);
        if hover_pos.is_some_and(|h| rect.contains(h)) {
            hovered = Some(i);
        }
    }
    hovered
}

struct Layout {
    x: Vec<f32>,
    depth: Vec<usize>,
    slots: usize,
    max_depth: usize,
}

impl Layout {
    fn compute(nodes: &[TreeNode]) -> Self {
        let mut is_child = vec![false; nodes.len()];
        for n in nodes {
            for &c in &n.children {
                is_child[c] = true;
            }
        }
        let mut layout = Layout { x: vec![0.0; nodes.len()], depth: vec![0; nodes.len()], slots: 0, max_depth: 0 };
        for root in (0..nodes.len()).filter(|&i| !is_child[i]) {
            layout.place(nodes, root, 0);
        }
        layout
    }

    fn place(&mut self, nodes: &[TreeNode], i: usize, depth: usize) {
        self.depth[i] = depth;
        self.max_depth = self.max_depth.max(depth);
        let children = &nodes[i].children;
        if children.is_empty() {
            self.x[i] = self.slots as f32;
            self.slots += 1;
        } else {
            for &c in children {
                self.place(nodes, c, depth + 1);
            }
            let first = self.x[children[0]];
            let last = self.x[*children.last().unwrap()];
            self.x[i] = (first + last) / 2.0;
        }
    }
}

/// 把一棵树的绘图区域放进可以横向滚动的容器里（树很宽时不会撑破页面）。
pub fn scrollable_tree(ui: &mut Ui, id: &str, nodes: &[TreeNode], style: impl Fn(usize) -> NodeStyle) -> Option<usize> {
    egui::ScrollArea::horizontal().id_salt(id).show(ui, |ui| tree_view(ui, nodes, style)).inner
}
