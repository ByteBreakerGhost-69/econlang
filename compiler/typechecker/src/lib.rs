#![forbid(unsafe_code)]

use ast::{
    BinaryOperator, BlockExpr, CallExpr, Decl, Expr, FunctionDecl, LiteralValue, Pattern, Program,
    Span, Stmt, TypeExpr,
};
use std::collections::HashMap;

/// Internal semantic type representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Unit,
    Bool,
    Char,
    String,

    Int8,
    Int16,
    Int32,
    Int64,
    Int128,

    UInt8,
    UInt16,
    UInt32,
    UInt64,
    UInt128,

    Float32,
    Float64,

    Array(Box<Type>),
    Tuple(Vec<Type>),

    Reference {
        inner: Box<Type>,
        mutable: bool,
    },

    Function {
        parameters: Vec<Type>,
        return_type: Box<Type>,
    },

    Named(String),
    Unknown,
}

/// Active borrow state for a binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BorrowState {
    Shared(usize),
    Mutable,
}

/// A local binding known by the type checker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct BindingId(usize);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Binding {
    id: BindingId,
    ty: Type,
    mutable: bool,
    span: Span,
}

/// A function signature.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionSignature {
    parameters: Vec<Type>,
    return_type: Type,
}

/// Type checker diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
}

/// A type-checking diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub span: Span,
}

/// Result of type checking.
#[derive(Debug, Default)]
pub struct TypeCheckResult {
    pub diagnostics: Vec<Diagnostic>,
}

impl TypeCheckResult {
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty()
    }
}

