#![forbid(unsafe_code)]

use ast::{
    BinaryExpr, BlockExpr, CallExpr, Decl, Expr, FunctionDecl, IfExpr, ImplMember, MatchArm,
    MatchExpr, Pattern, Program, Span, Stmt, StructLiteralExpr, TypeExpr,
};
use std::collections::HashMap;

/// A named entity known by the compiler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub span: Span,
}

/// The kinds of declarations known by semantic analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Variable,
    Constant,
    Function,
    Parameter,
    Struct,
    Enum,
    Trait,
    Module,
}

/// A lexical scope containing symbols and an optional parent scope.
#[derive(Debug, Clone)]
pub struct Scope {
    symbols: HashMap<String, Symbol>,
    parent: Option<usize>,
}

/// Symbol table containing all scopes in the analyzed program.
#[derive(Debug, Default)]
pub struct SymbolTable {
    scopes: Vec<Scope>,
    current_scope: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

/// A semantic diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub span: Span,
}

/// Result of semantic analysis.
#[derive(Debug, Default)]
pub struct SemanticResult {
    pub diagnostics: Vec<Diagnostic>,
}

impl SemanticResult {
    pub fn is_ok(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }

    pub fn has_errors(&self) -> bool {
        !self.is_ok()
    }
}

impl Symbol {
    pub fn new(name: impl Into<String>, kind: SymbolKind, span: Span) -> Self {
        Self {
            name: name.into(),
            kind,
            span,
        }
    }
}

impl Scope {
    fn new(parent: Option<usize>) -> Self {
        Self {
            symbols: HashMap::new(),
            parent,
        }
    }
}

impl SymbolTable {
    /// Creates a symbol table with a global scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope::new(None)],
            current_scope: 0,
        }
    }

    pub fn current_scope(&self) -> usize {
        self.current_scope
    }

    /// Creates and enters a child scope.
    pub fn enter_scope(&mut self) -> usize {
        let parent = Some(self.current_scope);
        let scope_id = self.scopes.len();

        self.scopes.push(Scope::new(parent));
        self.current_scope = scope_id;

        scope_id
    }

    /// Leaves the current scope.
    ///
    /// Returns false if already in the global scope.
    pub fn exit_scope(&mut self) -> bool {
        let parent = match self.scopes[self.current_scope].parent {
            Some(parent) => parent,
            None => return false,
        };

        self.current_scope = parent;
        true
    }

    /// Defines a symbol in the current scope.
    pub fn define(&mut self, symbol: Symbol) -> Result<(), Symbol> {
        let scope = &mut self.scopes[self.current_scope];

        if scope.symbols.contains_key(&symbol.name) {
            return Err(symbol);
        }

        scope.symbols.insert(symbol.name.clone(), symbol);
        Ok(())
    }

    /// Resolves a symbol through the current scope and all parents.
    pub fn resolve(&self, name: &str) -> Option<&Symbol> {
        let mut scope_id = Some(self.current_scope);

        while let Some(id) = scope_id {
            let scope = &self.scopes[id];

            if let Some(symbol) = scope.symbols.get(name) {
                return Some(symbol);
            }

            scope_id = scope.parent;
        }

        None
    }

    /// Resolves a symbol only in the current scope.
    pub fn resolve_current(&self, name: &str) -> Option<&Symbol> {
        self.scopes[self.current_scope].symbols.get(name)
    }

    pub fn scope_count(&self) -> usize {
        self.scopes.len()
    }
}

pub fn semantic_error(message: impl Into<String>, span: Span) -> Diagnostic {
    Diagnostic {
        severity: DiagnosticSeverity::Error,
        message: message.into(),
        span,
    }
}

pub fn semantic_warning(message: impl Into<String>, span: Span) -> Diagnostic {
    Diagnostic {
        severity: DiagnosticSeverity::Warning,
        message: message.into(),
        span,
    }
}

/// Performs semantic analysis over an AST program.
#[derive(Debug, Default)]
pub struct SemanticAnalyzer {
    symbols: SymbolTable,
    diagnostics: Vec<Diagnostic>,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            symbols: SymbolTable::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Analyze an entire program.
    pub fn analyze(mut self, program: &Program) -> SemanticResult {
        self.collect_top_level_symbols(program);

        for declaration in &program.declarations {
            self.analyze_decl(declaration);
        }

        SemanticResult {
            diagnostics: self.diagnostics,
        }
    }

    pub fn symbol_table(&self) -> &SymbolTable {
        &self.symbols
    }

