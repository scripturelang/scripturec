use scripturec::{
    ParseError,
    ast::{
        BinaryOperator as B, DeclarationKind as D, Expr, ExprKind as E, ReplItem,
        StatementKind as S, UnaryOperator as U,
    },
    lexer::{Lexer, Token},
    parse_program, parse_repl,
};

fn expression(source: &str) -> Expr {
    let mut items = parse_repl(source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    assert_eq!(items.len(), 1);
    match items.remove(0) {
        ReplItem::Statement(statement) => match statement.value {
            S::Expression(expr) => expr,
            other => panic!("expected expression, got {other:?}"),
        },
        other => panic!("expected statement, got {other:?}"),
    }
}

#[test]
fn metadata_keywords_cannot_be_declared_assigned_or_called() {
    for word in ["_scripture_version_", "_scripture_credits_"] {
        for source in [
            format!("let there be {word} = 1"),
            format!("make it so that {word} = 1"),
            format!("{word} = \"replacement\""),
            format!("commandment {word}(): amen"),
            format!("commandment example({word}): amen"),
            format!("{word}()"),
        ] {
            assert!(parse_repl(&source).is_err(), "accepted {source:?}");
        }
        let parsed = expression(word);
        assert_eq!(parsed.span, 0..word.len());
        assert!(matches!(parsed.value, E::String(_)));
    }
}

fn binary(expr: &Expr, expected: B) -> (&Expr, &Expr) {
    let E::Binary {
        left,
        operator,
        right,
    } = &expr.value
    else {
        panic!("expected binary expression, got {expr:?}");
    };
    assert_eq!(*operator, expected);
    (left, right)
}

#[test]
fn every_example_parses_as_a_complete_program() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut count = 0;
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "scripture") {
            let source = std::fs::read_to_string(&path).unwrap();
            parse_program(&source).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            count += 1;
        }
    }
    assert!(count > 0, "no examples were found");
}

#[test]
fn newline_and_semicolon_separate_statements_and_declarations() {
    for separator in ["\n", "\r\n", "\r", ";", ";;\n;\n", " \t; \t\n"] {
        let source = format!(
            "{separator}let there be x = 1{separator}genesis:{separator}praise x{separator}x = 2{separator}amen{separator}"
        );
        let program = parse_program(&source).unwrap();
        assert_eq!(program.declarations.len(), 2);
        let D::Genesis(body) = &program.declarations[1].value else {
            panic!()
        };
        assert_eq!(body.len(), 2);
        assert!(matches!(body[0].value, S::Praise(_)));
        assert!(matches!(body[1].value, S::Assignment { .. }));
    }
}

#[test]
fn missing_separators_are_rejected_even_after_blocks() {
    for source in [
        "genesis: praise 1 praise 2 amen",
        "let there be x = 1 let there be y = 2",
        "genesis: amen genesis: amen",
        "genesis: if true: praise 1 amen praise 2 amen",
        "genesis: x = 1 y = 2 amen",
        "genesis: first(1) second(2) amen",
    ] {
        assert!(parse_program(source).is_err(), "accepted {source:?}");
    }
}

#[test]
fn separators_are_optional_before_amen_and_eof() {
    parse_program("genesis: praise 1; praise 2 amen").unwrap();
    parse_program("genesis: if true: praise 1 amen amen").unwrap();
    parse_program("let there be x = 1").unwrap();
    for source in ["", "\n;;\r\n\t", "genesis: amen", "genesis: ;\n; amen"] {
        parse_program(source).unwrap();
    }
}

#[test]
fn newlines_continue_expressions_only_inside_parentheses() {
    parse_program("commandment sum(\na,\nb\n): rejoice (a +\nb) amen\ngenesis: praise sum(\n1, (\n2 +\n3\n)\n) amen").unwrap();
    parse_program("genesis: praise (\r\n1 +\r\n2\r\n) amen").unwrap();
    for source in [
        "genesis: praise 1 +\n2 amen",
        "genesis: praise\n1 amen",
        "genesis: praise (1; 2) amen",
        "genesis: praise call(1; 2) amen",
        "genesis: praise (1 + 2\namen",
        "genesis: praise 1) amen",
    ] {
        assert!(parse_program(source).is_err(), "accepted {source:?}");
    }
}

#[test]
fn subtraction_is_distinct_from_two_statements() {
    assert_eq!(parse_repl("a - b").unwrap().len(), 1);
    for source in ["a\n-b", "a; -b"] {
        let items = parse_repl(source).unwrap();
        assert_eq!(items.len(), 2);
        let ReplItem::Statement(statement) = &items[1] else {
            panic!()
        };
        assert!(matches!(
            &statement.value,
            S::Expression(Expr {
                value: E::Unary {
                    operator: U::Negative,
                    ..
                },
                ..
            })
        ));
    }
}

