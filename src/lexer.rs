//! Declarative token definitions, with a small adapter for compound keywords,
//! newline handling, and LALRPOP's spanned token interface.

use std::fmt;

use logos::Logos;

use crate::ast::Span;

#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq)]
#[logos(skip r"[ \t]+")]
#[logos(error = LexErrorKind)]
pub enum Token<'input> {
    #[token("genesis")]
    Genesis,
    #[token("commandment")]
    Commandment,
    #[token("rejoice")]
    Rejoice,
    #[token("preach")]
    Preach,
    #[token("prophecy")]
    Prophecy,
    #[token("amen")]
    Amen,
    #[token("pray")]
    Pray,
    #[token("praise")]
    Praise,
    #[token("sin")]
    Sin,
    #[token("confess")]
    Confess,
    #[token("absolve")]
    Absolve,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("_scripture_version_")]
    ScriptureVersion,
    #[token("_scripture_credits_")]
    ScriptureCredits,
    // Compound keywords are assembled by the adapter after Logos has matched
    // their first word. This preserves identifiers such as `let` and ensures
    // `not equality` remains Not + Identifier("equality").
    LetThereBe,
    MakeItSoThat,
    TimeOfOurLord,
    #[token("veritas", |_| true)]
    #[token("vero", |_| true)]
    #[token("verdadero", |_| true)]
    #[token("true", |_| true)]
    #[token("falsitas", |_| false)]
    #[token("falso", |_| false)]
    #[token("false", |_| false)]
    Boolean(bool),
    #[regex(r"[a-zA-Z][a-zA-Z0-9]*", |lex| lex.slice())]
    Identifier(&'input str),
    #[regex(r"[0-9]+(\.[0-9]+)?", |lex| lex.slice())]
    Number(&'input str),
    // Backslashes are literal; strings end at the next quote, never a newline.
    #[regex(r#""[^"\r\n]*""#, |lex| &lex.slice()[1..lex.slice().len() - 1])]
    // The complete string wins by longest match. This shorter pattern gives
    // unterminated strings a useful error span without manually scanning them.
    #[regex(r#""[^"\r\n]*"#, |_| Err(LexErrorKind::UnterminatedString))]
    String(&'input str),
    #[token(":")]
    Colon,
    #[token("(")]
    LeftParen,
    #[token(")")]
    RightParen,
    #[token(",")]
    Comma,
    #[token(";")]
    #[regex(r"\r\n|\r|\n")]
    Separator,
    #[token("=")]
    Assign,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("==")]
    Equal,
    #[token("!=")]
    NotEqual,
    #[token("<")]
    Less,
    #[token(">")]
    Greater,
    #[token("and")]
    And,
    #[token("or")]
    Or,
    #[token("!")]
    #[token("not")]
    Not,
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identifier(value) | Self::Number(value) => f.write_str(value),
            Self::String(value) => write!(f, "\"{value}\""),
            Self::Boolean(value) => write!(f, "{value}"),
            Self::Separator => f.write_str("newline or ';'"),
            Self::ScriptureVersion => f.write_str("_scripture_version_"),
            Self::ScriptureCredits => f.write_str("_scripture_credits_"),
            Self::TimeOfOurLord => f.write_str("time of our lord"),
            other => write!(f, "{other:?}"),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum LexErrorKind {
    #[default]
    UnexpectedCharacter,
    UnterminatedString,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub span: Span,
    pub message: String,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at bytes {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for LexError {}

pub struct Lexer<'input> {
    inner: logos::Lexer<'input, Token<'input>>,
    paren_depth: usize,
}

impl<'input> Lexer<'input> {
    pub fn new(source: &'input str) -> Self {
        let mut inner = Token::lexer(source);
        // Ignore a Unix shebang only at byte zero, retaining original offsets.
        if source.starts_with("#!") {
            inner.bump(source.find(['\r', '\n']).unwrap_or(source.len()));
        }
        Self {
            inner,
            paren_depth: 0,
        }
    }

    fn compound_keyword(&mut self, token: Token<'input>) -> Token<'input> {
        let (suffix, compound) = match token {
            Token::Identifier("let") => (" there be", Token::LetThereBe),
            Token::Identifier("make") => (" it so that", Token::MakeItSoThat),
            Token::Identifier("time") => (" of our lord", Token::TimeOfOurLord),
            Token::Not if self.inner.slice() == "not" => (" equal", Token::NotEqual),
            _ => return token,
        };
        // Inspect the raw source before whitespace is skipped: each interior
        // space must be exactly U+0020, even inside parentheses.
        if let Some(rest) = self.inner.remainder().strip_prefix(suffix)
            && !rest.starts_with(|c: char| c.is_ascii_alphanumeric())
        {
            self.inner.bump(suffix.len());
            compound
        } else {
            token
        }
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = Result<(usize, Token<'input>, usize), LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let token = match self.inner.next()? {
                Ok(token) => self.compound_keyword(token),
                Err(kind) => {
                    let message = match kind {
                        LexErrorKind::UnexpectedCharacter => {
                            let character = self.inner.slice().chars().next().unwrap();
                            format!("unexpected character {character:?}")
                        }
                        LexErrorKind::UnterminatedString => {
                            if self.inner.remainder().starts_with(['\r', '\n']) {
                                "unterminated string; strings cannot contain newlines".into()
                            } else {
                                "unterminated string".into()
                            }
                        }
                    };
                    return Some(Err(LexError {
                        span: self.inner.span(),
                        message,
                    }));
                }
            };
            match token {
                Token::LeftParen => self.paren_depth += 1,
                // Let the parser diagnose unmatched closing parentheses.
                Token::RightParen => self.paren_depth = self.paren_depth.saturating_sub(1),
                Token::Separator if self.paren_depth > 0 && self.inner.slice() != ";" => continue,
                _ => {}
            }
            let span = self.inner.span();
            return Some(Ok((span.start, token, span.end)));
        }
    }
}