    fn collect_top_level_symbols(&mut self, program: &Program) {
        for declaration in &program.declarations {
            let (name, kind, span) = match declaration {
                Decl::Function(function) => {
                    (function.name.clone(), SymbolKind::Function, function.span)
                }
                Decl::Struct(struct_decl) => (
                    struct_decl.name.clone(),
                    SymbolKind::Struct,
                    struct_decl.span,
                ),
                Decl::Enum(enum_decl) => (enum_decl.name.clone(), SymbolKind::Enum, enum_decl.span),
                Decl::Trait(trait_decl) => {
                    (trait_decl.name.clone(), SymbolKind::Trait, trait_decl.span)
                }
                Decl::Const(const_decl) => (
                    const_decl.name.clone(),
                    SymbolKind::Constant,
                    const_decl.span,
                ),
                Decl::Import(import) => {
                    let Some(name) = import.path.last() else {
                        continue;
                    };

                    (name.clone(), SymbolKind::Module, import.span)
                }
                Decl::Impl(_) => continue,
            };

            self.define_symbol(Symbol::new(name, kind, span));
        }
    }

    fn analyze_decl(&mut self, declaration: &Decl) {
        match declaration {
            Decl::Function(function) => self.analyze_function(function),
            Decl::Struct(struct_decl) => {
                for field in &struct_decl.fields {
                    self.analyze_type(&field.ty);
                }
            }
            Decl::Enum(enum_decl) => {
                for variant in &enum_decl.variants {
                    for field_type in &variant.fields {
                        self.analyze_type(field_type);
                    }
                }
            }
            Decl::Trait(trait_decl) => {
                for member in &trait_decl.members {
                    match member {
                        ast::TraitMember::Function(signature) => {
                            for parameter in &signature.parameters {
                                self.analyze_type(&parameter.ty);
                            }

                            if let Some(return_type) = &signature.return_type {
                                self.analyze_type(return_type);
                            }
                        }
                    }
                }
            }
            Decl::Impl(impl_decl) => {
                self.analyze_type(&impl_decl.target_type);

                if let Some(trait_type) = &impl_decl.trait_type {
                    self.analyze_type(trait_type);
                }

                for member in &impl_decl.members {
                    match member {
                        ImplMember::Function(function) => self.analyze_function(function),
                    }
                }
            }
            Decl::Const(const_decl) => {
                self.analyze_type(&const_decl.ty);
                self.analyze_expr(&const_decl.value);
            }
            Decl::Import(_) => {}
        }
    }

    fn analyze_function(&mut self, function: &FunctionDecl) {
        self.symbols.enter_scope();

        for parameter in &function.parameters {
            self.analyze_type(&parameter.ty);

            self.define_symbol_with_error(
                Symbol::new(
                    parameter.name.clone(),
                    SymbolKind::Parameter,
                    parameter.span,
                ),
                format!("duplicate parameter `{}`", parameter.name),
            );
        }

        if let Some(return_type) = &function.return_type {
            self.analyze_type(return_type);
        }

        self.analyze_block(&function.body);

        let _ = self.symbols.exit_scope();
    }

    fn analyze_block(&mut self, block: &BlockExpr) {
        self.symbols.enter_scope();

        for statement in &block.statements {
            self.analyze_stmt(statement);
        }

        if let Some(expression) = &block.trailing_expr {
            self.analyze_expr(expression);
        }

        let _ = self.symbols.exit_scope();
    }

