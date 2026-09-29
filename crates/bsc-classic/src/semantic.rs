use crate::ast::*;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub ty: Type,
    pub is_const: bool,
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub symbols: HashMap<String, Symbol>,
}

impl Scope {
    pub fn new() -> Self {
        Self { symbols: HashMap::new() }
    }
}

pub struct TypeChecker {
    scopes: Vec<Scope>,
    functions: HashMap<String, (Vec<Type>, Type)>, // name -> (param_types, ret_type)
    current_fn_ret_type: Option<Type>,
    pub diagnostics: Vec<String>,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut checker = Self {
            scopes: vec![Scope::new()],
            functions: HashMap::new(),
            current_fn_ret_type: None,
            diagnostics: Vec::new(),
        };
        // Built-in print
        checker.functions.insert("print".to_string(), (vec![Type::Int], Type::Void));
        checker
    }

    pub fn check_program(&mut self, prog: &Program) -> Result<(), String> {
        // First pass: register function prototypes
        for func in &prog.functions {
            let param_types: Vec<Type> = func.params.iter().map(|(_, ty)| ty.clone()).collect();
            if self.functions.contains_key(&func.name) {
                return Err(format!("Duplicate function '{}' defined at {}:{}", func.name, func.span.line, func.span.col));
            }
            self.functions.insert(func.name.clone(), (param_types, func.ret_type.clone()));
        }

        // Second pass: check function bodies
        for func in &prog.functions {
            self.check_function(func)?;
        }

        if !self.diagnostics.is_empty() {
            Err(self.diagnostics.join("\n"))
        } else {
            Ok(())
        }
    }

    fn check_function(&mut self, func: &Function) -> Result<(), String> {
        self.current_fn_ret_type = Some(func.ret_type.clone());
        self.enter_scope();

        for (p_name, p_type) in &func.params {
            self.define_symbol(p_name.clone(), p_type.clone(), false);
        }

        for stmt in &func.body {
            self.check_stmt(stmt)?;
        }

        self.exit_scope();
        self.current_fn_ret_type = None;
        Ok(())
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<(), String> {
        match stmt {
            Stmt::Let(name, ty, init, span) => {
                let init_ty = self.check_expr(init)?;
                if init_ty != *ty {
                    self.diagnostics.push(format!("Type mismatch in variable '{}' at {}:{}: expected {:?}, found {:?}", name, span.line, span.col, ty, init_ty));
                }
                self.define_symbol(name.clone(), ty.clone(), false);
            }
            Stmt::Assign(name, expr, span) => {
                let sym_ty = self.lookup_symbol(name).map(|s| s.ty.clone()).ok_or_else(|| {
                    format!("Undeclared variable '{}' assigned at {}:{}", name, span.line, span.col)
                })?;
                let expr_ty = self.check_expr(expr)?;
                if sym_ty != expr_ty {
                    self.diagnostics.push(format!("Type mismatch in assignment to '{}' at {}:{}: variable is {:?}, value is {:?}", name, span.line, span.col, sym_ty, expr_ty));
                }
            }
            Stmt::If(cond, then_block, else_block, span) => {
                let cond_ty = self.check_expr(cond)?;
                if cond_ty != Type::Bool {
                    self.diagnostics.push(format!("Condition of 'if' at {}:{} must be bool, got {:?}", span.line, span.col, cond_ty));
                }
                self.enter_scope();
                for s in then_block { self.check_stmt(s)?; }
                self.exit_scope();

                if let Some(el) = else_block {
                    self.enter_scope();
                    for s in el { self.check_stmt(s)?; }
                    self.exit_scope();
                }
            }
            Stmt::While(cond, body, span) => {
                let cond_ty = self.check_expr(cond)?;
                if cond_ty != Type::Bool {
                    self.diagnostics.push(format!("Condition of 'while' at {}:{} must be bool, got {:?}", span.line, span.col, cond_ty));
                }
                self.enter_scope();
                for s in body { self.check_stmt(s)?; }
                self.exit_scope();
            }
            Stmt::Return(maybe_expr, span) => {
                let expected = self.current_fn_ret_type.clone().unwrap();
                match (maybe_expr, &expected) {
                    (Some(_), Type::Void) => {
                        self.diagnostics.push(format!("Function returns void, but expression returned at {}:{}", span.line, span.col));
                    }
                    (Some(expr), ty) => {
                        let actual = self.check_expr(expr)?;
                        if actual != *ty {
                            self.diagnostics.push(format!("Return type mismatch at {}:{}: expected {:?}, got {:?}", span.line, span.col, ty, actual));
                        }
                    }
                    (None, Type::Void) => {}
                    (None, ty) => {
                        self.diagnostics.push(format!("Missing return value at {}:{}: expected {:?}", span.line, span.col, ty));
                    }
                }
            }
            Stmt::Print(expr, _) => {
                self.check_expr(expr)?;
            }
            Stmt::Expr(expr, _) => {
                self.check_expr(expr)?;
            }
        }
        Ok(())
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<Type, String> {
        match expr {
            Expr::Literal(_, _) => Ok(Type::Int),
            Expr::BoolLit(_, _) => Ok(Type::Bool),
            Expr::Variable(name, span) => {
                if let Some(sym) = self.lookup_symbol(name) {
                    Ok(sym.ty.clone())
                } else {
                    self.diagnostics.push(format!("Undeclared variable '{}' at {}:{}", name, span.line, span.col));
                    Ok(Type::Int) // error recovery
                }
            }
            Expr::Binary(op, left, right, span) => {
                let l_ty = self.check_expr(left)?;
                let r_ty = self.check_expr(right)?;

                match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                        if l_ty != Type::Int || r_ty != Type::Int {
                            self.diagnostics.push(format!("Arithmetic operands must be int at {}:{}", span.line, span.col));
                        }
                        Ok(Type::Int)
                    }
                    BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                        if l_ty != Type::Int || r_ty != Type::Int {
                            self.diagnostics.push(format!("Comparison operands must be int at {}:{}", span.line, span.col));
                        }
                        Ok(Type::Bool)
                    }
                    BinOp::Eq | BinOp::Ne => {
                        if l_ty != r_ty {
                            self.diagnostics.push(format!("Equality check operands must have identical types at {}:{}", span.line, span.col));
                        }
                        Ok(Type::Bool)
                    }
                    BinOp::And | BinOp::Or => {
                        if l_ty != Type::Bool || r_ty != Type::Bool {
                            self.diagnostics.push(format!("Logical operands must be bool at {}:{}", span.line, span.col));
                        }
                        Ok(Type::Bool)
                    }
                }
            }
            Expr::Unary(op, inner, span) => {
                let in_ty = self.check_expr(inner)?;
                match op {
                    UnOp::Neg => {
                        if in_ty != Type::Int {
                            self.diagnostics.push(format!("Negation '-' requires int at {}:{}", span.line, span.col));
                        }
                        Ok(Type::Int)
                    }
                    UnOp::Not => {
                        if in_ty != Type::Bool {
                            self.diagnostics.push(format!("Logical not '!' requires bool at {}:{}", span.line, span.col));
                        }
                        Ok(Type::Bool)
                    }
                }
            }
            Expr::Call(name, args, span) => {
                if let Some((param_tys, ret_ty)) = self.functions.get(name).cloned() {
                    if args.len() != param_tys.len() {
                        self.diagnostics.push(format!("Function '{}' expects {} arguments, got {} at {}:{}", name, param_tys.len(), args.len(), span.line, span.col));
                    }
                    for (i, (arg, expected_ty)) in args.iter().zip(param_tys.iter()).enumerate() {
                        let arg_ty = self.check_expr(arg)?;
                        if arg_ty != *expected_ty {
                            self.diagnostics.push(format!("Argument {} of '{}' type mismatch at {}:{}: expected {:?}, got {:?}", i+1, name, span.line, span.col, expected_ty, arg_ty));
                        }
                    }
                    Ok(ret_ty)
                } else {
                    self.diagnostics.push(format!("Call to undeclared function '{}' at {}:{}", name, span.line, span.col));
                    Ok(Type::Int)
                }
            }
        }
    }

    fn enter_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    fn define_symbol(&mut self, name: String, ty: Type, is_const: bool) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.symbols.insert(name.clone(), Symbol { name, ty, is_const });
        }
    }

    fn lookup_symbol(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(sym) = scope.symbols.get(name) {
                return Some(sym);
            }
        }
        None
    }

    pub fn dump_symbol_table(&self) -> String {
        let mut out = String::from("; Symbol Table & Function Signatures:\n");
        for (fn_name, (params, ret)) in &self.functions {
            let p_str = params.iter().map(|p| format!("{:?}", p)).collect::<Vec<_>>().join(", ");
            out.push_str(&format!("  fn {}({}) -> {:?}\n", fn_name, p_str, ret));
        }
        out
    }
}
