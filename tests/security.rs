use scripturec::{
    MAX_SOURCE_BYTES,
    diagnostic::Diagnostic,
    interpreter::{Interpreter, run},
    parse_program, parse_repl,
};

fn names(prefix: &str, count: usize) -> String {
    (0..count)
        .map(|i| format!("{prefix}{i}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn expect_error(source: &str, message: &str) {
    let result = run(source);
    assert!(
        result
            .error
            .as_deref()
            .is_some_and(|error| error.contains(message)),
        "{result:?}"
    );
}

#[test]
fn syntax_depth_is_bounded_before_evaluation_cloning_and_drop() {
    let expressions = [
        format!("{}true", "not ".repeat(4000)),
        format!("{}1", "1+".repeat(1900)),
        format!("{}1{}", "(".repeat(1900), ")".repeat(1900)),
        format!("{}1{}", "f(".repeat(1000), ")".repeat(1000)),
    ];
    for expression in expressions {
        for source in [
            format!("genesis: if false: praise {expression} amen amen"),
            format!("commandment f(): rejoice {expression} amen"),
            format!("prophecy {expression}: amen"),
        ] {
            let error = parse_program(&source).unwrap_err();
            assert!(
                Diagnostic::from_parse(&source, error)
                    .message
                    .contains("expression nesting limit")
            );
        }
        assert!(parse_repl(&expression).is_err());
    }
    let source = format!(
        "commandment f(): {}f() {}\ngenesis: f() amen",
        "if true: ".repeat(450),
        "amen ".repeat(451)
    );
    expect_error(&source, "block nesting limit");
    assert!(parse_program(&source).is_err());
    assert!(parse_repl(&source).is_err());
    assert!(parse_program(&format!("genesis: praise {}1 amen", "+".repeat(63))).is_ok());
}

#[test]
fn else_branches_obey_the_same_block_depth_limit() {
    for branch in ["if true: ", "if false: else: "] {
        let allowed = format!(
            "genesis: {}praise 1 {}amen",
            branch.repeat(31),
            "amen ".repeat(31)
        );
        assert_eq!(run(&allowed).output, ["1"]);
        let too_deep = format!(
            "genesis: {}praise 1 {}amen",
            branch.repeat(32),
            "amen ".repeat(32)
        );
        expect_error(&too_deep, "block nesting limit");
    }
    // An else is a sibling branch, not another nested block.
    let siblings = format!(
        "genesis: {}praise 1 {}amen",
        "if true: ".repeat(31),
        "else: praise 2 amen ".repeat(31)
    );
    let result = run(&siblings);
    assert!(result.error.is_none(), "{result:?}");
    assert_eq!(result.output, ["1"]);
}

#[test]
fn combined_runtime_nesting_unwinds_and_preserves_the_session() {
    let mut session = Interpreter::new();
    let definition = format!(
        "commandment f(): {}f() {}",
        "if true: ".repeat(31),
        "amen ".repeat(32)
    );
    assert!(session.execute_repl(&definition).error.is_none());
    for _ in 0..3 {
        let result = session.execute_repl("f()");
        assert!(result.error.unwrap().contains("execution nesting limit"));
        assert_eq!(session.execute_repl("2 + 2").output, ["4"]);
    }
}

#[test]
fn string_fanout_is_rejected_before_creating_bindings() {
    let mut session = Interpreter::new();
    let source = format!(
        "let there be {} = \"{}\"",
        names("v", 1000),
        "x".repeat(100_000)
    );
    assert!(source.len() < MAX_SOURCE_BYTES);
    assert!(
        session
            .execute_repl(&source)
            .error
            .unwrap()
            .contains("storage limit")
    );
    // Failed bulk declarations must not leave partial bindings or quota reservations.
    let result = session.execute_repl("let there be v0 = 7; v0");
    assert!(result.error.is_none(), "{result:?}");
    assert_eq!(result.output, ["7"]);
}

#[test]
fn stored_values_are_bounded_across_submissions_and_assignment_reclaims_space() {
    let mut session = Interpreter::new();
    let mut rejected = None;
    for index in 0..100 {
        let source = format!("let there be v{index} = \"{}\"", "x".repeat(100_000));
        if let Some(error) = session.execute_repl(&source).error {
            assert!(error.contains("storage limit"), "{error}");
            rejected = Some(index);
            break;
        }
    }
    let index = rejected.expect("persistent storage must be bounded");
    assert!(index > 0);
    assert!(session.execute_repl("v0 = 0").error.is_none());
    let source = format!("let there be v{index} = \"{}\"", "x".repeat(100_000));
    assert!(session.execute_repl(&source).error.is_none());
    assert_eq!(session.execute_repl("v0").output, ["0"]);
}

#[test]
fn scopes_and_partially_evaluated_arguments_release_their_budgets() {
    let mut session = Interpreter::new();
    let locals = format!(
        "genesis: let there be {} = \"{}\" amen",
        names("v", 500),
        "x".repeat(10_000)
    );
    for _ in 0..4 {
        let result = session.execute_repl(&locals);
        assert!(result.error.is_none(), "{result:?}");
    }
    assert!(
        session
            .execute_repl("commandment f(a,b,c,d,e): rejoice true amen")
            .error
            .is_none()
    );
    let globals = format!(
        "let there be {} = \"{}\"",
        names("v", 80),
        "x".repeat(100_000)
    );
    assert!(session.execute_repl(&globals).error.is_none());
    for _ in 0..4 {
        assert!(
            session
                .execute_repl("f(v0,v0,v0,v0,v0)")
                .error
                .unwrap()
                .contains("storage limit")
        );
        assert!(
            session
                .execute_repl("f(v0,missing,0,0,0)")
                .error
                .unwrap()
                .contains("undefined name")
        );
        let result = session.execute_repl("f(0,0,0,0,0)");
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(result.output, ["true"]);
    }
    // Count repeated assignment targets once, and release the old string charge.
    assert!(session.execute_repl("v0 = v0 = 0").error.is_none());
    assert!(session.execute_repl("v0 = v0 = v1").error.is_none());
}

#[test]
fn bindings_and_retained_definitions_have_session_limits() {
    let mut session = Interpreter::new();
    for batch in 0..8 {
        let source = format!("let there be {} = 0", names(&format!("b{batch}v"), 512));
        assert!(session.execute_repl(&source).error.is_none());
    }
    assert!(
        session
            .execute_repl("let there be extra = 0")
            .error
            .unwrap()
            .contains("binding limit")
    );
    let mut session = Interpreter::new();
    for index in 0..256 {
        assert!(
            session
                .execute_repl(&format!("commandment f{index}(): amen"))
                .error
                .is_none()
        );
    }
    assert!(
        session
            .execute_repl("prophecy false: amen")
            .error
            .unwrap()
            .contains("definition limit")
    );
    let mut session = Interpreter::new();
    let mut rejected = false;
    for index in 0..40 {
        let source = format!(
            "commandment f{index}(): rejoice \"{}\" amen",
            "x".repeat(100_000)
        );
        if let Some(error) = session.execute_repl(&source).error {
            assert!(error.contains("storage limit"), "{error}");
            rejected = true;
            break;
        }
    }
    assert!(rejected, "retained source and syntax must be budgeted");
    assert_eq!(session.execute_repl("f0() == f0()").output, ["true"]);
}

#[test]
fn growing_event_queues_are_bounded_and_cleared_on_error() {
    let mut session = Interpreter::new();
    let source = format!(
        "let there be x = 0\n{}",
        "prophecy x == 1: x = 0; x = 1 amen\n".repeat(32)
    );
    assert!(session.execute_repl(&source).error.is_none());
    assert!(
        session
            .execute_repl("x = 1")
            .error
            .unwrap()
            .contains("pending event limit")
    );
    assert_eq!(session.execute_repl("2 + 2").output, ["4"]);
}

#[test]
fn oversized_library_input_and_diagnostics_are_bounded() {
    let source = "é".repeat(MAX_SOURCE_BYTES);
    expect_error(&source, "source limit");
    let diagnostic = Diagnostic::from_parse(&source, parse_program(&source).unwrap_err());
    assert!(diagnostic.source.len() <= MAX_SOURCE_BYTES);
    assert!(diagnostic.to_string().len() < 4096);
    let source = format!("genesis: {}praise missing amen", " ".repeat(100_000));
    let result = run(&source);
    assert!(result.error.unwrap().len() < 4096);
}

#[test]
fn diagnostic_messages_and_excerpts_escape_terminal_controls() {
    for source in [
        "genesis: \u{1b}[31mBAD amen",
        "genesis: confess \"\u{1b}]52;c;SGVsbG8=\u{7}\u{202e}\" amen",
        "genesis: praise \"x\" \"\u{1b}[2J\" amen",
    ] {
        let error = run(source).error.unwrap();
        assert!(
            !error.contains(['\u{1b}', '\u{7}', '\u{202e}']),
            "{error:?}"
        );
        assert!(error.contains("\\u{1b}"), "{error:?}");
    }
}

#[test]
fn cli_rejects_oversized_streams_without_waiting_for_eof() {
    use std::{
        io::Write,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    for args in [vec!["-"], vec!["--check", "-"], vec![]] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_scripturec"))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        input.write_all(&vec![b' '; MAX_SOURCE_BYTES + 1]).unwrap();
        input.flush().unwrap();
        // Keep the writer open: a read-to-EOF implementation must fail this test.
        let deadline = Instant::now() + Duration::from_secs(5);
        let exited = loop {
            if child.try_wait().unwrap().is_some() {
                break true;
            }
            if Instant::now() >= deadline {
                break false;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        if !exited {
            child.kill().unwrap();
        }
        drop(input);
        let output = child.wait_with_output().unwrap();
        assert!(exited, "input limit waited for EOF");
        assert!(!output.status.success());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("source limit")
        );
    }
}
