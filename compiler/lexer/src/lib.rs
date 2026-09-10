#![forbid(unsafe_code)]

//! Lexer for the EconLang programming language.

/// A location span in the source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// A token produced by the lexer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// All token kinds currently supported by EconLang.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    // Keywords
    Fn,
    Let,
    Var,
    Const,
    In,
    Struct,
    Enum,
    Trait,
    Impl,
    If,
    Else,
    For,
    While,
    Match,
    Return,
    Break,
    Continue,
    Parallel,
    Async,
    Await,
    Import,
    Pub,
    Unsafe,

    // Literals
    Integer(String),
    Float(String),
    String(String),
    Char(char),
    True,
    False,

    // Identifiers
    Identifier(String),

    // Operators
    Plus,
    Minus,
    Arrow,
    Star,
    Slash,
    Percent,
    At,
    DotStar,
    DotSlash,
    Tilde,

    EqualEqual,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,

    AndAnd,
    OrOr,
    Bang,

    Equal,
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,

    // Punctuation
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Dot,
    Colon,
    ColonColon,
    Semicolon,

    // End of source
    Eof,
}

/// A lexical error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

/// Lexer state.
pub struct Lexer<'a> {
    source: &'a str,
    position: usize,
}

impl<'a> Lexer<'a> {
    /// Creates a new lexer for the given source code.
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
        }
    }

    /// Returns true when the lexer reached the end of the source.
    fn is_at_end(&self) -> bool {
        self.position >= self.source.len()
    }

    /// Returns the current byte, if one exists.
    fn current_byte(&self) -> Option<u8> {
        self.source.as_bytes().get(self.position).copied()
    }

    /// Advances one byte and returns it.
    fn advance(&mut self) -> Option<u8> {
        let byte = self.current_byte()?;
        self.position += 1;
        Some(byte)
    }

    /// Returns the current span.
    fn current_span(&self, start: usize) -> Span {
        Span {
            start,
            end: self.position,
        }
    }

    /// Tokenizes the entire source.
    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        while !self.is_at_end() {
            self.skip_whitespace_and_comments();

            if self.is_at_end() {
                break;
            }

            tokens.push(self.next_token()?);
        }

        let end = self.position;

        tokens.push(Token {
            kind: TokenKind::Eof,
            span: Span { start: end, end },
        });

        Ok(tokens)
    }

    /// Produces one token from the current position.
    fn next_token(&mut self) -> Result<Token, LexError> {
        let start = self.position;
        let byte = self.advance().expect("lexer is not at EOF");

        let kind = match byte {
            b'(' => TokenKind::LeftParen,
            b')' => TokenKind::RightParen,
            b'{' => TokenKind::LeftBrace,
            b'}' => TokenKind::RightBrace,
            b'[' => TokenKind::LeftBracket,
            b']' => TokenKind::RightBracket,
            b',' => TokenKind::Comma,
            b';' => TokenKind::Semicolon,

            b':' => {
                if self.match_byte(b':') {
                    TokenKind::ColonColon
                } else {
                    TokenKind::Colon
                }
            }

            b'+' => {
                if self.match_byte(b'=') {
                    TokenKind::PlusEqual
                } else {
                    TokenKind::Plus
                }
            }

            b'-' => {
                if self.match_byte(b'=') {
                    TokenKind::MinusEqual
                } else if self.match_byte(b'>') {
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }

            b'*' => {
                if self.match_byte(b'=') {
                    TokenKind::StarEqual
                } else {
                    TokenKind::Star
                }
            }

            b'/' => {
                if self.match_byte(b'=') {
                    TokenKind::SlashEqual
                } else {
                    TokenKind::Slash
                }
            }

            b'%' => TokenKind::Percent,
            b'@' => TokenKind::At,
            b'~' => TokenKind::Tilde,

            b'!' => {
                if self.match_byte(b'=') {
                    TokenKind::NotEqual
                } else {
                    TokenKind::Bang
                }
            }

            b'=' => {
                if self.match_byte(b'=') {
                    TokenKind::EqualEqual
                } else {
                    TokenKind::Equal
                }
            }

            b'<' => {
                if self.match_byte(b'=') {
                    TokenKind::LessEqual
                } else {
                    TokenKind::Less
                }
            }

            b'>' => {
                if self.match_byte(b'=') {
                    TokenKind::GreaterEqual
                } else {
                    TokenKind::Greater
                }
            }

            b'&' => {
                if self.match_byte(b'&') {
                    TokenKind::AndAnd
                } else {
                    return Err(LexError {
                        message: "expected '&' after '&'".to_string(),
                        span: self.current_span(start),
                    });
                }
            }

            b'|' => {
                if self.match_byte(b'|') {
                    TokenKind::OrOr
                } else {
                    return Err(LexError {
                        message: "expected '|' after '|'".to_string(),
                        span: self.current_span(start),
                    });
                }
            }

            b'.' => {
                if self.match_byte(b'*') {
                    TokenKind::DotStar
                } else if self.match_byte(b'/') {
                    TokenKind::DotSlash
                } else {
                    TokenKind::Dot
                }
            }

            b'"' => self.read_string(start)?,

            b'\'' => self.read_char(start)?,

            b'0'..=b'9' => self.read_number(start),

            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.read_identifier(start),

            _ => {
                return Err(LexError {
                    message: format!("unexpected character '{}'", byte as char),
                    span: self.current_span(start),
                });
            }
        };

        Ok(Token {
            kind,
            span: self.current_span(start),
        })
    }

    /// Matches and consumes a specific byte.
    fn match_byte(&mut self, expected: u8) -> bool {
        if self.current_byte() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    /// Reads an identifier or keyword.
    fn read_identifier(&mut self, start: usize) -> TokenKind {
        while let Some(byte) = self.current_byte() {
            if byte.is_ascii_alphanumeric() || byte == b'_' {
                self.position += 1;
            } else {
                break;
            }
        }

        let text = &self.source[start..self.position];

        match text {
            "fn" => TokenKind::Fn,
            "let" => TokenKind::Let,
            "var" => TokenKind::Var,
            "const" => TokenKind::Const,
            "in" => TokenKind::In,
            "struct" => TokenKind::Struct,
            "enum" => TokenKind::Enum,
            "trait" => TokenKind::Trait,
            "impl" => TokenKind::Impl,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "for" => TokenKind::For,
            "while" => TokenKind::While,
            "match" => TokenKind::Match,
            "return" => TokenKind::Return,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "parallel" => TokenKind::Parallel,
            "async" => TokenKind::Async,
            "await" => TokenKind::Await,
            "import" => TokenKind::Import,
            "pub" => TokenKind::Pub,
            "unsafe" => TokenKind::Unsafe,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            _ => TokenKind::Identifier(text.to_string()),
        }
    }

    /// Reads an integer or floating-point literal.
    fn read_number(&mut self, start: usize) -> TokenKind {
        while matches!(self.current_byte(), Some(b'0'..=b'9')) {
            self.position += 1;
        }

        let mut is_float = false;

        if self.current_byte() == Some(b'.')
            && self
                .source
                .as_bytes()
                .get(self.position + 1)
                .is_some_and(u8::is_ascii_digit)
        {
            is_float = true;
            self.position += 1;

            while matches!(self.current_byte(), Some(b'0'..=b'9')) {
                self.position += 1;
            }
        }

        let text = self.source[start..self.position].to_string();

        if is_float {
            TokenKind::Float(text)
        } else {
            TokenKind::Integer(text)
        }
    }

    /// Reads a string literal.
    fn read_string(&mut self, start: usize) -> Result<TokenKind, LexError> {
        let mut value = String::new();

        while let Some(byte) = self.advance() {
            match byte {
                b'"' => return Ok(TokenKind::String(value)),
                b'\\' => {
                    let escaped = self.advance().ok_or_else(|| LexError {
                        message: "unterminated string escape".to_string(),
                        span: self.current_span(start),
                    })?;

                    let character = match escaped {
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'\\' => '\\',
                        b'"' => '"',
                        _ => {
                            return Err(LexError {
                                message: "unknown string escape".to_string(),
                                span: self.current_span(start),
                            });
                        }
                    };

                    value.push(character);
                }
                _ => value.push(byte as char),
            }
        }

        Err(LexError {
            message: "unterminated string literal".to_string(),
            span: self.current_span(start),
        })
    }

    /// Reads a character literal.
    fn read_char(&mut self, start: usize) -> Result<TokenKind, LexError> {
        let byte = self.advance().ok_or_else(|| LexError {
            message: "unterminated character literal".to_string(),
            span: self.current_span(start),
        })?;

        let character = if byte == b'\\' {
            let escaped = self.advance().ok_or_else(|| LexError {
                message: "unterminated character escape".to_string(),
                span: self.current_span(start),
            })?;

            match escaped {
                b'n' => '\n',
                b'r' => '\r',
                b't' => '\t',
                b'\\' => '\\',
                b'\'' => '\'',
                _ => {
                    return Err(LexError {
                        message: "unknown character escape".to_string(),
                        span: self.current_span(start),
                    });
                }
            }
        } else {
            byte as char
        };

        match self.advance() {
            Some(b'\'') => Ok(TokenKind::Char(character)),
            _ => Err(LexError {
                message: "unterminated character literal".to_string(),
                span: self.current_span(start),
            }),
        }
    }

    /// Skips spaces, line comments, and block comments.
    fn skip_whitespace_and_comments(&mut self) {
        loop {
            while matches!(self.current_byte(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
                self.position += 1;
            }

            if self.source[self.position..].starts_with("//") {
                while let Some(byte) = self.advance() {
                    if byte == b'\n' {
                        break;
                    }
                }

                continue;
            }

            if self.source[self.position..].starts_with("/*") {
                self.position += 2;

                while !self.is_at_end() && !self.source[self.position..].starts_with("*/") {
                    self.position += 1;
                }

                if self.source[self.position..].starts_with("*/") {
                    self.position += 2;
                }

                continue;
            }

            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_simple_function() {
        let source = r#"
            fn main() {
                let x: Int64 = 10;
            }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().expect("lexing should succeed");

        assert!(tokens.iter().any(|token| token.kind == TokenKind::Fn));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Let));

        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::Integer("10".to_string()))
        );
    }

    #[test]
    fn lex_economic_formula() {
        let source = "GDP ~ Capital + Labor";

        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().expect("lexing should succeed");

        assert_eq!(tokens[0].kind, TokenKind::Identifier("GDP".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Tilde);
        assert_eq!(tokens[2].kind, TokenKind::Identifier("Capital".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Plus);
        assert_eq!(tokens[4].kind, TokenKind::Identifier("Labor".to_string()));
    }

    #[test]
    fn lex_matrix_operators() {
        let source = "A @ B .* C ./ D";

        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().expect("lexing should succeed");

        assert_eq!(tokens[1].kind, TokenKind::At);
        assert_eq!(tokens[3].kind, TokenKind::DotStar);
        assert_eq!(tokens[5].kind, TokenKind::DotSlash);
    }

    #[test]
    fn lex_comments() {
        let source = r#"
            // comment
            let x = 10;
            /* another comment */
            let y = 20;
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().expect("lexing should succeed");

        assert!(tokens.iter().any(|token| token.kind == TokenKind::Let));
        assert_eq!(
            tokens.last().map(|token| &token.kind),
            Some(&TokenKind::Eof)
        );
    }

    #[test]
    fn lex_for_keyword() {
        let source = "for i in 0..10";

        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().expect("lexing should succeed");

        assert_eq!(tokens[0].kind, TokenKind::For);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("i".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::In);
    }
}
