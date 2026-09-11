#![forbid(unsafe_code)]

//! Parser for EconLang.
//!
//! Converts lexer tokens into the EconLang AST.

use ast::*;
use lexer::{LexError, Lexer, Token, TokenKind};

/// A parser error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

/// EconLang parser.
pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    /// Creates a parser directly from source code.
    pub fn from_source(source: &str) -> Result<Self, LexError> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize()?;

        Ok(Self { tokens, current: 0 })
    }

    /// Creates a parser from an existing token stream.
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    /// Parses an entire EconLang program.
    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut declarations = Vec::new();

        while !self.check(&TokenKind::Eof) {
            declarations.push(self.parse_declaration()?);
        }

        Ok(Program { declarations })
    }

    fn parse_declaration(&mut self) -> Result<Decl, ParseError> {
        match self.peek_kind() {
            TokenKind::Fn => self.parse_function(),
            _ => Err(self.error_here("expected a top-level declaration")),
        }
    }

    // -------------------------------------------------------------------------
    // Declarations
    // -------------------------------------------------------------------------

    fn parse_function(&mut self) -> Result<Decl, ParseError> {
        let start = self.advance().span.start;

        let name_token = self.consume_identifier("expected function name")?;

        let name = match name_token.kind {
            TokenKind::Identifier(name) => name,
            _ => unreachable!(),
        };

        self.consume(&TokenKind::LeftParen, "expected '(' after function name")?;

        let parameters = self.parse_parameters()?;

        self.consume(&TokenKind::RightParen, "expected ')' after parameters")?;

        let return_type = if self.match_kind(&TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = self.parse_block()?;

        Ok(Decl::Function(FunctionDecl {
            attributes: Vec::new(),
            name,
            generic_params: Vec::new(),
            parameters,
            return_type,
            span: Span::new(start, body.span.end),
            body,
        }))
    }

    fn parse_parameters(&mut self) -> Result<Vec<Parameter>, ParseError> {
        let mut parameters = Vec::new();

        if self.check(&TokenKind::RightParen) {
            return Ok(parameters);
        }

        loop {
            let name_token = self.consume_identifier("expected parameter name")?;

            let name = match name_token.kind {
                TokenKind::Identifier(name) => name,
                _ => unreachable!(),
            };

            self.consume(&TokenKind::Colon, "expected ':' after parameter name")?;

            let ty = self.parse_type()?;

            parameters.push(Parameter {
                name,
                ty,
                span: ast_span(name_token.span),
            });

            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }

        Ok(parameters)
    }

    fn parse_type(&mut self) -> Result<TypeExpr, ParseError> {
        let token = self.advance();

        match token.kind {
            TokenKind::Identifier(name) => Ok(TypeExpr::Named {
                name,
                span: ast_span(token.span),
            }),

            _ => Err(ParseError {
                message: "expected a type".to_string(),
                span: ast_span(token.span),
            }),
        }
    }

    // -------------------------------------------------------------------------
    // Statements
    // -------------------------------------------------------------------------

    fn parse_block(&mut self) -> Result<BlockExpr, ParseError> {
        let start = self.consume(&TokenKind::LeftBrace, "expected '{' to start block")?;

        let mut statements = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.check(&TokenKind::Eof) {
            statements.push(self.parse_statement()?);
        }

        let end = self.consume(&TokenKind::RightBrace, "expected '}' after block")?;

        Ok(BlockExpr {
            statements,
            trailing_expr: None,
            span: Span::new(start.span.start, end.span.end),
        })
    }

    fn parse_statement(&mut self) -> Result<Stmt, ParseError> {
        match self.peek_kind() {
            TokenKind::Let => self.parse_let_statement(),
            TokenKind::Var => self.parse_var_statement(),
            TokenKind::Return => self.parse_return_statement(),
            TokenKind::While => self.parse_while_statement(),
            TokenKind::For => self.parse_for_statement(),
            TokenKind::Parallel => self.parse_parallel_for_statement(),
            TokenKind::Break => self.parse_break_statement(),
            TokenKind::Continue => self.parse_continue_statement(),
            TokenKind::If => {
                // Control-flow `if` expressions may stand alone as statements
                // and therefore do not require a trailing semicolon.
                let expr = self.parse_expression()?;
                let span = expr_span(&expr);

                Ok(Stmt::Expr(ExprStmt { expr, span }))
            }
            _ => {
                let expr = self.parse_expression()?;
                let span = expr_span(&expr);

                self.consume(&TokenKind::Semicolon, "expected ';' after expression")?;

                Ok(Stmt::Expr(ExprStmt { expr, span }))
            }
        }
    }

    fn parse_let_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.advance().span.start;

        let name_token = self.consume_identifier("expected variable name")?;

        let name = match name_token.kind {
            TokenKind::Identifier(name) => name,
            _ => unreachable!(),
        };

        let pattern = Pattern::Identifier {
            name,
            span: ast_span(name_token.span),
        };

        let ty = if self.match_kind(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.consume(&TokenKind::Equal, "expected '=' in let statement")?;

        let value = self.parse_expression()?;

        let end = self.consume(&TokenKind::Semicolon, "expected ';' after let statement")?;

        Ok(Stmt::Let(LetStmt {
            pattern,
            ty,
            value: Some(value),
            span: Span::new(start, end.span.end),
        }))
    }

    fn parse_var_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.advance().span.start;

        let name_token = self.consume_identifier("expected variable name")?;

        let name = match name_token.kind {
            TokenKind::Identifier(name) => name,
            _ => unreachable!(),
        };

        let pattern = Pattern::Identifier {
            name,
            span: ast_span(name_token.span),
        };

        let ty = if self.match_kind(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let value = if self.match_kind(&TokenKind::Equal) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        let end = self.consume(&TokenKind::Semicolon, "expected ';' after var statement")?;

        Ok(Stmt::Var(VarStmt {
            pattern,
            ty,
            value,
            span: Span::new(start, end.span.end),
        }))
    }

    fn parse_return_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.advance().span.start;

        let value = if self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expression()?)
        };

        let end = self.consume(&TokenKind::Semicolon, "expected ';' after return statement")?;

        Ok(Stmt::Return(ReturnStmt {
            value,
            span: Span::new(start, end.span.end),
        }))
    }

    fn parse_while_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.advance().span.start;

        let condition = self.parse_expression()?;
        let body = self.parse_block()?;

        let end = body.span.end;

        Ok(Stmt::While(WhileStmt {
            condition,
            body,
            span: Span::new(start, end),
        }))
    }

    fn parse_for_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.advance().span.start;

        let name_token = self.consume_identifier("expected loop variable")?;

        let name = match name_token.kind {
            TokenKind::Identifier(name) => name,
            _ => unreachable!(),
        };

        let pattern = Pattern::Identifier {
            name,
            span: ast_span(name_token.span),
        };

        self.consume(&TokenKind::In, "expected 'in' in for loop")?;

        let iterable = self.parse_expression()?;
        let body = self.parse_block()?;

        Ok(Stmt::For(ForStmt {
            pattern,
            iterable,
            span: Span::new(start, body.span.end),
            body,
        }))
    }

    fn parse_parallel_for_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.advance().span.start;

        self.consume(&TokenKind::For, "expected 'for' after 'parallel'")?;

        let name_token = self.consume_identifier("expected loop variable")?;

        let name = match name_token.kind {
            TokenKind::Identifier(name) => name,
            _ => unreachable!(),
        };

        let pattern = Pattern::Identifier {
            name,
            span: ast_span(name_token.span),
        };

        self.consume(&TokenKind::In, "expected 'in' in parallel for loop")?;

        let iterable = self.parse_expression()?;
        let body = self.parse_block()?;

        Ok(Stmt::ParallelFor(ParallelForStmt {
            pattern,
            iterable,
            span: Span::new(start, body.span.end),
            body,
        }))
    }

    fn parse_break_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.advance().span.start;

        let end = self.consume(&TokenKind::Semicolon, "expected ';' after break")?;

        Ok(Stmt::Break(BreakStmt {
            span: Span::new(start, end.span.end),
        }))
    }

    fn parse_continue_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.advance().span.start;

        let end = self.consume(&TokenKind::Semicolon, "expected ';' after continue")?;

        Ok(Stmt::Continue(ContinueStmt {
            span: Span::new(start, end.span.end),
        }))
    }

    // -------------------------------------------------------------------------
    // Expressions
    // -------------------------------------------------------------------------

    fn parse_expression(&mut self) -> Result<Expr, ParseError> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<Expr, ParseError> {
        let target = self.parse_formula()?;

        let operator = if self.match_kind(&TokenKind::Equal) {
            Some(AssignmentOperator::Assign)
        } else if self.match_kind(&TokenKind::PlusEqual) {
            Some(AssignmentOperator::AddAssign)
        } else if self.match_kind(&TokenKind::MinusEqual) {
            Some(AssignmentOperator::SubAssign)
        } else if self.match_kind(&TokenKind::StarEqual) {
            Some(AssignmentOperator::MulAssign)
        } else if self.match_kind(&TokenKind::SlashEqual) {
            Some(AssignmentOperator::DivAssign)
        } else {
            None
        };

        let Some(operator) = operator else {
            return Ok(target);
        };

        let value = self.parse_assignment()?;

        let span = Span::new(expr_span(&target).start, expr_span(&value).end);

        Ok(Expr::Assignment(AssignmentExpr {
            target: Box::new(target),
            operator,
            value: Box::new(value),
            span,
        }))
    }

    fn parse_formula(&mut self) -> Result<Expr, ParseError> {
        let response = self.parse_range()?;
        let response_start = expr_span(&response).start;

        if self.match_kind(&TokenKind::Tilde) {
            let predictor_expr = self.parse_range()?;
            let predictors = flatten_addition(predictor_expr);

            let end = predictors
                .last()
                .map(expr_span)
                .unwrap_or(Span::new(response_start, response_start))
                .end;

            return Ok(Expr::Formula(FormulaExpr {
                response: Box::new(response),
                predictors,
                span: Span::new(response_start, end),
            }));
        }

        Ok(response)
    }

    /// Parses `start..end`.
    ///
    /// The lexer represents `..` as two `.` tokens. The postfix parser
    /// deliberately stops when it sees two consecutive dots.
    fn parse_range(&mut self) -> Result<Expr, ParseError> {
        let start_expr = self.parse_logical_or()?;

        if !self.check(&TokenKind::Dot) {
            return Ok(start_expr);
        }

        if !self.check_next(&TokenKind::Dot) {
            return Ok(start_expr);
        }

        self.advance();
        self.advance();

        let end_expr = if self.check(&TokenKind::LeftBrace)
            || self.check(&TokenKind::RightBracket)
            || self.check(&TokenKind::RightParen)
            || self.check(&TokenKind::Semicolon)
            || self.check(&TokenKind::Eof)
        {
            None
        } else {
            Some(Box::new(self.parse_logical_or()?))
        };

        let end = end_expr
            .as_ref()
            .map(|expr| expr_span(expr).end)
            .unwrap_or_else(|| self.tokens[self.current - 1].span.end);

        let start = expr_span(&start_expr).start;

        Ok(Expr::Range(RangeExpr {
            start: Some(Box::new(start_expr)),
            end: end_expr,
            inclusive: false,
            span: Span::new(start, end),
        }))
    }

    fn parse_logical_or(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_logical_and()?;

        while self.match_kind(&TokenKind::OrOr) {
            let right = self.parse_logical_and()?;
            expr = self.make_binary(expr, BinaryOperator::Or, right);
        }

        Ok(expr)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_equality()?;

        while self.match_kind(&TokenKind::AndAnd) {
            let right = self.parse_equality()?;
            expr = self.make_binary(expr, BinaryOperator::And, right);
        }

        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_comparison()?;

        loop {
            let operator = if self.match_kind(&TokenKind::EqualEqual) {
                BinaryOperator::Eq
            } else if self.match_kind(&TokenKind::NotEqual) {
                BinaryOperator::Ne
            } else {
                break;
            };

            let right = self.parse_comparison()?;
            expr = self.make_binary(expr, operator, right);
        }

        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_additive()?;

        loop {
            let operator = if self.match_kind(&TokenKind::Less) {
                BinaryOperator::Lt
            } else if self.match_kind(&TokenKind::LessEqual) {
                BinaryOperator::Le
            } else if self.match_kind(&TokenKind::Greater) {
                BinaryOperator::Gt
            } else if self.match_kind(&TokenKind::GreaterEqual) {
                BinaryOperator::Ge
            } else {
                break;
            };

            let right = self.parse_additive()?;
            expr = self.make_binary(expr, operator, right);
        }

        Ok(expr)
    }

    fn parse_additive(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_multiplicative()?;

        loop {
            let operator = if self.match_kind(&TokenKind::Plus) {
                BinaryOperator::Add
            } else if self.match_kind(&TokenKind::Minus) {
                BinaryOperator::Sub
            } else {
                break;
            };

            let right = self.parse_multiplicative()?;
            expr = self.make_binary(expr, operator, right);
        }

        Ok(expr)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_matrix()?;

        loop {
            let operator = if self.match_kind(&TokenKind::Star) {
                BinaryOperator::Mul
            } else if self.match_kind(&TokenKind::Slash) {
                BinaryOperator::Div
            } else if self.match_kind(&TokenKind::Percent) {
                BinaryOperator::Mod
            } else {
                break;
            };

            let right = self.parse_matrix()?;
            expr = self.make_binary(expr, operator, right);
        }

        Ok(expr)
    }

    fn parse_matrix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_unary()?;

        loop {
            let operator = if self.match_kind(&TokenKind::At) {
                BinaryOperator::MatrixMul
            } else if self.match_kind(&TokenKind::DotStar) {
                BinaryOperator::ElementWiseMul
            } else if self.match_kind(&TokenKind::DotSlash) {
                BinaryOperator::ElementWiseDiv
            } else {
                break;
            };

            let right = self.parse_unary()?;
            expr = self.make_binary(expr, operator, right);
        }

        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if self.match_kind(&TokenKind::Minus) {
            let operand = self.parse_unary()?;
            let span = expr_span(&operand);

            return Ok(Expr::Unary(UnaryExpr {
                operator: UnaryOperator::Neg,
                operand: Box::new(operand),
                span,
            }));
        }

        if self.match_kind(&TokenKind::Bang) {
            let operand = self.parse_unary()?;
            let span = expr_span(&operand);

            return Ok(Expr::Unary(UnaryExpr {
                operator: UnaryOperator::Not,
                operand: Box::new(operand),
                span,
            }));
        }

        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;

        loop {
            // Function call.
            if self.match_kind(&TokenKind::LeftParen) {
                let start = expr_span(&expr).start;
                let mut arguments = Vec::new();

                if !self.check(&TokenKind::RightParen) {
                    loop {
                        arguments.push(self.parse_expression()?);

                        if !self.match_kind(&TokenKind::Comma) {
                            break;
                        }
                    }
                }

                let end = self.consume(&TokenKind::RightParen, "expected ')' after arguments")?;

                expr = Expr::Call(CallExpr {
                    callee: Box::new(expr),
                    generic_args: Vec::new(),
                    arguments,
                    span: Span::new(start, end.span.end),
                });

                continue;
            }

            // Indexing.
            if self.match_kind(&TokenKind::LeftBracket) {
                let start = expr_span(&expr).start;
                let mut indices = Vec::new();

                if !self.check(&TokenKind::RightBracket) {
                    loop {
                        indices.push(self.parse_expression()?);

                        if !self.match_kind(&TokenKind::Comma) {
                            break;
                        }
                    }
                }

                let end = self.consume(
                    &TokenKind::RightBracket,
                    "expected ']' after index expression",
                )?;

                expr = Expr::Index(IndexExpr {
                    object: Box::new(expr),
                    indices,
                    span: Span::new(start, end.span.end),
                });

                continue;
            }

            // Member access.
            //
            // IMPORTANT:
            // If the next two tokens are `..`, this is a range and
            // must NOT be interpreted as member access.
            if self.check(&TokenKind::Dot) && !self.check_next(&TokenKind::Dot) {
                let start = expr_span(&expr).start;

                self.advance();

                let member_token = self.consume_identifier("expected member name after '.'")?;

                let member = match member_token.kind {
                    TokenKind::Identifier(member) => member,
                    _ => unreachable!(),
                };

                expr = Expr::Member(MemberExpr {
                    object: Box::new(expr),
                    member,
                    span: Span::new(start, member_token.span.end),
                });

                continue;
            }

            break;
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let token = self.advance();

        match token.kind {
            TokenKind::Integer(value) => Ok(Expr::Literal(LiteralExpr {
                value: LiteralValue::Integer(value),
                span: ast_span(token.span),
            })),

            TokenKind::Float(value) => Ok(Expr::Literal(LiteralExpr {
                value: LiteralValue::Float(value),
                span: ast_span(token.span),
            })),

            TokenKind::String(value) => Ok(Expr::Literal(LiteralExpr {
                value: LiteralValue::String(value),
                span: ast_span(token.span),
            })),

            TokenKind::Char(value) => Ok(Expr::Literal(LiteralExpr {
                value: LiteralValue::Char(value),
                span: ast_span(token.span),
            })),

            TokenKind::True => Ok(Expr::Literal(LiteralExpr {
                value: LiteralValue::Bool(true),
                span: ast_span(token.span),
            })),

            TokenKind::False => Ok(Expr::Literal(LiteralExpr {
                value: LiteralValue::Bool(false),
                span: ast_span(token.span),
            })),

            TokenKind::Identifier(name) => Ok(Expr::Identifier(IdentifierExpr {
                name,
                span: ast_span(token.span),
            })),

            TokenKind::If => self.parse_if_expression(token.span.start),

            TokenKind::LeftParen => {
                let first = self.parse_expression()?;
                let start = expr_span(&first).start;

                if self.match_kind(&TokenKind::Comma) {
                    let mut elements = vec![first];

                    loop {
                        elements.push(self.parse_expression()?);

                        if !self.match_kind(&TokenKind::Comma) {
                            break;
                        }
                    }

                    let end = self.consume(&TokenKind::RightParen, "expected ')' after tuple")?;

                    Ok(Expr::Tuple(TupleExpr {
                        elements,
                        span: Span::new(start, end.span.end),
                    }))
                } else {
                    self.consume(&TokenKind::RightParen, "expected ')' after expression")?;

                    Ok(first)
                }
            }

            TokenKind::LeftBracket => {
                let mut elements = Vec::new();

                if !self.check(&TokenKind::RightBracket) {
                    loop {
                        elements.push(self.parse_expression()?);

                        if !self.match_kind(&TokenKind::Comma) {
                            break;
                        }
                    }
                }

                let end =
                    self.consume(&TokenKind::RightBracket, "expected ']' after array literal")?;

                Ok(Expr::Array(ArrayExpr {
                    elements,
                    span: Span::new(token.span.start, end.span.end),
                }))
            }

            TokenKind::Eof => Err(ParseError {
                message: "unexpected end of file while parsing expression".to_string(),
                span: ast_span(token.span),
            }),

            _ => Err(ParseError {
                message: "expected expression".to_string(),
                span: ast_span(token.span),
            }),
        }
    }

    fn parse_if_expression(&mut self, start: usize) -> Result<Expr, ParseError> {
        let condition = self.parse_expression()?;
        let then_branch = self.parse_block()?;

        let else_branch = if self.match_kind(&TokenKind::Else) {
            if self.match_kind(&TokenKind::If) {
                let else_start = self.tokens[self.current - 1].span.start;

                Some(Box::new(self.parse_if_expression(else_start)?))
            } else {
                Some(Box::new(Expr::Block(self.parse_block()?)))
            }
        } else {
            None
        };

        let end = else_branch
            .as_ref()
            .map(|expr| expr_span(expr).end)
            .unwrap_or(then_branch.span.end);

        Ok(Expr::If(IfExpr {
            condition: Box::new(condition),
            then_branch,
            else_branch,
            span: Span::new(start, end),
        }))
    }

    // -------------------------------------------------------------------------
    // Parser helpers
    // -------------------------------------------------------------------------

    fn make_binary(&self, left: Expr, operator: BinaryOperator, right: Expr) -> Expr {
        let span = Span::new(expr_span(&left).start, expr_span(&right).end);

        Expr::Binary(BinaryExpr {
            left: Box::new(left),
            operator,
            right: Box::new(right),
            span,
        })
    }

    fn consume_identifier(&mut self, message: &str) -> Result<Token, ParseError> {
        if matches!(self.peek_kind(), TokenKind::Identifier(_)) {
            Ok(self.advance())
        } else {
            Err(self.error_here(message))
        }
    }

    fn consume(&mut self, expected: &TokenKind, message: &str) -> Result<Token, ParseError> {
        if self.check(expected) {
            Ok(self.advance())
        } else {
            Err(self.error_here(message))
        }
    }

    fn match_kind(&mut self, expected: &TokenKind) -> bool {
        if self.check(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check(&self, expected: &TokenKind) -> bool {
        std::mem::discriminant(self.peek_kind()) == std::mem::discriminant(expected)
    }

    fn check_next(&self, expected: &TokenKind) -> bool {
        if self.current + 1 >= self.tokens.len() {
            return false;
        }

        std::mem::discriminant(&self.tokens[self.current + 1].kind)
            == std::mem::discriminant(expected)
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.tokens[self.current].kind
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.current].clone();

        if self.current < self.tokens.len() - 1 {
            self.current += 1;
        }

        token
    }

    fn error_here(&self, message: &str) -> ParseError {
        ParseError {
            message: message.to_string(),
            span: ast_span(self.tokens[self.current].span),
        }
    }
}

