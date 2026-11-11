use scripturec::lexer::{Lexer, Token};

fn tokens(source: &str) -> Vec<Token<'_>> {
    Lexer::new(source).map(|result| result.unwrap().1).collect()
}

#[test]
fn metadata_keywords_are_reserved_tokens_but_quoted_text_is_literal() {
    for (word, token) in [
        ("_scripture_version_", Token::ScriptureVersion),
        ("_scripture_credits_", Token::ScriptureCredits),
    ] {
        let source = format!("  {word}");
        assert_eq!(
            Lexer::new(&source).next().unwrap().unwrap(),
            (2, token, source.len())
        );
        assert_eq!(tokens(&format!("\"{word}\"")), [Token::String(word)]);
        assert_eq!(token.to_string(), word);
    }
}

#[test]
fn compound_keywords_include_the_whole_phrase_in_their_span() {
    for (phrase, expected) in [
        ("let there be", Token::LetThereBe),
        ("make it so that", Token::MakeItSoThat),
        ("not equal", Token::NotEqual),
        ("time of our lord", Token::TimeOfOurLord),
    ] {
        let source = format!("  {phrase}(x)");
        let mut lexer = Lexer::new(&source);
        assert_eq!(
            lexer.next().unwrap().unwrap(),
            (2, expected, 2 + phrase.len())
        );
        assert_eq!(
            lexer.next().unwrap().unwrap(),
            (2 + phrase.len(), Token::LeftParen, 3 + phrase.len())
        );
    }
}

#[test]
fn failed_compound_matches_leave_every_word_available() {
    for source in [
        "not equality",
        "not equal2",
        "! equal",
        "not  equal",
        "not\tequal",
    ] {
        let expected_word = source.split_whitespace().last().unwrap();
        assert_eq!(
            tokens(source),
            vec![Token::Not, Token::Identifier(expected_word)]
        );
    }
    assert_eq!(
        tokens("let there be2"),
        vec![
            Token::Identifier("let"),
            Token::Identifier("there"),
            Token::Identifier("be2")
        ]
    );
    assert_eq!(
        tokens("make it so thatx"),
        vec![
            Token::Identifier("make"),
            Token::Identifier("it"),
            Token::Identifier("so"),
            Token::Identifier("thatx")
        ]
    );
    assert_eq!(
        tokens("(not\n equal)"),
        vec![
            Token::LeftParen,
            Token::Not,
            Token::Identifier("equal"),
            Token::RightParen
        ]
    );
    assert_eq!(
        tokens("let make there be it so that notable"),
        ["let", "make", "there", "be", "it", "so", "that", "notable"]
            .into_iter()
            .map(Token::Identifier)
            .collect::<Vec<_>>()
    );
}

#[test]
fn regex_string_errors_keep_spans_and_do_not_consume_the_next_line() {
    for newline in ["\n", "\r\n", "\r"] {
        let source = format!(" \"héllo{newline}42");
        let mut lexer = Lexer::new(&source);
        let error = lexer.next().unwrap().unwrap_err();
        assert_eq!(&source[error.span], "\"héllo");
        assert_eq!(
            error.message,
            "unterminated string; strings cannot contain newlines"
        );
        assert_eq!(lexer.next().unwrap().unwrap().1, Token::Separator);
        assert_eq!(lexer.next().unwrap().unwrap().1, Token::Number("42"));
        assert!(lexer.next().is_none());
    }
    for source in ["\"", "\"héllo"] {
        let error = Lexer::new(source).next().unwrap().unwrap_err();
        assert_eq!(error.span, 0..source.len());
        assert_eq!(error.message, "unterminated string");
    }
    assert_eq!(
        tokens(r#""" "héllo""world""#),
        vec![
            Token::String(""),
            Token::String("héllo"),
            Token::String("world")
        ]
    );
}

#[test]
fn unicode_errors_have_byte_spans_and_lexing_can_continue() {
    let source = "é 🦀 @ 1";
    let mut lexer = Lexer::new(source);
    for character in ['é', '🦀', '@'] {
        let error = lexer.next().unwrap().unwrap_err();
        assert_eq!(&source[error.span], character.to_string());
        assert_eq!(error.message, format!("unexpected character {character:?}"));
    }
    assert_eq!(lexer.next().unwrap().unwrap().1, Token::Number("1"));
    assert!(lexer.next().is_none());
}