#[test]
fn arithmetic_precedence_and_associativity_are_preserved() {
    let expr = expression("1 + 2 * 3");
    let (left, right) = binary(&expr, B::Add);
    assert_eq!(left.value, E::Number("1".into()));
    binary(right, B::Multiply);

    let expr = expression("10 - 3 - 2");
    let (left, right) = binary(&expr, B::Subtract);
    binary(left, B::Subtract);
    assert_eq!(right.value, E::Number("2".into()));

    let expr = expression("12 / 3 * 2");
    let (left, _) = binary(&expr, B::Multiply);
    binary(left, B::Divide);

    let expr = expression("(1 + 2) * -3");
    let (left, right) = binary(&expr, B::Multiply);
    assert!(matches!(left.value, E::Grouped(_)));
    assert!(matches!(
        right.value,
        E::Unary {
            operator: U::Negative,
            ..
        }
    ));
}

#[test]
fn boolean_precedence_and_nested_negation_are_preserved() {
    let expr = expression("not ready or active and enabled");
    let (left, right) = binary(&expr, B::Or);
    assert!(matches!(
        left.value,
        E::Unary {
            operator: U::Not,
            ..
        }
    ));
    binary(right, B::And);

    let expr = expression("not not ready");
    let E::Unary {
        operator: U::Not,
        operand,
    } = expr.value
    else {
        panic!()
    };
    assert!(matches!(
        operand.value,
        E::Unary {
            operator: U::Not,
            ..
        }
    ));

    let expr = expression("not x == y");
    let (left, _) = binary(&expr, B::Equal);
    assert!(matches!(
        left.value,
        E::Unary {
            operator: U::Not,
            ..
        }
    ));
}

#[test]
fn comparisons_cannot_chain_but_can_be_grouped_or_combined() {
    for first in ["==", "!=", "not equal", "<", ">"] {
        for second in ["==", "!=", "not equal", "<", ">"] {
            let source = format!("a {first} b {second} c");
            assert!(parse_repl(&source).is_err(), "accepted {source:?}");
        }
    }
    for source in [
        "a < b and b < c",
        "(a < b) == (c < d)",
        "a not equal b or ready",
    ] {
        expression(source);
    }
}

#[test]
fn assignment_chains_keep_target_order_and_one_value() {
    let items = parse_repl("first = second = third = produce(7)").unwrap();
    let ReplItem::Statement(statement) = &items[0] else {
        panic!()
    };
    let S::Assignment { targets, value } = &statement.value else {
        panic!()
    };
    assert_eq!(targets, &["first", "second", "third"]);
    assert!(
        matches!(&value.value, E::Call { name, arguments } if name == "produce" && arguments.len() == 1)
    );
    for source in ["(x = 1)", "praise x = 1", "call(x = 1)", "1 = x"] {
        assert!(parse_repl(source).is_err(), "accepted {source:?}");
    }
}

#[test]
fn calls_preserve_argument_order_and_preach_is_a_statement() {
    let expr = expression("combine(first(1), second(2))");
    let E::Call { name, arguments } = expr.value else {
        panic!()
    };
    assert_eq!(name, "combine");
    assert_eq!(arguments.len(), 2);
    assert!(matches!(&arguments[0].value, E::Call { name, .. } if name == "first"));
    assert!(matches!(&arguments[1].value, E::Call { name, .. } if name == "second"));
    parse_repl("preach combine(1, 2)").unwrap();
    assert!(parse_repl("praise preach combine(1, 2)").is_err());
}

#[test]
fn pray_accepts_an_expression_and_is_only_a_statement() {
    let items = parse_repl("pray delay(0.5) * 2").unwrap();
    let ReplItem::Statement(statement) = &items[0] else {
        panic!()
    };
    assert_eq!(statement.span, 0..19);
    let S::Pray(expr) = &statement.value else {
        panic!()
    };
    binary(expr, B::Multiply);
    parse_program("genesis: pray (0.5); pray 0 amen").unwrap();
    for source in [
        "pray",
        "pray()",
        "pray 1, 2",
        "praise pray 1",
        "let there be x = pray 1",
        "pray 1s",
    ] {
        assert!(parse_repl(source).is_err(), "accepted {source:?}");
    }
}

#[test]
fn keyword_boundaries_and_exact_multiword_spacing_are_enforced() {
    parse_program("let there be notable, trueValue, genesis2 = 1\nmake it so that ready = true")
        .unwrap();
    assert_eq!(expression("notable").value, E::Identifier("notable".into()));
    assert!(matches!(
        expression("not equality").value,
        E::Unary {
            operator: U::Not,
            ..
        }
    ));
    for source in [
        "let  there be x = 1",
        "let\tthere be x = 1",
        "let there\nbe x = 1",
        "let there bee = 1",
        "make it so thatx = 1",
        "make it  so that x = 1",
        "let there be genesis = 1",
    ] {
        assert!(parse_program(source).is_err(), "accepted {source:?}");
    }
    for source in ["x not  equal y", "x not\tequal y", "x not\nequal y"] {
        assert!(parse_repl(source).is_err(), "accepted {source:?}");
    }
    for source in ["x != y", "x not equal y"] {
        binary(&expression(source), B::NotEqual);
    }
}

