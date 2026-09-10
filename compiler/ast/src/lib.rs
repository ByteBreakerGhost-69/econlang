#![forbid(unsafe_code)]

//! Abstract Syntax Tree (AST) for the EconLang programming language.
//!
//! This crate contains only syntax-level structures.
//! Semantic information such as inferred types belongs to later compiler stages.

/// A location in the original source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// The root node of an EconLang source file.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub declarations: Vec<Decl>,
}

/// Top-level declarations.
#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    Function(FunctionDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Trait(TraitDecl),
    Impl(ImplDecl),
    Const(ConstDecl),
    Import(ImportDecl),
}

/// Function declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub generic_params: Vec<GenericParam>,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<TypeExpr>,
    pub body: BlockExpr,
    pub span: Span,
}

/// Function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

/// Struct declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub generic_params: Vec<GenericParam>,
    pub fields: Vec<Field>,
    pub span: Span,
}

/// Struct field.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

/// Enum declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub generic_params: Vec<GenericParam>,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

/// Enum variant.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Vec<TypeExpr>,
    pub span: Span,
}

/// Trait declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct TraitDecl {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub generic_params: Vec<GenericParam>,
    pub members: Vec<TraitMember>,
    pub span: Span,
}

/// Trait member.
#[derive(Debug, Clone, PartialEq)]
pub enum TraitMember {
    Function(FunctionSignature),
}

/// Function signature without a body.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSignature {
    pub name: String,
    pub generic_params: Vec<GenericParam>,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<TypeExpr>,
    pub span: Span,
}

/// Implementation declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ImplDecl {
    pub generic_params: Vec<GenericParam>,
    pub trait_type: Option<TypeExpr>,
    pub target_type: TypeExpr,
    pub members: Vec<ImplMember>,
    pub span: Span,
}

/// Implementation member.
#[derive(Debug, Clone, PartialEq)]
pub enum ImplMember {
    Function(FunctionDecl),
}

/// Constant declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstDecl {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub ty: TypeExpr,
    pub value: Expr,
    pub span: Span,
}

/// Import declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    pub path: Vec<String>,
    pub span: Span,
}

/// Attributes such as `@vectorize` or `@parallel`.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub arguments: Vec<Expr>,
    pub span: Span,
}

/// Generic type parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct GenericParam {
    pub name: String,
    pub bounds: Vec<TypeExpr>,
    pub span: Span,
}

/// Statements inside a block.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let(LetStmt),
    Var(VarStmt),
    Expr(ExprStmt),
    Return(ReturnStmt),
    While(WhileStmt),
    For(ForStmt),
    ParallelFor(ParallelForStmt),
    Break(BreakStmt),
    Continue(ContinueStmt),
}

/// Immutable binding.
#[derive(Debug, Clone, PartialEq)]
pub struct LetStmt {
    pub pattern: Pattern,
    pub ty: Option<TypeExpr>,
    pub value: Option<Expr>,
    pub span: Span,
}

/// Mutable binding.
#[derive(Debug, Clone, PartialEq)]
pub struct VarStmt {
    pub pattern: Pattern,
    pub ty: Option<TypeExpr>,
    pub value: Option<Expr>,
    pub span: Span,
}

/// Expression used as a statement.
#[derive(Debug, Clone, PartialEq)]
pub struct ExprStmt {
    pub expr: Expr,
    pub span: Span,
}

/// Return statement.
#[derive(Debug, Clone, PartialEq)]
pub struct ReturnStmt {
    pub value: Option<Expr>,
    pub span: Span,
}

/// While loop.
#[derive(Debug, Clone, PartialEq)]
pub struct WhileStmt {
    pub condition: Expr,
    pub body: BlockExpr,
    pub span: Span,
}

/// Standard for loop.
#[derive(Debug, Clone, PartialEq)]
pub struct ForStmt {
    pub pattern: Pattern,
    pub iterable: Expr,
    pub body: BlockExpr,
    pub span: Span,
}

/// Parallel for loop.
#[derive(Debug, Clone, PartialEq)]
pub struct ParallelForStmt {
    pub pattern: Pattern,
    pub iterable: Expr,
    pub body: BlockExpr,
    pub span: Span,
}

/// Break statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakStmt {
    pub span: Span,
}

/// Continue statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContinueStmt {
    pub span: Span,
}

/// Expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(LiteralExpr),
    Identifier(IdentifierExpr),
    Binary(BinaryExpr),
    Assignment(AssignmentExpr),
    Unary(UnaryExpr),
    Call(CallExpr),
    Member(MemberExpr),
    Index(IndexExpr),
    Slice(SliceExpr),
    Array(ArrayExpr),
    Tuple(TupleExpr),
    Block(BlockExpr),
    If(IfExpr),
    Match(MatchExpr),
    Range(RangeExpr),
    Formula(FormulaExpr),
    StructLiteral(StructLiteralExpr),
    Cast(CastExpr),
    Await(AwaitExpr),
}