    fn analyze_stmt(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Let(let_stmt) => {
                if let Some(value) = &let_stmt.value {
                    self.analyze_expr(value);
                }

                if let Some(ty) = &let_stmt.ty {
                    self.analyze_type(ty);
                }

                self.define_pattern(&let_stmt.pattern, SymbolKind::Variable);
            }

            Stmt::Var(var_stmt) => {
                if let Some(value) = &var_stmt.value {
                    self.analyze_expr(value);
                }

                if let Some(ty) = &var_stmt.ty {
                    self.analyze_type(ty);
                }

                self.define_pattern(&var_stmt.pattern, SymbolKind::Variable);
            }

            Stmt::Expr(expr_stmt) => self.analyze_expr(&expr_stmt.expr),

            Stmt::Return(return_stmt) => {
                if let Some(value) = &return_stmt.value {
                    self.analyze_expr(value);
                }
            }

            Stmt::While(while_stmt) => {
                self.analyze_expr(&while_stmt.condition);
                self.analyze_block(&while_stmt.body);
            }

            Stmt::For(for_stmt) => {
                self.analyze_expr(&for_stmt.iterable);

                self.symbols.enter_scope();
                self.define_pattern(&for_stmt.pattern, SymbolKind::Variable);
                self.analyze_block(&for_stmt.body);
                let _ = self.symbols.exit_scope();
            }

            Stmt::ParallelFor(for_stmt) => {
                self.analyze_expr(&for_stmt.iterable);

                self.symbols.enter_scope();
                self.define_pattern(&for_stmt.pattern, SymbolKind::Variable);
                self.analyze_block(&for_stmt.body);
                let _ = self.symbols.exit_scope();
            }

            Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    fn analyze_expr(&mut self, expression: &Expr) {
        match expression {
            Expr::Literal(_) => {}

            Expr::Identifier(identifier) => {
                if self.symbols.resolve(&identifier.name).is_none() {
                    self.diagnostics.push(semantic_error(
                        format!("unknown identifier `{}`", identifier.name),
                        identifier.span,
                    ));
                }
            }

            Expr::Binary(binary) => self.analyze_binary(binary),

            Expr::Assignment(assignment) => {
                self.analyze_expr(&assignment.target);
                self.analyze_expr(&assignment.value);
            }

            Expr::Unary(unary) => self.analyze_expr(&unary.operand),

            Expr::Call(call) => self.analyze_call(call),

            Expr::Member(member) => {
                self.analyze_expr(&member.object);
            }

            Expr::Index(index) => {
                self.analyze_expr(&index.object);

                for expression in &index.indices {
                    self.analyze_expr(expression);
                }
            }

            Expr::Slice(slice) => {
                self.analyze_expr(&slice.object);

                if let Some(start) = &slice.start {
                    self.analyze_expr(start);
                }

                if let Some(end) = &slice.end {
                    self.analyze_expr(end);
                }

                if let Some(step) = &slice.step {
                    self.analyze_expr(step);
                }
            }

            Expr::Array(array) => {
                for element in &array.elements {
                    self.analyze_expr(element);
                }
            }

            Expr::Tuple(tuple) => {
                for element in &tuple.elements {
                    self.analyze_expr(element);
                }
            }

            Expr::Block(block) => self.analyze_block(block),

            Expr::If(if_expr) => self.analyze_if(if_expr),

            Expr::Match(match_expr) => self.analyze_match(match_expr),

            Expr::Range(range) => {
                if let Some(start) = &range.start {
                    self.analyze_expr(start);
                }

                if let Some(end) = &range.end {
                    self.analyze_expr(end);
                }
            }

            Expr::Formula(formula) => {
                self.analyze_expr(&formula.response);

                for predictor in &formula.predictors {
                    self.analyze_expr(predictor);
                }
            }

            Expr::StructLiteral(struct_literal) => self.analyze_struct_literal(struct_literal),

            Expr::Cast(cast) => {
                self.analyze_expr(&cast.expr);
                self.analyze_type(&cast.target_type);
            }

            Expr::Await(await_expr) => self.analyze_expr(&await_expr.expr),
        }
    }

    fn analyze_binary(&mut self, binary: &BinaryExpr) {
        self.analyze_expr(&binary.left);
        self.analyze_expr(&binary.right);
    }

    fn analyze_call(&mut self, call: &CallExpr) {
        self.analyze_expr(&call.callee);

        for generic_arg in &call.generic_args {
            self.analyze_type(generic_arg);
        }

        for argument in &call.arguments {
            self.analyze_expr(argument);
        }
    }

    fn analyze_if(&mut self, if_expr: &IfExpr) {
        self.analyze_expr(&if_expr.condition);
        self.analyze_block(&if_expr.then_branch);

        if let Some(else_branch) = &if_expr.else_branch {
            self.analyze_expr(else_branch);
        }
    }

    fn analyze_match(&mut self, match_expr: &MatchExpr) {
        self.analyze_expr(&match_expr.scrutinee);

        for arm in &match_expr.arms {
            self.analyze_match_arm(arm);
        }
    }

    fn analyze_match_arm(&mut self, arm: &MatchArm) {
        self.symbols.enter_scope();

        self.define_pattern(&arm.pattern, SymbolKind::Variable);

        if let Some(guard) = &arm.guard {
            self.analyze_expr(guard);
        }

        self.analyze_expr(&arm.body);

        let _ = self.symbols.exit_scope();
    }

    fn analyze_struct_literal(&mut self, literal: &StructLiteralExpr) {
        if self.symbols.resolve(&literal.name).is_none() {
            self.diagnostics.push(semantic_error(
                format!("unknown struct `{}`", literal.name),
                literal.span,
            ));
        }

        for field in &literal.fields {
            self.analyze_expr(&field.value);
        }
    }

    fn analyze_type(&mut self, ty: &TypeExpr) {
        match ty {
            TypeExpr::Named { .. } => {}

            TypeExpr::Generic { arguments, .. } => {
                for argument in arguments {
                    self.analyze_type(argument);
                }
            }

            TypeExpr::Reference { inner, .. } => self.analyze_type(inner),

            TypeExpr::Tuple { elements, .. } => {
                for element in elements {
                    self.analyze_type(element);
                }
            }

            TypeExpr::Array { element, size, .. } => {
                self.analyze_type(element);

                if let Some(size) = size {
                    self.analyze_expr(size);
                }
            }

            TypeExpr::Function {
                parameters,
                return_type,
                ..
            } => {
                for parameter in parameters {
                    self.analyze_type(parameter);
                }

                self.analyze_type(return_type);
            }
        }
    }

    fn define_pattern(&mut self, pattern: &Pattern, kind: SymbolKind) {
        match pattern {
            Pattern::Identifier { name, span } => {
                self.define_symbol_with_error(
                    Symbol::new(name.clone(), kind, *span),
                    format!("duplicate declaration `{}`", name),
                );
            }

            Pattern::Wildcard { .. } | Pattern::Literal { .. } => {}

            Pattern::Tuple { elements, .. } => {
                for element in elements {
                    self.define_pattern(element, kind);
                }
            }

            Pattern::EnumVariant { fields, .. } => {
                for field in fields {
                    self.define_pattern(field, kind);
                }
            }
        }
    }

    fn define_symbol_with_error(&mut self, symbol: Symbol, message: String) {
        if self.symbols.define(symbol.clone()).is_err() {
            self.diagnostics.push(semantic_error(message, symbol.span));
        }
    }

    fn define_symbol(&mut self, symbol: Symbol) {
        if self.symbols.define(symbol.clone()).is_err() {
            self.diagnostics.push(semantic_error(
                format!("duplicate declaration `{}`", symbol.name),
                symbol.span,
            ));
        }
    }
}

/// Convenience function for semantic analysis.
pub fn analyze(program: &Program) -> SemanticResult {
    SemanticAnalyzer::new().analyze(program)
}

#[cfg(test)]
mod tests {
    use super::*;
    use parser::Parser;

