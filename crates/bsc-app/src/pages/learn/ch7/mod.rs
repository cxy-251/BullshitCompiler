//! 第 7 章 · 运行时：垃圾回收。

pub mod mark;
pub mod rc;
pub mod tricolor;

use bsc_core::Span;
use bsc_runtime::{Algorithm, Color, Snapshot, Trace, run};
use eframe::egui::{self, RichText, TextEdit, Ui};

use crate::theme::{Palette, mono};
use crate::widgets::{
    GEdge, GEdgeStyle, GNode, GNodeStyle, Graph, Mark, Stepper, card, diagnostic_view, graph_view, prose_sized,
    source_view,
};

/// 第 7 章各课的状态。
#[derive(Default)]
pub struct Chapter {
    pub rc: rc::Lesson,
    pub mark: mark::Lesson,
    pub tricolor: tricolor::Lesson,
}

/// 一个可编辑脚本、可单步回放的垃圾回收演示。
pub struct GcDemo {
    id: &'static str,
    pub src: String,
    pub alg: Algorithm,
    trace: Trace,
    stepper: Stepper,
}

impl GcDemo {
    pub fn new(id: &'static str, src: &str, alg: Algorithm) -> Self {
        let mut d = Self {
            id,
            src: src.to_owned(),
            alg,
            trace: Trace { snaps: vec![], error: None },
            stepper: Stepper::default(),
        };
        d.rerun();
        d
    }

    pub fn rerun(&mut self) {
        self.trace = run(&self.src, self.alg);
        self.stepper.reset();
    }

    pub fn ui(&mut self, ui: &mut Ui, presets: &[(&str, &str)]) {
        let p = Palette::of(ui);
        let changed = card(ui, |ui| {
            let mut changed = false;
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("例子：").color(p.muted));
                for (name, code) in presets {
                    if ui.button(*name).clicked() {
                        self.src = (*code).to_owned();
                        changed = true;
                    }
                }
            });
            // 下面的回放视图已经显示了整段脚本，编辑框默认收起
            changed |= egui::CollapsingHeader::new("自己动手改脚本")
                .id_salt(("gc_editor", self.id))
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut self.src)
                            .font(mono(15.0))
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    )
                    .changed()
                })
                .body_returned
                .unwrap_or(false);
            changed
        });
        if changed {
            self.rerun();
        }
        if let Some(e) = &self.trace.error {
            diagnostic_view(ui, &self.src, e);
        }
        if self.trace.snaps.is_empty() {
            return;
        }
        self.stepper.ui(ui, self.trace.snaps.len() - 1);
        let s = &self.trace.snaps[self.stepper.pos];

        // 脚本里当前这一行
        let mut marks = Vec::new();
        if self.stepper.pos > 0 {
            let mut off = 0;
            for (i, line) in self.src.split('\n').enumerate() {
                if i == s.line {
                    marks.push(Mark { span: Span::new(off, off + line.len()), bg: p.focus_bg });
                }
                off += line.len() + 1;
            }
        }
        if ui.available_width() > 760.0 {
            ui.columns(2, |c| {
                c[0].label(RichText::new("程序（脚本）").strong());
                card(&mut c[0], |ui| {
                    source_view(ui, &self.src, &[], &marks, 15.0);
                });
                c[1].label(RichText::new("这一步").strong());
                card(&mut c[1], |ui| {
                    prose_sized(ui, &s.note, 16.0);
                });
            });
        } else {
            card(ui, |ui| {
                source_view(ui, &self.src, &[], &marks, 15.0);
            });
            prose_sized(ui, &s.note, 16.0);
        }
        heap_view(ui, self.id, s, self.alg);
    }
}

/// 堆的样子：根和对象画成图，指针画成边。
fn heap_view(ui: &mut Ui, id: &str, s: &Snapshot, alg: Algorithm) {
    let p = Palette::of(ui);
    let nr = s.roots.len();
    let mut nodes: Vec<GNode> = s
        .roots
        .iter()
        .map(|(n, _)| GNode { label: n.clone(), accepting: false, note: Some("根".to_owned()) })
        .collect();
    let mut edges = Vec::new();
    let mut edge_ids: Vec<Option<(usize, usize)>> = Vec::new();
    for (i, (_, r)) in s.roots.iter().enumerate() {
        if let Some(o) = r {
            edges.push(GEdge { from: i, to: nr + o, label: String::new() });
            edge_ids.push(None);
        }
    }
    for (i, o) in s.objs.iter().enumerate() {
        let note = if !o.alive {
            if o.lost { "误回收！".to_owned() } else { "已回收".to_owned() }
        } else {
            match alg {
                Algorithm::RefCount => format!("计数 {}", o.rc),
                // 标记过程中（以及清除那一步）标出颜色
                _ if s.marking || s.objs.iter().any(|x| x.color != Color::White) => match o.color {
                    Color::White => "白".to_owned(),
                    Color::Gray => "灰".to_owned(),
                    Color::Black => "黑".to_owned(),
                },
                _ => String::new(),
            }
        };
        nodes.push(GNode { label: o.name.clone(), accepting: false, note: (!note.is_empty()).then_some(note) });
        if o.alive || o.lost {
            for (f, t) in o.fields.iter().enumerate() {
                if let Some(t) = t {
                    edges.push(GEdge { from: nr + i, to: nr + t, label: format!(".{f}") });
                    edge_ids.push(Some((i, f)));
                }
            }
        }
    }
    let g = Graph { nodes, edges, start: (nr > 0).then_some(0), show_start: false };
    card(ui, |ui| {
        if s.objs.is_empty() && s.roots.is_empty() {
            ui.label(RichText::new("（堆是空的）").color(p.muted));
            return;
        }
        graph_view(
            ui,
            id,
            &g,
            |n| {
                if n < nr {
                    return GNodeStyle::filled(&p, p.code_bg);
                }
                let i = n - nr;
                let o = &s.objs[i];
                let mut st = if o.lost {
                    GNodeStyle::filled(&p, p.error_bg)
                } else if !o.alive {
                    GNodeStyle { visible: true, fill: p.card_bg, stroke: p.card_stroke, emphasized: false }
                } else {
                    match o.color {
                        Color::White => GNodeStyle::normal(&p),
                        Color::Gray => GNodeStyle::filled(&p, p.card_stroke),
                        Color::Black => GNodeStyle::filled(&p, p.muted),
                    }
                };
                if s.focus.contains(&i) {
                    st.stroke = p.accent;
                    st.emphasized = true;
                }
                st
            },
            |e| {
                if edge_ids[e].is_some() && edge_ids[e] == s.edge {
                    GEdgeStyle::focus(&p)
                } else {
                    GEdgeStyle::normal(&p)
                }
            },
        );
    });
}