/// Literal expression.
#[derive(Debug, Clone, PartialEq)]
pub struct LiteralExpr {
    pub value: LiteralValue,
    pub span: Span,
}

/// Literal values.
#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    Integer(String),
    Float(String),
    Bool(bool),
    String(String),
    Char(char),
}

/// Identifier expression.
#[derive(Debug, Clone, PartialEq)]
pub struct IdentifierExpr {
    pub name: String,
    pub span: Span,
}

/// Binary expression.
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryExpr {
    pub left: Box<Expr>,
    pub operator: BinaryOperator,
    pub right: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssignmentExpr {
    pub target: Box<Expr>,
    pub operator: AssignmentOperator,
    pub value: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentOperator {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
}

/// Unary expression.
#[derive(Debug, Clone, PartialEq)]
pub struct UnaryExpr {
    pub operator: UnaryOperator,
    pub operand: Box<Expr>,
    pub span: Span,
}

/// Function call.
#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    pub callee: Box<Expr>,
    pub generic_args: Vec<TypeExpr>,
    pub arguments: Vec<Expr>,
    pub span: Span,
}

/// Member access: `object.member`.
#[derive(Debug, Clone, PartialEq)]
pub struct MemberExpr {
    pub object: Box<Expr>,
    pub member: String,
    pub span: Span,
}

/// Indexing: `object[index]` or `object[i, j]`.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexExpr {
    pub object: Box<Expr>,
    pub indices: Vec<Expr>,
    pub span: Span,
}

/// Slice expression.
#[derive(Debug, Clone, PartialEq)]
pub struct SliceExpr {
    pub object: Box<Expr>,
    pub start: Option<Box<Expr>>,
    pub end: Option<Box<Expr>>,
    pub step: Option<Box<Expr>>,
    pub span: Span,
}

/// Array literal.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayExpr {
    pub elements: Vec<Expr>,
    pub span: Span,
}

/// Tuple expression.
#[derive(Debug, Clone, PartialEq)]
pub struct TupleExpr {
    pub elements: Vec<Expr>,
    pub span: Span,
}

/// Block expression.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockExpr {
    pub statements: Vec<Stmt>,
    pub trailing_expr: Option<Box<Expr>>,
    pub span: Span,
}

/// If expression.
#[derive(Debug, Clone, PartialEq)]
pub struct IfExpr {
    pub condition: Box<Expr>,
    pub then_branch: BlockExpr,
    pub else_branch: Option<Box<Expr>>,
    pub span: Span,
}

/// Match expression.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchExpr {
    pub scrutinee: Box<Expr>,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

/// Match arm.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

/// Range expression.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeExpr {
    pub start: Option<Box<Expr>>,
    pub end: Option<Box<Expr>>,
    pub inclusive: bool,
    pub span: Span,
}

/// Econometric formula such as `GDP ~ Capital + Labor`.
#[derive(Debug, Clone, PartialEq)]
pub struct FormulaExpr {
    pub response: Box<Expr>,
    pub predictors: Vec<Expr>,
    pub span: Span,
}

/// Struct literal.
#[derive(Debug, Clone, PartialEq)]
pub struct StructLiteralExpr {
    pub name: String,
    pub fields: Vec<StructFieldInit>,
    pub span: Span,
}

/// Struct field initializer.
#[derive(Debug, Clone, PartialEq)]
pub struct StructFieldInit {
    pub name: String,
    pub value: Expr,
    pub span: Span,
}

/// Explicit type cast.
#[derive(Debug, Clone, PartialEq)]
pub struct CastExpr {
    pub expr: Box<Expr>,
    pub target_type: TypeExpr,
    pub span: Span,
}

/// Await expression.
#[derive(Debug, Clone, PartialEq)]
pub struct AwaitExpr {
    pub expr: Box<Expr>,
    pub span: Span,
}

/// Patterns used by bindings and `match`.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Identifier {
        name: String,
        span: Span,
    },
    Wildcard {
        span: Span,
    },
    Literal {
        value: LiteralValue,
        span: Span,
    },
    Tuple {
        elements: Vec<Pattern>,
        span: Span,
    },
    EnumVariant {
        path: Vec<String>,
        fields: Vec<Pattern>,
        span: Span,
    },
}

/// Type expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Named {
        name: String,
        span: Span,
    },
    Generic {
        name: String,
        arguments: Vec<TypeExpr>,
        span: Span,
    },
    Reference {
        inner: Box<TypeExpr>,
        mutable: bool,
        span: Span,
    },
    Tuple {
        elements: Vec<TypeExpr>,
        span: Span,
    },
    Array {
        element: Box<TypeExpr>,
        size: Option<Box<Expr>>,
        span: Span,
    },
    Function {
        parameters: Vec<TypeExpr>,
        return_type: Box<TypeExpr>,
        span: Span,
    },
}

/// Binary operators supported by the AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Sub,
    Mul,
    Div,
    Mod,

    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    And,
    Or,

    MatrixMul,
    ElementWiseMul,
    ElementWiseDiv,

    Formula,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Neg,
    Not,
}
