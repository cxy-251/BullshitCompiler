//! Hindley–Milner 类型推导（算法 W 的"全局替换"写法，也叫算法 J）。
//!
//! 每种语法结构的规则：
//!
//! ```text
//! 整数 / true / false     int / bool
//! 名字 x                  查环境得到类型模式 ∀α.τ，把 α 换成新的类型变量（实例化）
//! fun x -> e              x 的类型先设为新变量 α；推出 e 的类型 τ；整体是 α → τ
//! f a                     推出 f 的类型 τf、a 的类型 τa；结果设为新变量 β；合一 τf = τa → β
//! let x = e1 in e2        推出 e1 的类型，泛化（环境里没出现的变量都加上 ∀），再推 e2
//! if c then a else b      合一 c = bool；合一 a = b
//! + - *                   两边合一成 int，结果 int；< 结果 bool；== 两边合一，结果 bool
//! ```
//!
//! **合一**：让两个类型相等需要什么条件？变量可以等于任何（不含它自己的）类型；
//! 两个函数类型相等，当且仅当参数和结果分别相等；int 和 bool 永远不相等。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use bsc_core::Diagnostic;

use crate::syntax::{BinOp, ExprId, ExprKind, Program};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    /// 类型变量（编号）。
    Var(u32),
    Int,
    Bool,
    /// 函数类型：参数 → 结果。
    Fun(Box<Ty>, Box<Ty>),
}

const GREEK: &[&str] = &["α", "β", "γ", "δ", "ζ", "η", "θ", "ι", "κ", "λ", "μ", "ν", "ξ", "π", "ρ", "σ", "τ", "φ", "χ", "ψ", "ω"];

/// 类型变量的名字：0 号是 α，1 号是 β……用完了就写成 t21、t22。
pub fn var_name(v: u32) -> String {
    GREEK.get(v as usize).map_or_else(|| format!("t{v}"), |s| (*s).to_owned())
}

impl Ty {
    fn fun(a: Ty, b: Ty) -> Ty {
        Ty::Fun(Box::new(a), Box::new(b))
    }

    /// 写法：`(α → β) → int`。
    pub fn show(&self) -> String {
        match self {
            Ty::Var(v) => var_name(*v),
            Ty::Int => "int".to_owned(),
            Ty::Bool => "bool".to_owned(),
            Ty::Fun(a, b) => {
                let left = if matches!(**a, Ty::Fun(..)) { format!("({})", a.show()) } else { a.show() };
                format!("{left} → {}", b.show())
            }
        }
    }

    fn vars(&self, out: &mut Vec<u32>) {
        match self {
            Ty::Var(v) => {
                if !out.contains(v) {
                    out.push(*v);
                }
            }
            Ty::Fun(a, b) => {
                a.vars(out);
                b.vars(out);
            }
            _ => {}
        }
    }

    fn occurs(&self, v: u32) -> bool {
        match self {
            Ty::Var(w) => *w == v,
            Ty::Fun(a, b) => a.occurs(v) || b.occurs(v),
            _ => false,
        }
    }

    fn rename(&self, map: &BTreeMap<u32, u32>) -> Ty {
        match self {
            Ty::Var(v) => Ty::Var(*map.get(v).unwrap_or(v)),
            Ty::Fun(a, b) => Ty::fun(a.rename(map), b.rename(map)),
            t => t.clone(),
        }
    }
}

/// 类型模式 ∀α β. τ：`vars` 里的变量可以被换成任何类型。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scheme {
    pub vars: Vec<u32>,
    pub ty: Ty,
}

impl Scheme {
    pub fn show(&self) -> String {
        if self.vars.is_empty() {
            return self.ty.show();
        }
        let vs: Vec<String> = self.vars.iter().map(|&v| var_name(v)).collect();
        format!("∀{}. {}", vs.join(" "), self.ty.show())
    }

    /// 把变量按出现顺序重新编号成 α β γ……（显示最终结果用）。
    pub fn normalized(&self) -> Scheme {
        let mut order = Vec::new();
        self.ty.vars(&mut order);
        let map: BTreeMap<u32, u32> = order.iter().enumerate().map(|(i, &v)| (v, i as u32)).collect();
        let vars = order.iter().filter(|v| self.vars.contains(v)).map(|v| map[v]).collect();
        Scheme { vars, ty: self.ty.rename(&map) }
    }
}

