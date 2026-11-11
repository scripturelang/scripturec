//! Syntax trees shared by the native and browser frontends.

use std::ops::Range;

/// Half-open UTF-8 byte offsets into the original source.
pub type Span = Range<usize>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    pub span: Span,
    pub value: T,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub declarations: Vec<Declaration>,
}

pub type Declaration = Spanned<DeclarationKind>;
pub type Statement = Spanned<StatementKind>;
pub type Expr = Spanned<ExprKind>;
pub type Block = Vec<Statement>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclarationKind {
    Variable {
        names: Vec<String>,
        initializer: Option<Expr>,
    },
    Constant {
        names: Vec<String>,
        initializer: Option<Expr>,
    },
    Genesis(Block),
    Commandment {
        name: String,
        parameters: Vec<String>,
        body: Block,
    },
    Prophecy {
        condition: Expr,
        body: Block,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementKind {
    Declaration(Declaration),
    Expression(Expr),
    /// Targets are stored in source order; execution will assign right to left.
    Assignment {
        targets: Vec<String>,
        value: Expr,
    },
    Return(Expr),
    Throw(Expr),
    Conditional {
        condition: Expr,
        body: Block,
    },
    Preach(Expr),
    Pray(Expr),
    Praise(Expr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprKind {
    /// Preserve the spelling for diagnostics and later finite-f64 validation.
    Number(String),
    String(String),
    Boolean(bool),
    Time(Vec<Expr>),
    Identifier(String),
    Call {
        name: String,
        arguments: Vec<Expr>,
    },
    Unary {
        operator: UnaryOperator,
        operand: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        operator: BinaryOperator,
        right: Box<Expr>,
    },
    Grouped(Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Positive,
    Negative,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    Less,
    Greater,
    And,
    Or,
}

impl Expr {
    /// Validate every expression reduction, before an arbitrarily deep tree exists.
    /// Iteration also covers long operator chains, which have no parentheses.
    pub(crate) fn bounded(self) -> Result<Self, crate::ParseError<'static>> {
        let mut pending = vec![(&self, 1)];
        while let Some((expr, depth)) = pending.pop() {
            if depth > 64 {
                return Err(crate::ParseError::User {
                    error: crate::lexer::LexError {
                        span: self.span.clone(),
                        message: "expression nesting limit reached (64 syntax nodes)".into(),
                    },
                });
            }
            match &expr.value {
                ExprKind::Unary { operand, .. } | ExprKind::Grouped(operand) => {
                    pending.push((operand, depth + 1))
                }
                ExprKind::Binary { left, right, .. } => {
                    pending.push((left, depth + 1));
                    pending.push((right, depth + 1));
                }
                ExprKind::Call { arguments, .. } | ExprKind::Time(arguments) => {
                    pending.extend(arguments.iter().map(|arg| (arg, depth + 1)))
                }
                _ => {}
            }
        }
        Ok(self)
    }

    pub(crate) fn binary(left: Self, operator: BinaryOperator, right: Self) -> Self {
        Self {
            span: left.span.start..right.span.end,
            value: ExprKind::Binary {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            },
        }
    }
}

/// Declarations and statements accepted at the REPL's outer level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplItem {
    Declaration(Declaration),
    Statement(Statement),
}