/// Main type checker.
#[derive(Debug)]
pub struct TypeChecker {
    scopes: Vec<HashMap<String, Binding>>,
    functions: HashMap<String, FunctionSignature>,
    diagnostics: Vec<Diagnostic>,
    current_return_type: Type,
    borrows: HashMap<BindingId, BorrowState>,
    scope_borrows: Vec<Vec<(BindingId, bool)>>,
    next_binding_id: usize,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            diagnostics: Vec::new(),
            current_return_type: Type::Unit,
            borrows: HashMap::new(),
            scope_borrows: vec![Vec::new()],
            next_binding_id: 0,
        }
    }

    /// Type-checks an entire program.
    pub fn check(mut self, program: &Program) -> TypeCheckResult {
        self.collect_functions(program);

        for declaration in &program.declarations {
            match declaration {
                Decl::Function(function) => self.check_function(function),
                Decl::Const(const_decl) => {
                    let expected = self.lower_type(&const_decl.ty);
                    let actual = self.check_expr(&const_decl.value);

                    self.require_assignable(
                        &expected,
                        &actual,
                        const_decl.span,
                        "constant value type does not match declared type",
                    );
                }
                _ => {}
            }
        }

        TypeCheckResult {
            diagnostics: self.diagnostics,
        }
    }

    fn collect_functions(&mut self, program: &Program) {
        for declaration in &program.declarations {
            if let Decl::Function(function) = declaration {
                let parameters = function
                    .parameters
                    .iter()
                    .map(|parameter| self.lower_type(&parameter.ty))
                    .collect();

                let return_type = function
                    .return_type
                    .as_ref()
                    .map(|ty| self.lower_type(ty))
                    .unwrap_or(Type::Unknown);

                self.functions.insert(
                    function.name.clone(),
                    FunctionSignature {
                        parameters,
                        return_type,
                    },
                );
            }
        }
    }

    fn check_function(&mut self, function: &FunctionDecl) {
        self.enter_scope();

        for parameter in &function.parameters {
            let ty = self.lower_type(&parameter.ty);

            let binding_id = self.new_binding_id();

            self.scopes
                .last_mut()
                .expect("function scope exists")
                .insert(
                    parameter.name.clone(),
                    Binding {
                        id: binding_id,
                        ty,
                        mutable: false,
                        span: parameter.span,
                    },
                );
        }

        let new_return_type = function
            .return_type
            .as_ref()
            .map(|ty| self.lower_type(ty))
            .unwrap_or(Type::Unknown);

        let previous_return = std::mem::replace(&mut self.current_return_type, new_return_type);

        self.check_block(&function.body);

        self.current_return_type = previous_return;
        self.exit_scope();
    }

    fn check_block(&mut self, block: &BlockExpr) {
        self.enter_scope();

        for statement in &block.statements {
            self.check_stmt(statement);
        }

        if let Some(expression) = &block.trailing_expr {
            self.check_expr(expression);
        }

        self.exit_scope();
    }

    fn check_stmt(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Let(let_stmt) => {
                let value_type = let_stmt
                    .value
                    .as_ref()
                    .map(|expr| self.check_expr(expr))
                    .unwrap_or(Type::Unknown);

                let binding_type = if let Some(type_expr) = &let_stmt.ty {
                    let declared_type = self.lower_type(type_expr);

                    if let_stmt.value.is_some() {
                        self.require_assignable(
                            &declared_type,
                            &value_type,
                            let_stmt.span,
                            "let binding has incompatible value type",
                        );
                    }

                    declared_type
                } else {
                    value_type
                };

                self.bind_pattern(&let_stmt.pattern, binding_type, false);
            }

            Stmt::Var(var_stmt) => {
                let inferred = var_stmt
                    .value
                    .as_ref()
                    .map(|expr| self.check_expr(expr))
                    .unwrap_or(Type::Unknown);

                let ty = var_stmt
                    .ty
                    .as_ref()
                    .map(|type_expr| self.lower_type(type_expr))
                    .unwrap_or(inferred);

                self.bind_pattern(&var_stmt.pattern, ty, true);
            }

            Stmt::Expr(expr_stmt) => {
                self.check_expr(&expr_stmt.expr);
            }

            Stmt::Return(return_stmt) => {
                let actual = return_stmt
                    .value
                    .as_ref()
                    .map(|expr| self.check_expr(expr))
                    .unwrap_or(Type::Unit);

                if self.current_return_type == Type::Unknown {
                    self.current_return_type = actual;
                } else {
                    let expected = self.current_return_type.clone();
                    self.require_assignable(
                        &expected,
                        &actual,
                        return_stmt.span,
                        "return type does not match function return type",
                    );
                }
            }

            Stmt::While(while_stmt) => {
                let condition = self.check_expr(&while_stmt.condition);

                self.require_type(
                    &Type::Bool,
                    &condition,
                    while_stmt.span,
                    "while condition must be Bool",
                );

                self.check_block(&while_stmt.body);
            }

            Stmt::For(for_stmt) => {
                let iterable = self.check_expr(&for_stmt.iterable);

                let element_type = match iterable {
                    Type::Array(inner) => *inner,
                    Type::String => Type::Char,
                    Type::Unknown => Type::Unknown,
                    _ => {
                        self.error(
                            "for-loop expression must be an Array or String",
                            for_stmt.span,
                        );
                        Type::Unknown
                    }
                };

                self.enter_scope();
                self.bind_pattern(&for_stmt.pattern, element_type, false);
                self.check_block(&for_stmt.body);
                self.exit_scope();
            }

            Stmt::ParallelFor(for_stmt) => {
                let iterable = self.check_expr(&for_stmt.iterable);

                let element_type = match iterable {
                    Type::Array(inner) => *inner,
                    Type::String => Type::Char,
                    Type::Unknown => Type::Unknown,
                    _ => {
                        self.error(
                            "parallel for-loop expression must be an Array or String",
                            for_stmt.span,
                        );
                        Type::Unknown
                    }
                };

                self.enter_scope();
                self.bind_pattern(&for_stmt.pattern, element_type, false);
                self.check_block(&for_stmt.body);
                self.exit_scope();
            }

            Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    fn check_expr(&mut self, expression: &Expr) -> Type {
        match expression {
            Expr::Literal(literal) => self.literal_type(&literal.value),

            Expr::Identifier(identifier) => self
                .lookup(&identifier.name)
                .map(|binding| binding.ty.clone())
                .unwrap_or_else(|| Type::Unknown),

            Expr::Binary(binary) => {
                let left = self.check_expr(&binary.left);
                let right = self.check_expr(&binary.right);

                self.check_binary(binary.operator, left, right, binary.span)
            }

            Expr::Assignment(assignment) => {
                let value_type = self.check_expr(&assignment.value);

                match assignment.target.as_ref() {
                    Expr::Identifier(identifier) => {
                        if let Some(binding) = self.lookup(&identifier.name).cloned() {
                            if !binding.mutable {
                                self.error(
                                    format!(
                                        "cannot assign to immutable binding `{}`",
                                        identifier.name
                                    ),
                                    identifier.span,
                                );
                            }

                            self.require_assignable(
                                &binding.ty,
                                &value_type,
                                assignment.span,
                                "assigned value has incompatible type",
                            );

                            binding.ty
                        } else {
                            Type::Unknown
                        }
                    }

                    _ => {
                        self.error("assignment target must be an identifier", assignment.span);
                        Type::Unknown
                    }
                }
            }

            Expr::Unary(unary) => {
                let operand_type = self.check_expr(&unary.operand);

                match unary.operator {
                    ast::UnaryOperator::Neg => {
                        if !self.is_numeric(&operand_type) && operand_type != Type::Unknown {
                            self.error("unary '-' requires a numeric operand", unary.span);
                        }

                        operand_type
                    }

                    ast::UnaryOperator::Not => {
                        self.require_type(
                            &Type::Bool,
                            &operand_type,
                            unary.span,
                            "logical '!' requires Bool",
                        );

                        Type::Bool
                    }

                    ast::UnaryOperator::BorrowShared => {
                        if let Expr::Identifier(identifier) = unary.operand.as_ref() {
                            if let Some(binding) = self.lookup(&identifier.name).cloned() {
                                self.acquire_borrow(
                                    binding.id,
                                    false,
                                    identifier.span,
                                    &identifier.name,
                                );
                            }
                        } else {
                            self.error("shared borrow requires an identifier", unary.span);
                        }

                        Type::Reference {
                            inner: Box::new(operand_type),
                            mutable: false,
                        }
                    }

                    ast::UnaryOperator::BorrowMutable => {
                        match unary.operand.as_ref() {
                            Expr::Identifier(identifier) => {
                                if let Some(binding) = self.lookup(&identifier.name).cloned() {
                                    if !binding.mutable {
                                        self.error(
                                            format!(
                                                "cannot mutably borrow immutable binding `{}`",
                                                identifier.name
                                            ),
                                            identifier.span,
                                        );
                                    } else {
                                        self.acquire_borrow(
                                            binding.id,
                                            true,
                                            identifier.span,
                                            &identifier.name,
                                        );
                                    }
                                }
                            }

                            _ => {
                                self.error("mutable borrow requires an identifier", unary.span);
                            }
                        }

                        Type::Reference {
                            inner: Box::new(operand_type),
                            mutable: true,
                        }
                    }
                }
            }

            Expr::Call(call) => self.check_call(call),

            Expr::Member(member) => {
                self.check_expr(&member.object);
                Type::Unknown
            }

            Expr::Index(index) => {
                let object_type = self.check_expr(&index.object);

                for index_expr in &index.indices {
                    let index_type = self.check_expr(index_expr);

                    if index_type != Type::Int64 && index_type != Type::Unknown {
                        self.error(
                            "array index must be Int64",
                            ast::Span::new(
                                index_expr_span(index_expr).start,
                                index_expr_span(index_expr).end,
                            ),
                        );
                    }
                }

                match object_type {
                    Type::Array(inner) => *inner,
                    Type::String => Type::Char,
                    Type::Unknown => Type::Unknown,
                    _ => {
                        self.error("value is not indexable", index.span);
                        Type::Unknown
                    }
                }
            }

            Expr::Slice(slice) => {
                let object_type = self.check_expr(&slice.object);

                if let Some(start) = &slice.start {
                    let start_type = self.check_expr(start);
                    self.require_type(
                        &Type::Int64,
                        &start_type,
                        index_expr_span(start),
                        "slice start must be Int64",
                    );
                }

                if let Some(end) = &slice.end {
                    let end_type = self.check_expr(end);
                    self.require_type(
                        &Type::Int64,
                        &end_type,
                        index_expr_span(end),
                        "slice end must be Int64",
                    );
                }

                object_type
            }

            Expr::Array(array) => {
                if array.elements.is_empty() {
                    return Type::Array(Box::new(Type::Unknown));
                }

                let first = self.check_expr(&array.elements[0]);

                for element in array.elements.iter().skip(1) {
                    let ty = self.check_expr(element);

                    if !self.are_compatible(&first, &ty) {
                        self.error(
                            "array elements must have compatible types",
                            expression_span(element),
                        );
                    }
                }

                Type::Array(Box::new(first))
            }

            Expr::Tuple(tuple) => Type::Tuple(
                tuple
                    .elements
                    .iter()
                    .map(|element| self.check_expr(element))
                    .collect(),
            ),

            Expr::Block(block) => {
                self.check_block(block);
                block
                    .trailing_expr
                    .as_ref()
                    .map(|expr| self.check_expr(expr))
                    .unwrap_or(Type::Unit)
            }

            Expr::If(if_expr) => {
                let condition = self.check_expr(&if_expr.condition);

                self.require_type(
                    &Type::Bool,
                    &condition,
                    expression_span(expression),
                    "if condition must be Bool",
                );

                self.check_block(&if_expr.then_branch);

                if let Some(else_branch) = &if_expr.else_branch {
                    self.check_expr(else_branch)
                } else {
                    Type::Unit
                }
            }

            Expr::Match(match_expr) => {
                let _ = self.check_expr(&match_expr.scrutinee);

                for arm in &match_expr.arms {
                    self.bind_match_pattern(&arm.pattern);

                    if let Some(guard) = &arm.guard {
                        let guard_type = self.check_expr(guard);

                        self.require_type(
                            &Type::Bool,
                            &guard_type,
                            expression_span(guard),
                            "match guard must be Bool",
                        );
                    }

                    self.check_expr(&arm.body);

                    self.exit_scope();
                }

                Type::Unknown
            }

            Expr::Range(range) => {
                if let Some(start) = &range.start {
                    let start_type = self.check_expr(start);
                    self.require_type(
                        &Type::Int64,
                        &start_type,
                        index_expr_span(start),
                        "range start must be Int64",
                    );
                }

                if let Some(end) = &range.end {
                    let end_type = self.check_expr(end);
                    self.require_type(
                        &Type::Int64,
                        &end_type,
                        index_expr_span(end),
                        "range end must be Int64",
                    );
                }

                Type::Array(Box::new(Type::Int64))
            }

            Expr::Formula(formula) => {
                self.check_expr(&formula.response);

                for predictor in &formula.predictors {
                    self.check_expr(predictor);
                }

                Type::Unknown
            }

            Expr::StructLiteral(struct_literal) => {
                for field in &struct_literal.fields {
                    self.check_expr(&field.value);
                }

                Type::Named(struct_literal.name.clone())
            }

            Expr::Cast(cast) => {
                self.check_expr(&cast.expr);
                self.lower_type(&cast.target_type)
            }

            Expr::Await(await_expr) => self.check_expr(&await_expr.expr),
        }
    }

    fn check_call(&mut self, call: &CallExpr) -> Type {
        let function_name = match call.callee.as_ref() {
            Expr::Identifier(identifier) => identifier.name.as_str(),
            _ => {
                self.check_expr(&call.callee);
                for argument in &call.arguments {
                    self.check_expr(argument);
                }
                self.error("call target must be a function name", call.span);
                return Type::Unknown;
            }
        };

        let Some(signature) = self.functions.get(function_name).cloned() else {
            self.error(
                format!("cannot call unknown function `{}`", function_name),
                call.span,
            );

            for argument in &call.arguments {
                self.check_expr(argument);
            }

            return Type::Unknown;
        };

        if signature.parameters.len() != call.arguments.len() {
            self.error(
                format!(
                    "function `{}` expects {} argument(s), got {}",
                    function_name,
                    signature.parameters.len(),
                    call.arguments.len()
                ),
                call.span,
            );
        }

        for (index, argument) in call.arguments.iter().enumerate() {
            let actual = self.check_expr(argument);

            if let Some(expected) = signature.parameters.get(index) {
                self.require_assignable(
                    expected,
                    &actual,
                    expression_span(argument),
                    "function argument has incompatible type",
                );
            }
        }

        signature.return_type
    }

    fn check_binary(
        &mut self,
        operator: BinaryOperator,
        left: Type,
        right: Type,
        span: Span,
    ) -> Type {
        match operator {
            BinaryOperator::Add
            | BinaryOperator::Sub
            | BinaryOperator::Mul
            | BinaryOperator::Div
            | BinaryOperator::Mod => {
                if !self.is_numeric(&left) && left != Type::Unknown {
                    self.error("left operand must be numeric", span);
                }

                if !self.is_numeric(&right) && right != Type::Unknown {
                    self.error("right operand must be numeric", span);
                }

                if !self.are_compatible(&left, &right) {
                    self.error("binary operands must have compatible types", span);
                }

                if left == Type::Unknown { right } else { left }
            }

            BinaryOperator::Eq
            | BinaryOperator::Ne
            | BinaryOperator::Lt
            | BinaryOperator::Le
            | BinaryOperator::Gt
            | BinaryOperator::Ge => {
                if !self.are_compatible(&left, &right) {
                    self.error("comparison operands must have compatible types", span);
                }

                Type::Bool
            }

            BinaryOperator::And | BinaryOperator::Or => {
                self.require_type(
                    &Type::Bool,
                    &left,
                    span,
                    "logical left operand must be Bool",
                );
                self.require_type(
                    &Type::Bool,
                    &right,
                    span,
                    "logical right operand must be Bool",
                );

                Type::Bool
            }

            BinaryOperator::MatrixMul => {
                self.require_numeric_or_unknown(&left, span, "matrix operands must be numeric");
                self.require_numeric_or_unknown(&right, span, "matrix operands must be numeric");

                Type::Unknown
            }

            BinaryOperator::ElementWiseMul | BinaryOperator::ElementWiseDiv => {
                self.require_numeric_or_unknown(
                    &left,
                    span,
                    "element-wise operands must be numeric",
                );
                self.require_numeric_or_unknown(
                    &right,
                    span,
                    "element-wise operands must be numeric",
                );

                Type::Unknown
            }

            BinaryOperator::Formula => Type::Unknown,
        }
    }

    fn literal_type(&self, literal: &LiteralValue) -> Type {
        match literal {
            LiteralValue::Integer(_) => Type::Int64,
            LiteralValue::Float(_) => Type::Float64,
            LiteralValue::Bool(_) => Type::Bool,
            LiteralValue::String(_) => Type::String,
            LiteralValue::Char(_) => Type::Char,
        }
    }

    fn lower_type(&self, type_expr: &TypeExpr) -> Type {
        match type_expr {
            TypeExpr::Named { name, .. } => match name.as_str() {
                "Bool" => Type::Bool,
                "Char" => Type::Char,
                "String" => Type::String,

                "Int8" => Type::Int8,
                "Int16" => Type::Int16,
                "Int32" => Type::Int32,
                "Int64" => Type::Int64,
                "Int128" => Type::Int128,

                "UInt8" => Type::UInt8,
                "UInt16" => Type::UInt16,
                "UInt32" => Type::UInt32,
                "UInt64" => Type::UInt64,
                "UInt128" => Type::UInt128,

                "Float32" => Type::Float32,
                "Float64" => Type::Float64,

                "Unit" => Type::Unit,

                _ => Type::Named(name.clone()),
            },

            TypeExpr::Generic {
                name, arguments, ..
            } => {
                let lowered: Vec<Type> = arguments
                    .iter()
                    .map(|argument| self.lower_type(argument))
                    .collect();

                match name.as_str() {
                    "Array" if lowered.len() == 1 => Type::Array(Box::new(lowered[0].clone())),

                    "Option" | "Result" | "Vector" | "Matrix" | "Tensor" | "Series"
                    | "TimeSeries" | "Categorical" | "Money" => Type::Named(format!(
                        "{}<{}>",
                        name,
                        lowered
                            .iter()
                            .map(|ty| format!("{ty:?}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),

                    _ => Type::Named(name.clone()),
                }
            }

            TypeExpr::Reference { inner, mutable, .. } => Type::Reference {
                inner: Box::new(self.lower_type(inner)),
                mutable: *mutable,
            },

            TypeExpr::Tuple { elements, .. } => Type::Tuple(
                elements
                    .iter()
                    .map(|element| self.lower_type(element))
                    .collect(),
            ),

            TypeExpr::Array { element, .. } => Type::Array(Box::new(self.lower_type(element))),

            TypeExpr::Function {
                parameters,
                return_type,
                ..
            } => Type::Function {
                parameters: parameters
                    .iter()
                    .map(|parameter| self.lower_type(parameter))
                    .collect(),
                return_type: Box::new(self.lower_type(return_type)),
            },
        }
    }

    fn bind_pattern(&mut self, pattern: &Pattern, ty: Type, mutable: bool) {
        match pattern {
            Pattern::Identifier { name, span } => {
                let binding_id = self.new_binding_id();

                self.scopes.last_mut().expect("scope exists").insert(
                    name.clone(),
                    Binding {
                        id: binding_id,
                        ty,
                        mutable,
                        span: *span,
                    },
                );
            }

            Pattern::Wildcard { .. } => {}

            Pattern::Literal { .. } => {
                self.error(
                    "literal patterns cannot declare a local binding",
                    pattern_span(pattern),
                );
            }

            Pattern::Tuple { elements, .. } => {
                for element in elements {
                    self.bind_pattern(element, Type::Unknown, mutable);
                }
            }

            Pattern::EnumVariant { fields, .. } => {
                for field in fields {
                    self.bind_pattern(field, Type::Unknown, mutable);
                }
            }
        }
    }

    fn new_binding_id(&mut self) -> BindingId {
        let id = BindingId(self.next_binding_id);
        self.next_binding_id += 1;
        id
    }

    fn acquire_borrow(&mut self, id: BindingId, mutable: bool, span: Span, name: &str) {
        match (self.borrows.get(&id).copied(), mutable) {
            (None, false) => {
                self.borrows.insert(id, BorrowState::Shared(1));
                self.scope_borrows
                    .last_mut()
                    .expect("scope borrow stack exists")
                    .push((id, false));
            }

            (Some(BorrowState::Shared(count)), false) => {
                self.borrows.insert(id, BorrowState::Shared(count + 1));
                self.scope_borrows
                    .last_mut()
                    .expect("scope borrow stack exists")
                    .push((id, false));
            }

            (None, true) => {
                self.borrows.insert(id, BorrowState::Mutable);
                self.scope_borrows
                    .last_mut()
                    .expect("scope borrow stack exists")
                    .push((id, true));
            }

            (Some(BorrowState::Shared(_)), true) => {
                self.error(
                    format!("cannot mutably borrow `{name}` while shared borrow is active"),
                    span,
                );
            }

            (Some(BorrowState::Mutable), false) => {
                self.error(
                    "cannot immutably borrow while a mutable borrow is active",
                    span,
                );
            }

            (Some(BorrowState::Mutable), true) => {
                self.error(
                    "cannot mutably borrow while another mutable borrow is active",
                    span,
                );
            }
        }
    }

    fn release_borrow(&mut self, id: BindingId, mutable: bool) {
        match (self.borrows.get(&id).copied(), mutable) {
            (Some(BorrowState::Shared(count)), false) if count > 1 => {
                self.borrows.insert(id, BorrowState::Shared(count - 1));
            }

            (Some(BorrowState::Shared(_)), false) => {
                self.borrows.remove(&id);
            }

            (Some(BorrowState::Mutable), true) => {
                self.borrows.remove(&id);
            }

            _ => {}
        }
    }

    fn bind_match_pattern(&mut self, pattern: &Pattern) {
        self.enter_scope();
        self.bind_pattern(pattern, Type::Unknown, false);
    }

    fn lookup(&self, name: &str) -> Option<&Binding> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
        self.scope_borrows.push(Vec::new());
    }

    fn exit_scope(&mut self) {
        if self.scopes.len() <= 1 {
            return;
        }

        self.scopes.pop();

        let borrows = self.scope_borrows.pop().expect("scope borrow stack exists");

        for (id, mutable) in borrows {
            self.release_borrow(id, mutable);
        }
    }

    fn require_type(&mut self, expected: &Type, actual: &Type, span: Span, message: &str) {
        if actual == &Type::Unknown {
            return;
        }

        if expected != actual {
            self.error(
                format!("{message}: expected {expected:?}, got {actual:?}"),
                span,
            );
        }
    }

    fn require_assignable(&mut self, expected: &Type, actual: &Type, span: Span, message: &str) {
        if !self.are_compatible(expected, actual) {
            self.error(
                format!("{message}: expected {expected:?}, got {actual:?}"),
                span,
            );
        }
    }

    fn are_compatible(&self, left: &Type, right: &Type) -> bool {
        if left == &Type::Unknown || right == &Type::Unknown {
            return true;
        }

        left == right
    }

    fn is_numeric(&self, ty: &Type) -> bool {
        matches!(
            ty,
            Type::Int8
                | Type::Int16
                | Type::Int32
                | Type::Int64
                | Type::Int128
                | Type::UInt8
                | Type::UInt16
                | Type::UInt32
                | Type::UInt64
                | Type::UInt128
                | Type::Float32
                | Type::Float64
        )
    }

    fn require_numeric_or_unknown(&mut self, ty: &Type, span: Span, message: &str) {
        if ty != &Type::Unknown && !self.is_numeric(ty) {
            self.error(message, span);
        }
    }

    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.diagnostics.push(Diagnostic {
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            span,
        });
    }
}

/// Convenience entry point.
pub fn check(program: &Program) -> TypeCheckResult {
    TypeChecker::new().check(program)
}

fn expression_span(expression: &Expr) -> Span {
    match expression {
        Expr::Literal(node) => node.span,
        Expr::Identifier(node) => node.span,
        Expr::Binary(node) => node.span,
        Expr::Assignment(node) => node.span,
        Expr::Unary(node) => node.span,
        Expr::Call(node) => node.span,
        Expr::Member(node) => node.span,
        Expr::Index(node) => node.span,
        Expr::Slice(node) => node.span,
        Expr::Array(node) => node.span,
        Expr::Tuple(node) => node.span,
        Expr::Block(node) => node.span,
        Expr::If(node) => node.span,
        Expr::Match(node) => node.span,
        Expr::Range(node) => node.span,
        Expr::Formula(node) => node.span,
        Expr::StructLiteral(node) => node.span,
        Expr::Cast(node) => node.span,
        Expr::Await(node) => node.span,
    }
}

fn index_expr_span(expression: &Expr) -> Span {
    expression_span(expression)
}

fn pattern_span(pattern: &Pattern) -> Span {
    match pattern {
        Pattern::Identifier { span, .. } => *span,
        Pattern::Wildcard { span } => *span,
        Pattern::Literal { span, .. } => *span,
        Pattern::Tuple { span, .. } => *span,
        Pattern::EnumVariant { span, .. } => *span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parser::Parser;

    fn check_source(source: &str) -> TypeCheckResult {
        let mut parser = Parser::from_source(source).expect("source should lex");
        let program = parser.parse_program().expect("source should parse");
        check(&program)
    }

    #[test]
    fn accepts_integer_addition() {
        let result = check_source(
            r#"
            fn main() -> Int64 {
                let x: Int64 = 10;
                let y: Int64 = 20;
                return x + y;
            }
            "#,
        );

        assert!(result.is_ok(), "{:?}", result.diagnostics);
    }

    #[test]
    fn accepts_float_addition() {
        let result = check_source(
            r#"
            fn main() -> Float64 {
                let x: Float64 = 1.5;
                let y: Float64 = 2.5;
                return x + y;
            }
            "#,
        );

        assert!(result.is_ok(), "{:?}", result.diagnostics);
    }

    #[test]
    fn rejects_integer_plus_bool() {
        let result = check_source(
            r#"
            fn main() -> Int64 {
                let x: Int64 = 10;
                let flag: Bool = true;
                return x + flag;
            }
            "#,
        );

        assert!(result.has_errors());
    }

    #[test]
    fn rejects_wrong_variable_type() {
        let result = check_source(
            r#"
            fn main() {
                let x: Int64 = true;
            }
            "#,
        );

        assert!(result.has_errors());
    }

    #[test]
    fn rejects_assignment_to_immutable_binding() {
        let result = check_source(
            r#"
            fn main() {
                let x: Int64 = 10;
                x = 20;
            }
            "#,
        );

        assert!(result.has_errors());
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot assign to immutable binding `x`")
        }));
    }

    #[test]
    fn accepts_assignment_to_mutable_binding() {
        let result = check_source(
            r#"
            fn main() {
                var x: Int64 = 10;
                x = 20;
            }
            "#,
        );

        assert!(result.is_ok(), "{:?}", result.diagnostics);
    }

    #[test]
    fn rejects_wrong_function_argument_type() {
        let result = check_source(
            r#"
            fn add(a: Int64, b: Int64) -> Int64 {
                return a + b;
            }

            fn main() -> Int64 {
                return add(10, true);
            }
            "#,
        );

        assert!(result.has_errors());
    }

    #[test]
    fn rejects_wrong_function_return_type() {
        let result = check_source(
            r#"
            fn main() -> Int64 {
                return true;
            }
            "#,
        );

        assert!(result.has_errors());
    }

    #[test]
    fn accepts_boolean_condition() {
        let result = check_source(
            r#"
            fn main() {
                if true {
                    let x: Int64 = 10;
                }
            }
            "#,
        );

        assert!(result.is_ok(), "{:?}", result.diagnostics);
    }

    #[test]
    fn rejects_non_boolean_condition() {
        let result = check_source(
            r#"
            fn main() {
                if 10 {
                    let x: Int64 = 10;
                }
            }
            "#,
        );

        assert!(result.has_errors());
    }

    #[test]
    fn accepts_shared_borrow() {
        let result = check_source(
            r#"
            fn inspect(x: &Int64) {
                return;
            }

            fn main() {
                var value: Int64 = 10;
                inspect(&value);
            }
            "#,
        );

        assert!(result.is_ok(), "{:?}", result.diagnostics);
    }

    #[test]
    fn accepts_mutable_borrow_from_mutable_binding() {
        let result = check_source(
            r#"
            fn update(x: &mut Int64) {
                return;
            }

            fn main() {
                var value: Int64 = 10;
                update(&mut value);
            }
            "#,
        );

        assert!(result.is_ok(), "{:?}", result.diagnostics);
    }

    #[test]
    fn rejects_mutable_borrow_from_immutable_binding() {
        let result = check_source(
            r#"
            fn update(x: &mut Int64) {
                return;
            }

            fn main() {
                let value: Int64 = 10;
                update(&mut value);
            }
            "#,
        );

        assert!(result.has_errors());
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot mutably borrow immutable binding `value`")
        }));
    }

    #[test]
    fn accepts_multiple_shared_borrows() {
        let result = check_source(
            r#"
            fn main() {
                var x: Int64 = 10;
                let a = &x;
                let b = &x;
            }
            "#,
        );

        assert!(result.is_ok(), "{:?}", result.diagnostics);
    }

    #[test]
    fn rejects_mutable_borrow_while_shared_borrow_is_active() {
        let result = check_source(
            r#"
            fn main() {
                var x: Int64 = 10;
                let a = &x;
                let b = &mut x;
            }
            "#,
        );

        assert!(result.has_errors());
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot mutably borrow `x` while shared borrow is active")
        }));
    }

    #[test]
    fn rejects_two_mutable_borrows() {
        let result = check_source(
            r#"
            fn main() {
                var x: Int64 = 10;
                let a = &mut x;
                let b = &mut x;
            }
            "#,
        );

        assert!(result.has_errors());
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("another mutable borrow is active")
        }));
    }

    #[test]
    fn releases_borrow_when_binding_scope_ends() {
        let result = check_source(
            r#"
            fn main() {
                var x: Int64 = 10;

                if true {
                    let shared = &x;
                }

                let mutable = &mut x;
            }
            "#,
        );

        assert!(result.is_ok(), "{:?}", result.diagnostics);
    }
}