#[derive(Clone, Debug)]
pub enum Event {
    /// 开始处理一个表达式，`rule` 是用到的规则。
    Visit { node: ExprId, rule: &'static str },
    /// 引入新的类型变量。
    Fresh { node: ExprId, var: u32, why: String },
    /// 名字的类型模式被实例化。
    Instantiate { node: ExprId, name: String, scheme: Scheme, ty: Ty },
    /// 要求两个类型相等（左右两边已代入当时的替换）。
    Unify { node: ExprId, left: Ty, right: Ty, why: String },
    /// 合一解出了一个新的替换：变量 = 类型。
    Bind { var: u32, ty: Ty },
    /// let 绑定的类型被泛化。
    Generalize { node: ExprId, name: String, ty: Ty, scheme: Scheme },
    /// 一个表达式的类型推完了（未代入替换的原始形式）。
    Typed { node: ExprId, ty: Ty },
    /// 推导失败。
    Fail(Diagnostic),
}

#[derive(Clone, Debug)]
pub struct Inference {
    pub events: Vec<Event>,
    /// 最终的替换。
    pub subst: BTreeMap<u32, Ty>,
    /// 每个表达式的类型（未代入替换的原始形式）。
    pub types: Vec<Option<Ty>>,
    /// 整个程序的类型（已泛化、按出现顺序重新命名），或错误。
    pub result: Result<Scheme, Diagnostic>,
}

impl Inference {
    /// 用前 k 个事件里的替换，把类型"解"到当时能解到的程度。
    pub fn resolve_at(&self, t: &Ty, k: usize) -> Ty {
        let mut s = BTreeMap::new();
        for e in &self.events[..k] {
            if let Event::Bind { var, ty } = e {
                s.insert(*var, ty.clone());
            }
        }
        apply(&s, t)
    }

    /// 前 k 个事件里得到的替换（按得到的顺序）。
    pub fn binds_at(&self, k: usize) -> Vec<(u32, Ty)> {
        self.events[..k]
            .iter()
            .filter_map(|e| match e {
                Event::Bind { var, ty } => Some((*var, ty.clone())),
                _ => None,
            })
            .collect()
    }
}

fn apply(s: &BTreeMap<u32, Ty>, t: &Ty) -> Ty {
    match t {
        Ty::Var(v) => match s.get(v) {
            Some(u) => apply(s, u),
            None => t.clone(),
        },
        Ty::Fun(a, b) => Ty::fun(apply(s, a), apply(s, b)),
        _ => t.clone(),
    }
}

pub fn infer(prog: &Program) -> Inference {
    let mut w = W { prog, subst: BTreeMap::new(), next: 0, events: Vec::new(), types: vec![None; prog.exprs.len()] };
    let mut env: Vec<(String, Scheme)> = Vec::new();
    let result = match w.infer(&mut env, prog.root) {
        Ok(t) => {
            let t = apply(&w.subst, &t);
            let mut vars = Vec::new();
            t.vars(&mut vars);
            Ok(Scheme { vars, ty: t }.normalized())
        }
        Err(d) => {
            w.events.push(Event::Fail(d.clone()));
            Err(d)
        }
    };
    Inference { events: w.events, subst: w.subst, types: w.types, result }
}

struct W<'p> {
    prog: &'p Program,
    subst: BTreeMap<u32, Ty>,
    next: u32,
    events: Vec<Event>,
    types: Vec<Option<Ty>>,
}

type R<T> = Result<T, Diagnostic>;