#[test]
fn all_boolean_spellings_parse_and_numbers_keep_their_spelling() {
    for word in ["veritas", "vero", "verdadero", "true"] {
        assert_eq!(expression(word).value, E::Boolean(true));
    }
    for word in ["falsitas", "falso", "false"] {
        assert_eq!(expression(word).value, E::Boolean(false));
    }
    for number in [
        "0",
        "001",
        "123.45",
        "999999999999999999999999999999999999999999999",
    ] {
        assert_eq!(expression(number).value, E::Number(number.into()));
    }
    for invalid in [".5", "1.", "1.2.3", "1e3", "under_score"] {
        assert!(parse_repl(invalid).is_err());
    }
}

#[test]
fn strings_preserve_literal_content_and_do_not_change_parenthesis_depth() {
    let items = parse_repl("\"héllo; (amen)\"\n\"next\"").unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(expression(r#""a\nb""#).value, E::String(r"a\nb".into()));
    parse_repl("\"(\"\n1\n\")\"\n2").unwrap();
    for invalid in ["\"unfinished", "\"first\nsecond\"", "\"first\rsecond\""] {
        assert!(matches!(parse_repl(invalid), Err(ParseError::User { .. })));
    }
}

#[test]
fn spans_are_utf8_byte_offsets_and_errors_point_to_the_source() {
    let source = "praise \"é\";\npraise 2";
    let items = parse_repl(source).unwrap();
    let ReplItem::Statement(first) = &items[0] else {
        panic!()
    };
    assert_eq!(&source[first.span.clone()], "praise \"é\"");
    let S::Praise(value) = &first.value else {
        panic!()
    };
    assert_eq!(&source[value.span.clone()], "\"é\"");
    let ReplItem::Statement(second) = &items[1] else {
        panic!()
    };
    assert_eq!(&source[second.span.clone()], "praise 2");

    let invalid = "praise \"é\"; @";
    let Err(ParseError::User { error }) = parse_repl(invalid) else {
        panic!()
    };
    assert_eq!(&invalid[error.span], "@");

    let invalid = "genesis: praise 1 praise 2 amen";
    let Err(ParseError::UnrecognizedToken {
        token: (start, _, end),
        ..
    }) = parse_program(invalid)
    else {
        panic!()
    };
    assert_eq!(start, invalid.rfind("praise").unwrap());
    assert_eq!(&invalid[start..end], "praise");
}

#[test]
fn repl_and_file_entry_points_have_distinct_scopes() {
    let items = parse_repl("let there be x = 1; x = x + 1; praise x").unwrap();
    assert_eq!(items.len(), 3);
    assert!(matches!(items[0], ReplItem::Declaration(_)));
    assert!(matches!(items[1], ReplItem::Statement(_)));
    assert!(parse_program("praise 1").is_err());
    parse_program("genesis: let there be x = 1 amen").unwrap();
    assert!(parse_repl("").unwrap().is_empty());
    assert!(matches!(
        parse_repl("if true:\npraise 1"),
        Err(ParseError::UnrecognizedEof { .. })
    ));
}

#[test]
fn unspecified_constructs_are_not_silently_added() {
    for source in ["genesis: call(1,) amen", "genesis: absolve sin amen"] {
        assert!(parse_program(source).is_err(), "accepted {source:?}");
    }
}

#[test]
fn time_constructors_are_expressions_with_required_parentheses() {
    assert!(matches!(expression("time of our lord()").value, E::Time(args) if args.is_empty()));
    assert!(matches!(expression("time of our lord(\n stamp()\n)").value,
        E::Time(args) if matches!(&args[0].value, E::Call { name, .. } if name == "stamp")));
    binary(
        &expression("time of our lord() - time of our lord()"),
        B::Subtract,
    );
    parse_repl("let there be time = 1; time + 1").unwrap();
    for source in [
        "time of our lord",
        "time of our lord(1,)",
        "time  of our lord()",
        "time\tof our lord()",
        "time of\nour lord()",
        "time of our lords()",
        "let there be time of our lord = 1",
        "commandment time of our lord(): amen",
    ] {
        assert!(parse_repl(source).is_err(), "accepted {source:?}");
    }
    let nested = format!(
        "{}\"2026-12-25T00:00:00Z\"{}",
        "time of our lord(".repeat(70),
        ")".repeat(70)
    );
    assert!(
        parse_repl(&nested)
            .unwrap_err()
            .to_string()
            .contains("expression nesting limit")
    );
}

#[test]
fn lexer_tracks_nested_parentheses_without_swallowing_semicolons() {
    let tokens: Vec<_> = Lexer::new("(\n(1\n)\n;2\n)\n3")
        .map(|token| token.unwrap().1)
        .collect();
    assert_eq!(
        tokens,
        vec![
            Token::LeftParen,
            Token::LeftParen,
            Token::Number("1"),
            Token::RightParen,
            Token::Separator,
            Token::Number("2"),
            Token::RightParen,
            Token::Separator,
            Token::Number("3"),
        ]
    );
}
