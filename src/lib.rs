//! Scripture's shared parser and interpreter for native and browser frontends.

pub mod ast;
pub mod diagnostic;
pub mod interpreter;
pub mod lexer;
pub mod moment;
mod sleep;

/// Semantic version of the interpreter, shared by native and Wasm builds.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Credits exposed by the language's reserved metadata expression.
pub const CREDITS: &str = "Scripture brought to you by jcjimenez";
/// Maximum UTF-8 source size, enforced by readers and library entry points.
pub const MAX_SOURCE_BYTES: usize = 131_072;
pub const SOURCE_LIMIT_MESSAGE: &str = "source limit reached (128 KiB)";

lalrpop_util::lalrpop_mod!(grammar);

pub type ParseError<'input> =
    lalrpop_util::ParseError<usize, lexer::Token<'input>, lexer::LexError>;

/// Parse a complete file containing top-level declarations.
pub fn parse_program(source: &str) -> Result<ast::Program, ParseError<'_>> {
    check_input(source)?;
    grammar::ProgramParser::new().parse(lexer::Lexer::new(source))
}

/// Parse a REPL submission. Unexpected EOF tells the frontend to request more input.
pub fn parse_repl(source: &str) -> Result<Vec<ast::ReplItem>, ParseError<'_>> {
    check_input(source)?;
    grammar::ReplParser::new().parse(lexer::Lexer::new(source))
}

fn check_input(source: &str) -> Result<(), ParseError<'_>> {
    let message = if source.len() > MAX_SOURCE_BYTES {
        Some(SOURCE_LIMIT_MESSAGE)
    } else if lexer::Lexer::new(source).take(4097).count() > 4096 {
        Some("source limit reached (4096 tokens)")
    } else {
        None
    };
    match message {
        Some(message) => Err(ParseError::User {
            error: lexer::LexError {
                span: 0..0,
                message: message.into(),
            },
        }),
        None => {
            // Reject deep blocks before the parser can build recursively owned ASTs.
            let mut depth = 0usize;
            for token in lexer::Lexer::new(source) {
                let (start, token, end) = token.map_err(|error| ParseError::User { error })?;
                match token {
                    lexer::Token::Colon => depth += 1,
                    lexer::Token::Amen => depth = depth.saturating_sub(1),
                    _ => {}
                }
                if depth > 32 {
                    return Err(ParseError::User {
                        error: lexer::LexError {
                            span: start..end,
                            message: "block nesting limit reached (32 blocks)".into(),
                        },
                    });
                }
            }
            Ok(())
        }
    }
}

/// Run a complete script in a fresh interpreter and resolve to a JSON report.
/// The browser worker uses this same execution path as the native command.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn run_script(source: String) -> String {
    let result = interpreter::Interpreter::new()
        .execute_program_async(&source)
        .await;
    serde_json::to_string(&result).expect("execution reports contain only strings")
}
