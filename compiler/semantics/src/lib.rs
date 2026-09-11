use ast::Span;
use std::collections::HashMap;

/// A symbol is a named entity known by the compiler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub span: Span,
}

/// The kinds of declarations that can exist in a scope.
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

/// A lexical scope containing symbols and optionally a parent scope.
#[derive(Debug, Clone)]
pub struct Scope {
    symbols: HashMap<String, Symbol>,
    parent: Option<usize>,
}

/// Symbol table containing all scopes of a program.
#[derive(Debug, Default)]
pub struct SymbolTable {
    scopes: Vec<Scope>,
    current_scope: usize,
}

/// Semantic diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

/// A semantic analysis diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub span: Span,
}

/// The result of semantic analysis.
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
    /// Creates a symbol table with one global scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope::new(None)],
            current_scope: 0,
        }
    }

    /// Returns the current scope id.
    pub fn current_scope(&self) -> usize {
        self.current_scope
    }

    /// Creates a child scope and enters it.
    pub fn enter_scope(&mut self) -> usize {
        let parent = Some(self.current_scope);
        let scope_id = self.scopes.len();

        self.scopes.push(Scope::new(parent));
        self.current_scope = scope_id;

        scope_id
    }

    /// Leaves the current scope and returns to its parent.
    ///
    /// The global scope cannot be left.
    pub fn exit_scope(&mut self) -> bool {
        let parent = match self.scopes[self.current_scope].parent {
            Some(parent) => parent,
            None => return false,
        };

        self.current_scope = parent;
        true
    }

    /// Defines a symbol in the current scope.
    ///
    /// Returns an error if the symbol already exists in the current scope.
    pub fn define(&mut self, symbol: Symbol) -> Result<(), Symbol> {
        let scope = &mut self.scopes[self.current_scope];

        if scope.symbols.contains_key(&symbol.name) {
            return Err(symbol);
        }

        scope.symbols.insert(symbol.name.clone(), symbol);
        Ok(())
    }

    /// Resolves a symbol starting from the current scope and walking
    /// through parent scopes.
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

    /// Looks up a symbol only in the current scope.
    pub fn resolve_current(&self, name: &str) -> Option<&Symbol> {
        self.scopes[self.current_scope].symbols.get(name)
    }

    /// Returns the number of scopes currently allocated.
    pub fn scope_count(&self) -> usize {
        self.scopes.len()
    }
}

/// Helper for constructing semantic errors.
pub fn semantic_error(message: impl Into<String>, span: Span) -> Diagnostic {
    Diagnostic {
        severity: DiagnosticSeverity::Error,
        message: message.into(),
        span,
    }
}

/// Helper for constructing semantic warnings.
pub fn semantic_warning(message: impl Into<String>, span: Span) -> Diagnostic {
    Diagnostic {
        severity: DiagnosticSeverity::Warning,
        message: message.into(),
        span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> Span {
        Span::new(0, 1)
    }

    #[test]
    fn creates_global_scope() {
        let table = SymbolTable::new();

        assert_eq!(table.current_scope(), 0);
        assert_eq!(table.scope_count(), 1);
    }

    #[test]
    fn enters_and_exits_nested_scope() {
        let mut table = SymbolTable::new();

        let child = table.enter_scope();

        assert_eq!(table.current_scope(), child);
        assert_eq!(table.scope_count(), 2);

        assert!(table.exit_scope());
        assert_eq!(table.current_scope(), 0);
    }

    #[test]
    fn global_scope_cannot_be_exited() {
        let mut table = SymbolTable::new();

        assert!(!table.exit_scope());
        assert_eq!(table.current_scope(), 0);
    }

    #[test]
    fn defines_symbol() {
        let mut table = SymbolTable::new();

        let symbol = Symbol::new("x", SymbolKind::Variable, span());

        assert!(table.define(symbol).is_ok());
        assert!(table.resolve("x").is_some());
    }

    #[test]
    fn rejects_duplicate_symbol_in_same_scope() {
        let mut table = SymbolTable::new();

        let first = Symbol::new("x", SymbolKind::Variable, span());
        let second = Symbol::new("x", SymbolKind::Variable, span());

        assert!(table.define(first).is_ok());
        assert!(table.define(second).is_err());
    }

    #[test]
    fn resolves_parent_scope() {
        let mut table = SymbolTable::new();

        let symbol = Symbol::new("x", SymbolKind::Variable, span());

        table.define(symbol).unwrap();
        table.enter_scope();

        let resolved = table.resolve("x");

        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().name, "x");
    }

    #[test]
    fn child_scope_can_shadow_parent() {
        let mut table = SymbolTable::new();

        let outer = Symbol::new("x", SymbolKind::Variable, span());
        table.define(outer).unwrap();

        table.enter_scope();

        let inner = Symbol::new("x", SymbolKind::Variable, span());
        table.define(inner).unwrap();

        let resolved = table.resolve("x").unwrap();

        assert_eq!(resolved.span, span());
    }

    #[test]
    fn resolve_current_does_not_search_parent() {
        let mut table = SymbolTable::new();

        let symbol = Symbol::new("x", SymbolKind::Variable, span());
        table.define(symbol).unwrap();

        table.enter_scope();

        assert!(table.resolve_current("x").is_none());
        assert!(table.resolve("x").is_some());
    }

    #[test]
    fn semantic_result_is_ok_without_errors() {
        let result = SemanticResult::default();

        assert!(result.is_ok());
        assert!(!result.has_errors());
    }

    #[test]
    fn semantic_result_detects_error() {
        let result = SemanticResult {
            diagnostics: vec![semantic_error("unknown variable", span())],
        };

        assert!(!result.is_ok());
        assert!(result.has_errors());
    }
}
