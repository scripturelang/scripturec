//! Source diagnostics shared by command-line and browser frontends.
use crate::{ParseError, ast::Span};
use std::{fmt, rc::Rc};

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub source: Rc<str>,
    pub span: Span,
    pub message: String,
}

impl Diagnostic {
    pub fn from_parse(source: &str, error: ParseError<'_>) -> Self {
        let (span, message) = match error {
            ParseError::InvalidToken { location } => (location..location, "invalid token".into()),
            ParseError::UnrecognizedEof { location, expected } => (
                location..location,
                format!("unexpected end of input; expected {}", expected.join(", ")),
            ),
            ParseError::UnrecognizedToken {
                token: (start, token, end),
                expected,
            } => (
                start..end,
                format!("unexpected {token}; expected {}", expected.join(", ")),
            ),
            ParseError::ExtraToken {
                token: (start, token, end),
            } => (start..end, format!("unexpected extra token {token}")),
            ParseError::User { error } => (error.span, error.message),
        };
        Self {
            source: Rc::from(
                &source[..source.floor_char_boundary(crate::MAX_SOURCE_BYTES.min(source.len()))],
            ),
            span,
            message,
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let position = self
            .source
            .floor_char_boundary(self.span.start.min(self.source.len()));
        let prefix = &self.source[..position];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count()
            + prefix
                .as_bytes()
                .iter()
                .enumerate()
                .filter(|(i, byte)| **byte == b'\r' && prefix.as_bytes().get(i + 1) != Some(&b'\n'))
                .count()
            + 1;
        let start = prefix.rfind(['\r', '\n']).map_or(0, |index| index + 1);
        let column = self.source[start..position].chars().count() + 1;
        let end = self.source[position..]
            .find(['\r', '\n'])
            .map_or(self.source.len(), |index| position + index);
        let excerpt_start = self.source[start..position]
            .char_indices()
            .rev()
            .nth(79)
            .map_or(start, |(index, _)| start + index);
        let excerpt_end = self.source[position..end]
            .char_indices()
            .nth(120)
            .map_or(end, |(index, _)| position + index);
        let left = if excerpt_start > start { "…" } else { "" };
        let right = if excerpt_end < end { "…" } else { "" };
        let message: String = self.message.chars().take(1024).collect();
        let suffix = if message.len() < self.message.len() {
            "…"
        } else {
            ""
        };
        writeln!(
            f,
            "line {line}, column {column}: {}{suffix}",
            terminal_text(&message)
        )?;
        writeln!(
            f,
            "  {left}{}{right}",
            terminal_text(&self.source[excerpt_start..excerpt_end])
        )?;
        let caret = left.chars().count()
            + terminal_text(&self.source[excerpt_start..position])
                .chars()
                .count();
        write!(f, "  {}^", " ".repeat(caret))
    }
}

impl std::error::Error for Diagnostic {}

/// Render untrusted diagnostic text without terminal commands or bidi overrides.
pub fn terminal_text(text: &str) -> String {
    let mut escaped = String::new();
    for character in text.chars() {
        if character.is_control()
            || matches!(character, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            escaped.extend(character.escape_default());
        } else {
            escaped.push(character);
        }
    }
    escaped
}