fn ast_span(span: lexer::Span) -> Span {
    Span::new(span.start, span.end)
}

fn expr_span(expr: &Expr) -> Span {
    match expr {
        Expr::Literal(value) => value.span,
        Expr::Identifier(value) => value.span,
        Expr::Binary(value) => value.span,
        Expr::Assignment(value) => value.span,
        Expr::Unary(value) => value.span,
        Expr::Call(value) => value.span,
        Expr::Member(value) => value.span,
        Expr::Index(value) => value.span,
        Expr::Slice(value) => value.span,
        Expr::Array(value) => value.span,
        Expr::Tuple(value) => value.span,
        Expr::Block(value) => value.span,
        Expr::If(value) => value.span,
        Expr::Match(value) => value.span,
        Expr::Range(value) => value.span,
        Expr::Formula(value) => value.span,
        Expr::StructLiteral(value) => value.span,
        Expr::Cast(value) => value.span,
        Expr::Await(value) => value.span,
    }
}

fn flatten_addition(expr: Expr) -> Vec<Expr> {
    match expr {
        Expr::Binary(binary) if binary.operator == BinaryOperator::Add => {
            let mut values = flatten_addition(*binary.left);
            values.extend(flatten_addition(*binary.right));
            values
        }

        other => vec![other],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_function() {
        let source = r#"
            fn main() {
                let x: Int64 = 10;
                let y: Int64 = 20;
                let result = x + y;
                return result;
            }
        "#;

        let mut parser = Parser::from_source(source).expect("lexing should succeed");

        let program = parser.parse_program().expect("parsing should succeed");

        assert_eq!(program.declarations.len(), 1);

        match &program.declarations[0] {
            Decl::Function(function) => {
                assert_eq!(function.name, "main");
                assert_eq!(function.parameters.len(), 0);
                assert_eq!(function.body.statements.len(), 4);
            }

            _ => panic!("expected function declaration"),
        }
    }

    #[test]
    fn respects_operator_precedence() {
        let source = r#"
            fn main() {
                let x = 10 + 20 * 30;
            }
        "#;

        let mut parser = Parser::from_source(source).expect("lexing should succeed");

        let program = parser.parse_program().expect("parsing should succeed");

        let Decl::Function(function) = &program.declarations[0] else {
            panic!("expected function");
        };

        let Stmt::Let(statement) = &function.body.statements[0] else {
            panic!("expected let statement");
        };

        let Expr::Binary(addition) = statement.value.as_ref().unwrap() else {
            panic!("expected addition");
        };

        assert_eq!(addition.operator, BinaryOperator::Add);

        let Expr::Binary(multiplication) = addition.right.as_ref() else {
            panic!("expected multiplication");
        };

        assert_eq!(multiplication.operator, BinaryOperator::Mul);
    }

    #[test]
    fn parses_comparisons_and_logical_operators() {
        let source = r#"
            fn main() {
                let result = 10 < 20 && 30 >= 30;
            }
        "#;

        let mut parser = Parser::from_source(source).expect("lexing should succeed");

        parser
            .parse_program()
            .expect("comparison parsing should succeed");
    }

    #[test]
    fn parses_matrix_expression() {
        let source = r#"
            fn main() {
                let result = A @ B .* C ./ D;
            }
        "#;

        let mut parser = Parser::from_source(source).expect("lexing should succeed");

        parser
            .parse_program()
            .expect("matrix parsing should succeed");
    }

    #[test]
    fn parses_econometric_formula() {
        let source = r#"
            fn main() {
                let model =
                    GDP ~ Capital + Labor + Technology;
            }
        "#;

        let mut parser = Parser::from_source(source).expect("lexing should succeed");

        let program = parser
            .parse_program()
            .expect("formula parsing should succeed");

        let Decl::Function(function) = &program.declarations[0] else {
            panic!("expected function");
        };

        let Stmt::Let(statement) = &function.body.statements[0] else {
            panic!("expected let");
        };

        assert!(matches!(
            statement.value.as_ref().unwrap(),
            Expr::Formula(_)
        ));
    }

    #[test]
    fn parses_assignment() {
        let source = r#"
            fn main() {
                var x: Int64 = 10;
                x = x + 1;
                x += 2;
                x *= 3;
            }
        "#;

        let mut parser = Parser::from_source(source).expect("lexing should succeed");

        let program = parser
            .parse_program()
            .expect("assignment parsing should succeed");

        let Decl::Function(function) = &program.declarations[0] else {
            panic!("expected function");
        };

        assert_eq!(function.body.statements.len(), 4);

        assert!(matches!(
            function.body.statements[1],
            Stmt::Expr(ExprStmt {
                expr: Expr::Assignment(_),
                ..
            })
        ));
    }

    #[test]
    fn parses_for_loop() {
        let source = r#"
            fn main() {
                for i in 0..10 {
                    print(i);
                }
            }
        "#;

        let mut parser = Parser::from_source(source).expect("lexing should succeed");

        let program = parser
            .parse_program()
            .expect("for loop parsing should succeed");

        let Decl::Function(function) = &program.declarations[0] else {
            panic!("expected function");
        };

        let Stmt::For(for_stmt) = &function.body.statements[0] else {
            panic!("expected for statement");
        };

        assert!(matches!(for_stmt.iterable, Expr::Range(_)));
    }

    #[test]
    fn parses_parallel_for_loop() {
        let source = r#"
            fn main() {
                parallel for i in 0..10 {
                    print(i);
                }
            }
        "#;

        let mut parser = Parser::from_source(source).expect("lexing should succeed");

        parser
            .parse_program()
            .expect("parallel for parsing should succeed");
    }

    #[test]
    fn parses_function_call() {
        let source = r#"
            fn main() {
                print("Hello");
            }
        "#;

        let mut parser = Parser::from_source(source).expect("lexing should succeed");

        parser
            .parse_program()
            .expect("function call parsing should succeed");
    }

    #[test]
    fn parses_index_and_member_access() {
        let source = r#"
            fn main() {
                let x = data[0];
                let y = person.name;
            }
        "#;

        let mut parser = Parser::from_source(source).expect("lexing should succeed");

        parser
            .parse_program()
            .expect("index/member parsing should succeed");
    }

    #[test]
    fn parses_if_statement_without_semicolon() {
        let source = r#"
            fn main() {
                let x = 10;

                if x > 0 {
                    let y = x + 1;
                    return y;
                }
            }
        "#;

        let mut parser = Parser::from_source(source).expect("source should lex");
        let program = parser.parse_program().expect("source should parse");

        assert_eq!(program.declarations.len(), 1);
    }
}