    fn analyze_source(source: &str) -> SemanticResult {
        let mut parser = Parser::from_source(source).expect("source should lex");
        let program = parser.parse_program().expect("source should parse");
        analyze(&program)
    }

    #[test]
    fn valid_variable_resolution() {
        let result = analyze_source(
            r#"
            fn main() {
                let x = 10;
                let y = x + 20;
                return y;
            }
            "#,
        );

        assert!(result.is_ok(), "{:?}", result.diagnostics);
    }

    #[test]
    fn detects_unknown_identifier() {
        let result = analyze_source(
            r#"
            fn main() {
                let y = unknown_variable + 20;
                return y;
            }
            "#,
        );

        assert!(result.has_errors());
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unknown identifier `unknown_variable`")
        }));
    }

    #[test]
    fn detects_duplicate_local_declaration() {
        let result = analyze_source(
            r#"
            fn main() {
                let x = 10;
                let x = 20;
            }
            "#,
        );

        assert!(result.has_errors());
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("duplicate declaration `x`"))
        );
    }

    #[test]
    fn nested_scope_can_see_parent() {
        let result = analyze_source(
            r#"
            fn main() {
                let x = 10;

                if x > 0 {
                    let y = x + 1;
                    return y;
                }
            }
            "#,
        );

        assert!(result.is_ok(), "{:?}", result.diagnostics);
    }

    #[test]
    fn nested_scope_hides_outer_scope() {
        let result = analyze_source(
            r#"
            fn main() {
                let x = 10;

                if x > 0 {
                    let x = 20;
                    let y = x + 1;
                    return y;
                }
            }
            "#,
        );

        assert!(result.is_ok(), "{:?}", result.diagnostics);
    }

    #[test]
    fn function_can_call_another_function() {
        let result = analyze_source(
            r#"
            fn add(a: Int64, b: Int64) -> Int64 {
                return a + b;
            }

            fn main() {
                let result = add(10, 20);
                return result;
            }
            "#,
        );

        assert!(result.is_ok(), "{:?}", result.diagnostics);
    }

    #[test]
    fn function_can_call_itself() {
        let result = analyze_source(
            r#"
            fn factorial(n: Int64) -> Int64 {
                if n > 0 {
                    return n * factorial(n - 1);
                }

                return 1;
            }
            "#,
        );

        assert!(result.is_ok(), "{:?}", result.diagnostics);
    }

    #[test]
    fn detects_unknown_function() {
        let result = analyze_source(
            r#"
            fn main() {
                let result = does_not_exist(10);
                return result;
            }
            "#,
        );

        assert!(result.has_errors());
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unknown identifier `does_not_exist`")
        }));
    }

    #[test]
    fn for_loop_variable_is_scoped() {
        let result = analyze_source(
            r#"
            fn main() {
                for i in 0..10 {
                    let x = i;
                    return x;
                }
            }
            "#,
        );

        assert!(result.is_ok(), "{:?}", result.diagnostics);
    }

    #[test]
    fn detects_unknown_identifier_after_scope_ends() {
        let result = analyze_source(
            r#"
            fn main() {
                if true {
                    let inner = 10;
                }

                let result = inner;
                return result;
            }
            "#,
        );

        assert!(result.has_errors());
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("unknown identifier `inner`"))
        );
    }
}