impl W<'_> {
    fn fresh(&mut self, node: ExprId, why: String) -> Ty {
        let v = self.next;
        self.next += 1;
        self.events.push(Event::Fresh { node, var: v, why });
        Ty::Var(v)
    }

    fn unify(&mut self, node: ExprId, a: &Ty, b: &Ty, why: String) -> R<()> {
        let (la, lb) = (apply(&self.subst, a), apply(&self.subst, b));
        self.events.push(Event::Unify { node, left: la.clone(), right: lb.clone(), why: why.clone() });
        self.unify_inner(a, b).map_err(|fail| {
            let span = self.prog.expr(node).span;
            match fail {
                Fail::Mismatch(x, y) => Diagnostic::error(format!("类型不匹配：{} 和 {} 合不到一起", x.show(), y.show()))
                    .with_code("E1304")
                    .with_primary(span, format!("这里要求 {} = {}", la.show(), lb.show()))
                    .with_note(why),
                Fail::Occurs(v, t) => Diagnostic::error(format!("无限类型：{} 不能等于包含它自己的 {}", var_name(v), t.show()))
                    .with_code("E1305")
                    .with_primary(span, format!("这里要求 {} = {}", la.show(), lb.show()))
                    .with_note(why)
                    .with_help("典型的例子是 `x x`：x 要同时是函数和这个函数自己的参数，这样的类型写不出来"),
            }
        })
    }

    fn unify_inner(&mut self, a: &Ty, b: &Ty) -> Result<(), Fail> {
        let (a, b) = (apply(&self.subst, a), apply(&self.subst, b));
        match (&a, &b) {
            (Ty::Var(x), Ty::Var(y)) if x == y => Ok(()),
            (Ty::Var(x), t) | (t, Ty::Var(x)) => {
                if t.occurs(*x) {
                    return Err(Fail::Occurs(*x, t.clone()));
                }
                self.subst.insert(*x, t.clone());
                self.events.push(Event::Bind { var: *x, ty: t.clone() });
                Ok(())
            }
            (Ty::Int, Ty::Int) | (Ty::Bool, Ty::Bool) => Ok(()),
            (Ty::Fun(a1, r1), Ty::Fun(a2, r2)) => {
                self.unify_inner(a1, a2)?;
                self.unify_inner(r1, r2)
            }
            _ => Err(Fail::Mismatch(a.clone(), b.clone())),
        }
    }

    /// 环境里自由出现的类型变量（泛化时不能动它们）。
    fn env_vars(&self, env: &[(String, Scheme)]) -> BTreeSet<u32> {
        let mut out = BTreeSet::new();
        for (_, s) in env {
            let mut vs = Vec::new();
            apply(&self.subst, &s.ty).vars(&mut vs);
            out.extend(vs.into_iter().filter(|v| !s.vars.contains(v)));
        }
        out
    }

    fn infer(&mut self, env: &mut Vec<(String, Scheme)>, e: ExprId) -> R<Ty> {
        let ty = match self.prog.expr(e).kind.clone() {
            ExprKind::Int(_) => {
                self.events.push(Event::Visit { node: e, rule: "整数的类型是 int" });
                Ty::Int
            }
            ExprKind::Bool(_) => {
                self.events.push(Event::Visit { node: e, rule: "true 和 false 的类型是 bool" });
                Ty::Bool
            }
            ExprKind::Var(x) => {
                self.events.push(Event::Visit { node: e, rule: "名字：到环境里查它的类型模式，把 ∀ 的变量换成新变量" });
                let Some((_, scheme)) = env.iter().rev().find(|(n, _)| *n == x).cloned() else {
                    return Err(Diagnostic::error(format!("找不到名字 `{x}`"))
                        .with_code("E1303")
                        .with_primary(self.prog.expr(e).span, "")
                        .with_help("名字要先用 `fun x ->` 或 `let x = … in` 引入"));
                };
                let mut map = BTreeMap::new();
                for &v in &scheme.vars {
                    let Ty::Var(nv) = self.fresh(e, format!("实例化 {x} 的类型模式")) else { unreachable!() };
                    map.insert(v, nv);
                }
                let ty = scheme.ty.rename(&map);
                self.events.push(Event::Instantiate { node: e, name: x, scheme, ty: ty.clone() });
                ty
            }
            ExprKind::Fun { param, body } => {
                self.events.push(Event::Visit { node: e, rule: "函数：参数类型先用新变量代替，推出函数体的类型，整体是 参数 → 函数体" });
                let a = self.fresh(e, format!("参数 {param} 的类型还不知道，先用一个新变量代替"));
                env.push((param, Scheme { vars: vec![], ty: a.clone() }));
                let tb = self.infer(env, body);
                env.pop();
                Ty::fun(a, tb?)
            }
            ExprKind::App { func, arg } => {
                self.events.push(Event::Visit { node: e, rule: "调用：被调用的必须是函数，它的参数类型等于实参类型" });
                let tf = self.infer(env, func)?;
                let ta = self.infer(env, arg)?;
                let r = self.fresh(e, "调用结果的类型还不知道，先用一个新变量代替".to_owned());
                self.unify(
                    e,
                    &tf,
                    &Ty::fun(ta, r.clone()),
                    format!("`{}` 被当作函数调用，它的类型必须是 (实参类型 → 结果类型)", self.prog.text(func)),
                )?;
                r
            }
            ExprKind::Let { name, value, body } => {
                self.events.push(Event::Visit { node: e, rule: "let：先推出值的类型并泛化，再在它的作用域里推导后面的表达式" });
                let tv = self.infer(env, value)?;
                let tv = apply(&self.subst, &tv);
                let fixed = self.env_vars(env);
                let mut vs = Vec::new();
                tv.vars(&mut vs);
                vs.retain(|v| !fixed.contains(v));
                let scheme = Scheme { vars: vs, ty: tv.clone() };
                self.events.push(Event::Generalize { node: e, name: name.clone(), ty: tv, scheme: scheme.clone() });
                env.push((name, scheme));
                let tb = self.infer(env, body);
                env.pop();
                tb?
            }
            ExprKind::If { cond, then, els } => {
                self.events.push(Event::Visit { node: e, rule: "if：条件必须是 bool，两个分支的类型必须相同" });
                let tc = self.infer(env, cond)?;
                self.unify(cond, &tc, &Ty::Bool, "if 的条件必须是 bool".to_owned())?;
                let tt = self.infer(env, then)?;
                let te = self.infer(env, els)?;
                self.unify(e, &tt, &te, "if 的两个分支必须是同一种类型".to_owned())?;
                tt
            }
            ExprKind::Bin { op, lhs, rhs } => {
                let sym = op.symbol();
                self.events.push(Event::Visit {
                    node: e,
                    rule: match op {
                        BinOp::Add | BinOp::Sub | BinOp::Mul => "算术运算：两边都是 int，结果是 int",
                        BinOp::Lt => "比较大小：两边都是 int，结果是 bool",
                        BinOp::Eq => "相等比较：两边类型相同，结果是 bool",
                    },
                });
                let tl = self.infer(env, lhs)?;
                let tr = self.infer(env, rhs)?;
                match op {
                    BinOp::Eq => {
                        self.unify(e, &tl, &tr, "== 两边的类型必须相同".to_owned())?;
                        Ty::Bool
                    }
                    _ => {
                        self.unify(lhs, &tl, &Ty::Int, format!("`{sym}` 的左边必须是 int"))?;
                        self.unify(rhs, &tr, &Ty::Int, format!("`{sym}` 的右边必须是 int"))?;
                        if op == BinOp::Lt { Ty::Bool } else { Ty::Int }
                    }
                }
            }
        };
        self.types[e.index()] = Some(ty.clone());
        self.events.push(Event::Typed { node: e, ty: ty.clone() });
        Ok(ty)
    }
}

