//! # 垃圾回收模拟器
//!
//! 用一段小脚本描述程序对堆的操作，然后用不同的回收算法"运行"它，记录每一步之后整个堆的样子。
//!
//! ```text
//! new A          分配一个对象 A（每个对象有两个指针域 .0 和 .1）
//! root x = A     根变量 x 指向 A（根就是栈上、全局的变量）
//! A.0 = B        A 的第 0 个域指向 B
//! A.1 = null     清空
//! gc             做一次完整的回收
//! gc begin       （增量回收）开始标记
//! mark 2         （增量回收）处理 2 个灰色对象
//! gc end         （增量回收）标记完剩下的，然后清除
//! ```
//!
//! 三种算法：
//! - **引用计数**：每个对象记着有多少指针指向它，计数变成 0 立刻释放。简单、及时，但回收不了环。
//! - **标记-清除**：从根出发把能走到的对象都标记上，没标记的全部回收。能回收环，但要"停下全世界"。
//! - **三色增量标记**：把标记拆成很多小步，和程序交替执行。白 = 还没见到，灰 = 见到了但还没扫描它的域，
//!   黑 = 扫描完了。程序在标记途中改指针可能让回收器漏掉对象，所以需要**写屏障**。

use bsc_core::{Diagnostic, Span};

pub type ObjId = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    White,
    Gray,
    Black,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Obj {
    pub name: String,
    pub fields: [Option<ObjId>; 2],
    pub alive: bool,
    /// 引用计数（只有引用计数算法用）。
    pub rc: u32,
    pub color: Color,
    /// 还被引用着却被回收了（回收器出错的标志）。
    pub lost: bool,
}

/// 某一步之后堆的完整状态。
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub objs: Vec<Obj>,
    pub roots: Vec<(String, Option<ObjId>)>,
    /// 正在执行脚本的第几行（从 0 数）。
    pub line: usize,
    pub note: String,
    /// 这一步涉及的对象。
    pub focus: Vec<ObjId>,
    /// 这一步涉及的指针：(对象, 域)。
    pub edge: Option<(ObjId, usize)>,
    /// 增量标记是否正在进行。
    pub marking: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Algorithm {
    RefCount,
    MarkSweep,
    /// 三色增量标记；参数表示是否启用写屏障。
    TriColor {
        barrier: bool,
    },
}