enum Fail {
    Mismatch(Ty, Ty),
    Occurs(u32, Ty),
}

/// 推导结果的一行文字（测试和命令行用）。
pub fn summary(prog: &Program) -> String {
    let inf = infer(prog);
    let mut s = String::new();
    match inf.result {
        Ok(sc) => write!(s, "{}", sc.show()).unwrap(),
        Err(d) => write!(s, "错误 {}", d.code.unwrap_or("")).unwrap(),
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::parse;

    fn ty(src: &str) -> String {
        summary(&parse(src).unwrap())
    }

    #[test]
    fn classic_examples() {
        assert_eq!(ty("fun x -> x"), "∀α. α → α");
        assert_eq!(ty("fun x -> x + 1"), "int → int");
        assert_eq!(ty("fun f -> fun x -> f (f x)"), "∀α. (α → α) → α → α");
        assert_eq!(ty("fun f -> fun g -> fun x -> f (g x)"), "∀α β γ. (α → β) → (γ → α) → γ → β");
        assert_eq!(ty("fun x -> fun y -> x"), "∀α β. α → β → α");
        assert_eq!(ty("fun x -> fun y -> x == y"), "∀α. α → α → bool");
    }

    #[test]
    fn let_polymorphism() {
        // let 绑定的 id 被泛化，可以同时用在 bool 和 int 上
        assert_eq!(ty("let id = fun x -> x in if id true then id 1 else 2"), "int");
        // 函数参数不会被泛化（单态），同样的写法就报错
        assert_eq!(ty("(fun id -> if id true then id 1 else 2) (fun x -> x)"), "错误 E1304");
    }

    #[test]
    fn errors() {
        assert_eq!(ty("fun x -> x x"), "错误 E1305");
        assert_eq!(ty("1 + true"), "错误 E1304");
        assert_eq!(ty("if 1 then 2 else 3"), "错误 E1304");
        assert_eq!(ty("y + 1"), "错误 E1303");
    }
}