#[derive(Clone, Debug)]
pub struct Trace {
    pub snaps: Vec<Snapshot>,
    pub error: Option<Diagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Target {
    Null,
    Obj(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Op {
    New(String),
    Root(String, Target),
    Field(String, usize, Target),
    Gc,
    GcBegin,
    Mark(usize),
    GcEnd,
}

fn parse(src: &str) -> Result<Vec<(usize, Span, Op)>, Diagnostic> {
    let mut out = Vec::new();
    let mut offset = 0;
    for (ln, raw) in src.split('\n').enumerate() {
        let span = Span::new(offset, offset + raw.len());
        offset += raw.len() + 1;
        let line = raw.split("//").next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let err = |msg: &str| {
            Diagnostic::error(format!("看不懂这一行：{msg}"))
                .with_code("E1401")
                .with_primary(span, "")
                .with_help("支持的写法：new A、root x = A、A.0 = B、A.1 = null、gc、gc begin、mark 2、gc end")
        };
        let target = |t: &str| -> Result<Target, Diagnostic> {
            let t = t.trim();
            if t == "null" {
                Ok(Target::Null)
            } else if !t.is_empty() && t.chars().all(|c| c.is_alphanumeric() || c == '_') {
                Ok(Target::Obj(t.to_owned()))
            } else {
                Err(err("等号右边应该是对象名或 null"))
            }
        };
        let words: Vec<&str> = line.split_whitespace().collect();
        let op = match words.as_slice() {
            ["new", name] => Op::New((*name).to_owned()),
            ["gc"] => Op::Gc,
            ["gc", "begin"] => Op::GcBegin,
            ["gc", "end"] => Op::GcEnd,
            ["mark", n] => Op::Mark(n.parse().map_err(|_| err("mark 后面应该是一个数"))?),
            ["root", rest @ ..] => {
                let rest = rest.join(" ");
                let (name, t) = rest.split_once('=').ok_or_else(|| err("缺少 ="))?;
                Op::Root(name.trim().to_owned(), target(t)?)
            }
            _ => {
                let (lhs, t) = line.split_once('=').ok_or_else(|| err("不认识的操作"))?;
                let (obj, field) = lhs.trim().split_once('.').ok_or_else(|| err("左边应该写成 对象.域"))?;
                let field: usize = field.trim().parse().map_err(|_| err("域只能是 0 或 1"))?;
                if field > 1 {
                    return Err(err("每个对象只有两个域：.0 和 .1"));
                }
                Op::Field(obj.trim().to_owned(), field, target(t)?)
            }
        };
        out.push((ln, span, op));
    }
    Ok(out)
}

/// 从根出发能走到的对象。
pub fn reachable(objs: &[Obj], roots: &[(String, Option<ObjId>)]) -> Vec<bool> {
    let mut seen = vec![false; objs.len()];
    let mut stack: Vec<ObjId> = roots.iter().filter_map(|(_, r)| *r).collect();
    while let Some(o) = stack.pop() {
        if seen[o] {
            continue;
        }
        seen[o] = true;
        stack.extend(objs[o].fields.iter().flatten());
    }
    seen
}

struct Heap {
    alg: Algorithm,
    objs: Vec<Obj>,
    roots: Vec<(String, Option<ObjId>)>,
    snaps: Vec<Snapshot>,
    line: usize,
    marking: bool,
    gray: Vec<ObjId>,
}

impl Heap {
    fn snap(&mut self, note: String, focus: Vec<ObjId>, edge: Option<(ObjId, usize)>) {
        self.snaps.push(Snapshot {
            objs: self.objs.clone(),
            roots: self.roots.clone(),
            line: self.line,
            note,
            focus,
            edge,
            marking: self.marking,
        });
    }

    fn name(&self, o: ObjId) -> String {
        self.objs[o].name.clone()
    }

    fn lookup(&self, name: &str, span: Span) -> Result<ObjId, Diagnostic> {
        match self.objs.iter().position(|o| o.name == name) {
            Some(i) if self.objs[i].alive => Ok(i),
            Some(_) => Err(Diagnostic::error(format!("对象 {name} 已经被回收了，不能再使用"))
                .with_code("E1403")
                .with_primary(span, "")
                .with_note("在真实程序里这就是\"悬空指针\"：回收器出了错，或者程序用了已经释放的内存")),
            None => Err(Diagnostic::error(format!("没有叫 {name} 的对象"))
                .with_code("E1402")
                .with_primary(span, "")
                .with_help(format!("先用 `new {name}` 分配它"))),
        }
    }

    fn target(&self, t: &Target, span: Span) -> Result<Option<ObjId>, Diagnostic> {
        match t {
            Target::Null => Ok(None),
            Target::Obj(n) => self.lookup(n, span).map(Some),
        }
    }

    // ------------------------------------------------------------ 引用计数

    fn inc(&mut self, o: Option<ObjId>) {
        if let Some(o) = o {
            self.objs[o].rc += 1;
        }
    }

    fn dec(&mut self, o: Option<ObjId>) {
        let Some(o) = o else { return };
        self.objs[o].rc -= 1;
        if self.objs[o].rc == 0 {
            self.objs[o].alive = false;
            let name = self.name(o);
            self.snap(format!("{name} 的引用计数变成 0，立即释放；它指向的对象计数都减 1。"), vec![o], None);
            let kids = self.objs[o].fields;
            for k in kids {
                self.dec(k);
            }
        }
    }

    // ------------------------------------------------------------ 标记

    fn shade(&mut self, o: Option<ObjId>) -> bool {
        if let Some(o) = o
            && self.objs[o].alive
            && self.objs[o].color == Color::White
        {
            self.objs[o].color = Color::Gray;
            self.gray.push(o);
            return true;
        }
        false
    }

    fn begin_marking(&mut self) {
        for o in &mut self.objs {
            o.color = Color::White;
        }
        self.gray.clear();
        self.marking = true;
        let roots: Vec<Option<ObjId>> = self.roots.iter().map(|(_, r)| *r).collect();
        let mut shaded = Vec::new();
        for r in roots {
            if self.shade(r) {
                shaded.push(r.unwrap());
            }
        }
        let names: Vec<String> = shaded.iter().map(|&o| self.name(o)).collect();
        self.snap(
            format!(
                "开始标记：所有对象先是白色；根直接指向的 {} 标成灰色（见到了，还没扫描）。",
                if names.is_empty() { "（没有）".to_owned() } else { names.join("、") }
            ),
            shaded,
            None,
        );
    }

    /// 处理一个灰色对象，没有灰色对象时返回 false。
    fn mark_one(&mut self) -> bool {
        let Some(o) = self.gray.pop() else { return false };
        self.objs[o].color = Color::Black;
        let kids = self.objs[o].fields;
        let mut shaded = Vec::new();
        for k in kids {
            if self.shade(k) {
                shaded.push(k.unwrap());
            }
        }
        let names: Vec<String> = shaded.iter().map(|&x| self.name(x)).collect();
        let tail = if names.is_empty() {
            "它的域没有指向白色对象。".to_owned()
        } else {
            format!("它指向的白色对象 {} 变成灰色。", names.join("、"))
        };
        let mut focus = vec![o];
        focus.extend(shaded);
        self.snap(format!("扫描灰色的 {}：变成黑色，{tail}", self.name(o)), focus, None);
        true
    }

    fn sweep(&mut self) {
        let reach = reachable(&self.objs, &self.roots);
        let mut freed = Vec::new();
        for i in 0..self.objs.len() {
            if self.objs[i].alive && self.objs[i].color == Color::White {
                self.objs[i].alive = false;
                self.objs[i].lost = reach[i];
                freed.push(i);
            }
        }
        self.marking = false;
        let names: Vec<String> = freed.iter().map(|&o| self.name(o)).collect();
        let lost: Vec<String> = freed.iter().filter(|&&o| self.objs[o].lost).map(|&o| self.name(o)).collect();
        let mut note = if names.is_empty() {
            "清除：没有白色对象，什么都不用回收。".to_owned()
        } else {
            format!("清除：仍是白色的 {} 没人能走到，回收。", names.join("、"))
        };
        if !lost.is_empty() {
            note = format!(
                "清除：回收了白色的 {}。出错了！{} 其实还被引用着——回收器把活着的对象当成了垃圾，程序再用它就会出错。",
                names.join("、"),
                lost.join("、")
            );
        }
        self.snap(note, freed, None);
        for o in &mut self.objs {
            if o.alive {
                o.color = Color::White;
            }
        }
    }

    /// 写屏障（Dijkstra 插入屏障）：标记期间，把指针写进对象时，若目标是白色就标成灰色。
    fn barrier(&mut self, target: Option<ObjId>) {
        if let Algorithm::TriColor { barrier: true } = self.alg
            && self.marking
            && self.shade(target)
        {
            let n = self.name(target.unwrap());
            self.snap(
                format!("写屏障：标记正在进行，新写入的指针指向白色的 {n}，把它标成灰色，保证不会漏掉。"),
                vec![target.unwrap()],
                None,
            );
        }
    }

    fn exec(&mut self, span: Span, op: &Op) -> Result<(), Diagnostic> {
        let rc = self.alg == Algorithm::RefCount;
        match op {
            Op::New(name) => {
                if self.objs.iter().any(|o| o.name == *name) {
                    return Err(Diagnostic::error(format!("对象 {name} 已经存在"))
                        .with_code("E1402")
                        .with_primary(span, "")
                        .with_help("换一个名字"));
                }
                // 标记期间新分配的对象直接标黑：本轮不回收它
                let color = if self.marking { Color::Black } else { Color::White };
                self.objs.push(Obj {
                    name: name.clone(),
                    fields: [None, None],
                    alive: true,
                    rc: 0,
                    color,
                    lost: false,
                });
                let o = self.objs.len() - 1;
                let note = if self.marking {
                    format!("分配 {name}。标记正在进行，新对象直接标成黑色（这一轮不回收它）。")
                } else if rc {
                    format!("分配 {name}，引用计数为 0（还没有指针指向它）。")
                } else {
                    format!("分配 {name}。")
                };
                self.snap(note, vec![o], None);
            }
            Op::Root(name, t) => {
                let new = self.target(t, span)?;
                let old = match self.roots.iter_mut().find(|(n, _)| n == name) {
                    Some((_, r)) => std::mem::replace(r, new),
                    None => {
                        self.roots.push((name.clone(), new));
                        None
                    }
                };
                self.barrier(new);
                let what = new.map_or("null".to_owned(), |o| self.name(o));
                let mut focus: Vec<ObjId> = new.into_iter().collect();
                focus.extend(old);
                if rc {
                    self.inc(new);
                    let tail = if old.is_some() || new.is_some() { "，指向的对象计数随之增减" } else { "" };
                    self.snap(format!("根 {name} = {what}{tail}。"), focus, None);
                    self.dec(old);
                } else {
                    self.snap(format!("根 {name} = {what}。"), focus, None);
                }
            }
            Op::Field(obj, f, t) => {
                let o = self.lookup(obj, span)?;
                let new = self.target(t, span)?;
                let old = std::mem::replace(&mut self.objs[o].fields[*f], new);
                self.barrier(new);
                let what = new.map_or("null".to_owned(), |x| self.name(x));
                let mut focus = vec![o];
                focus.extend(new);
                if rc {
                    self.inc(new);
                    self.snap(format!("{obj}.{f} = {what}。"), focus, Some((o, *f)));
                    self.dec(old);
                } else {
                    let warn = match (self.alg, self.marking, new) {
                        (Algorithm::TriColor { barrier: false }, true, Some(x))
                            if self.objs[o].color == Color::Black && self.objs[x].color == Color::White =>
                        {
                            format!("（危险：黑色的 {obj} 指向了白色的 {what}，而黑色对象不会再被扫描）")
                        }
                        _ => String::new(),
                    };
                    self.snap(format!("{obj}.{f} = {what}。{warn}"), focus, Some((o, *f)));
                }
            }
            Op::Gc => match self.alg {
                Algorithm::RefCount => {
                    let reach = reachable(&self.objs, &self.roots);
                    let leaked: Vec<ObjId> =
                        (0..self.objs.len()).filter(|&i| self.objs[i].alive && !reach[i]).collect();
                    let names: Vec<String> = leaked.iter().map(|&o| self.name(o)).collect();
                    let note = if leaked.is_empty() {
                        "引用计数不需要专门的回收阶段：计数一到 0 就释放了。现在堆里没有垃圾。".to_owned()
                    } else {
                        format!(
                            "引用计数没有专门的回收阶段。但 {} 已经没有根能走到，计数却都不是 0——它们互相指着（环），永远不会被释放：内存泄漏。",
                            names.join("、")
                        )
                    };
                    self.snap(note, leaked, None);
                }
                _ => {
                    if !self.marking {
                        self.begin_marking();
                    }
                    while self.mark_one() {}
                    self.sweep();
                }
            },
            Op::GcBegin => {
                if !matches!(self.alg, Algorithm::TriColor { .. }) {
                    return Err(Diagnostic::error("gc begin 只用于三色增量标记")
                        .with_code("E1401")
                        .with_primary(span, ""));
                }
                self.begin_marking();
            }
            Op::Mark(n) => {
                if !self.marking {
                    return Err(Diagnostic::error("还没有 gc begin").with_code("E1401").with_primary(span, ""));
                }
                for _ in 0..*n {
                    if !self.mark_one() {
                        self.snap("没有灰色对象了，标记已经完成，等待 gc end 清除。".to_owned(), vec![], None);
                        break;
                    }
                }
            }
            Op::GcEnd => {
                if !self.marking {
                    return Err(Diagnostic::error("还没有 gc begin").with_code("E1401").with_primary(span, ""));
                }
                while self.mark_one() {}
                self.sweep();
            }
        }
        Ok(())
    }
}

/// 用指定算法运行脚本。出错时返回已经执行的部分和错误。
pub fn run(src: &str, alg: Algorithm) -> Trace {
    let ops = match parse(src) {
        Ok(o) => o,
        Err(e) => return Trace { snaps: vec![], error: Some(e) },
    };
    let mut h = Heap { alg, objs: vec![], roots: vec![], snaps: vec![], line: 0, marking: false, gray: vec![] };
    h.snap("开始：堆是空的。".to_owned(), vec![], None);
    for (ln, span, op) in &ops {
        h.line = *ln;
        if let Err(e) = h.exec(*span, op) {
            return Trace { snaps: h.snaps, error: Some(e) };
        }
    }
    Trace { snaps: h.snaps, error: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 随机生成的脚本：任何算法都不能回收还活着的对象；标记-清除回收后，活着的恰好是能走到的。
    #[test]
    fn collectors_are_safe_on_random_programs() {
        let mut seed = 7u64;
        let mut rand = |m: usize| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as usize % m
        };
        for _ in 0..200 {
            let mut lines = Vec::new();
            let names = ["A", "B", "C", "D", "E", "F"];
            let mut made = 0;
            let mut marking = false;
            for _ in 0..30 {
                let pick = |r: usize| if r == made { "null".to_owned() } else { names[r].to_owned() };
                match rand(9) {
                    0 | 1 if made < names.len() => {
                        lines.push(format!("new {}", names[made]));
                        made += 1;
                    }
                    2 if made > 0 => lines.push(format!("root {} = {}", ["x", "y"][rand(2)], pick(rand(made + 1)))),
                    3 | 4 | 5 if made > 0 => {
                        lines.push(format!("{}.{} = {}", names[rand(made)], rand(2), pick(rand(made + 1))))
                    }
                    6 if !marking => {
                        lines.push("gc begin".to_owned());
                        marking = true;
                    }
                    7 if marking => lines.push(format!("mark {}", 1 + rand(2))),
                    8 if marking => {
                        lines.push("gc end".to_owned());
                        marking = false;
                    }
                    _ => {}
                }
            }
            if marking {
                lines.push("gc end".to_owned());
            }
            let tri = lines.join("\n");
            let full: String = lines
                .iter()
                .filter(|l| !l.starts_with("gc") && !l.starts_with("mark"))
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
                + "\ngc";
            // 对象被回收以后，脚本还可能引用它的名字（报错 E1403）；只检查报错之前的部分
            for (src, alg) in [
                (&full, Algorithm::RefCount),
                (&full, Algorithm::MarkSweep),
                (&tri, Algorithm::TriColor { barrier: true }),
            ] {
                let t = run(src, alg);
                for s in &t.snaps {
                    assert!(s.objs.iter().all(|o| !o.lost), "{alg:?} 回收了活对象：\n{src}");
                    let reach = reachable(&s.objs, &s.roots);
                    assert!(s.objs.iter().zip(&reach).all(|(o, r)| o.alive || !r), "{alg:?}：\n{src}");
                }
                if alg == Algorithm::MarkSweep && t.error.is_none() {
                    let last = t.snaps.last().unwrap();
                    let reach = reachable(&last.objs, &last.roots);
                    assert!(last.objs.iter().zip(&reach).all(|(o, r)| o.alive == *r), "标记-清除没回收干净：\n{src}");
                }
            }
        }
    }

    /// 经典例子：没有写屏障时，增量标记会把活着的对象当成垃圾回收掉。
    #[test]
    fn barrier_prevents_lost_object() {
        let src = "new A\nnew B\nnew C\nroot x = A\nA.0 = B\nB.0 = C\ngc begin\nmark 1\nA.1 = C\nB.0 = null\ngc end";
        let lost =
            |barrier| run(src, Algorithm::TriColor { barrier }).snaps.last().unwrap().objs.iter().any(|o| o.lost);
        assert!(lost(false));
        assert!(!lost(true));
        // 引用计数回收不了环
        let cyc = run("new A\nnew B\nroot x = A\nA.0 = B\nB.0 = A\nroot x = null\ngc", Algorithm::RefCount);
        assert!(cyc.snaps.last().unwrap().objs.iter().all(|o| o.alive));
    }
}
