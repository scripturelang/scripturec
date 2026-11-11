use scripturec::interpreter::{Interpreter, run};

fn output(source: &str) -> Vec<String> {
    let result = run(source);
    assert!(
        result.error.is_none(),
        "{}",
        result.error.unwrap_or_default()
    );
    result.output
}

#[test]
fn interpreter_metadata_is_available_in_scripts_functions_and_repl() {
    let expected = [
        env!("CARGO_PKG_VERSION"),
        "Scripture brought to you by jcjimenez",
    ];
    assert_eq!(
        output(include_str!("../examples/metadata.scripture")),
        expected
    );
    assert_eq!(
        output(
            "let there be version = _scripture_version_\ncommandment credits(): rejoice _scripture_credits_ amen\ngenesis: praise version; praise credits(); praise version == _scripture_version_ amen"
        ),
        [expected[0], expected[1], "true"]
    );
    let mut session = Interpreter::new();
    let result = session.execute_repl("_scripture_version_; _scripture_credits_");
    assert!(result.error.is_none(), "{result:?}");
    assert_eq!(result.output, expected.map(|value| format!("\"{value}\"")));
}

fn fails(source: &str, expected: &str) {
    let result = run(source);
    assert!(
        result
            .error
            .as_deref()
            .is_some_and(|error| error.contains(expected)),
        "{result:?}"
    );
}

#[test]
fn pray_waits_for_fractional_seconds_and_evaluates_once() {
    let start = std::time::Instant::now();
    assert_eq!(
        output(
            "let there be calls = 0\ncommandment delay(): calls = calls + 1; rejoice 0.02 amen\ngenesis: praise calls; pray delay(); praise calls; pray 0; pray -0 amen"
        ),
        ["0", "1"]
    );
    assert!(start.elapsed() >= std::time::Duration::from_millis(20));
}

#[test]
fn pray_preserves_function_and_prophecy_execution_order() {
    assert_eq!(
        output(
            "let there be x = 0\ncommandment pause(n): pray n; rejoice 7 amen\nprophecy x == 1: pray 0.001; praise x amen\ngenesis: praise pause(0.001); x = 1; praise 2 amen"
        ),
        ["7", "1", "2"]
    );
}

#[test]
fn pray_validates_durations_and_repl_recovers() {
    for (expr, message) in [
        ("-1", "non-negative seconds"),
        ("18446744073709551616", "supported duration range"),
        ("true", "expected a number"),
        ("\"0.01\"", "expected a number"),
        ("missing", "undefined name"),
        ("1 / 0", "division by zero"),
    ] {
        let mut session = Interpreter::new();
        let result = session.execute_repl(&format!("praise 1; pray {expr}; praise 2"));
        assert_eq!(result.output, ["1"]);
        let error = result.error.unwrap();
        assert!(error.contains(message), "{error}");
        assert!(error.contains("line 1"), "{error}");
        let recovered = session.execute_repl("pray (0); praise 3");
        assert!(recovered.error.is_none(), "{recovered:?}");
        assert_eq!(recovered.output, ["3"]);
    }
    fails(
        &format!("genesis: pray {} amen", "9".repeat(309)),
        "finite f64",
    );
}

#[test]
fn all_examples_execute_and_reactive_example_fires() {
    for entry in std::fs::read_dir("examples").unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "scripture") {
            output(&std::fs::read_to_string(path).unwrap());
        }
    }
    assert_eq!(
        output(include_str!("../examples/example02.scripture")),
        ["It finally happened!"]
    );
}

#[test]
fn finite_numbers_and_strict_types_follow_the_agreed_model() {
    assert_eq!(
        output(
            "genesis: praise 5 / 2; praise 1 == 1.0; praise 1 == true; praise 1 != \"1\"; praise -0 amen"
        ),
        ["2.5", "true", "false", "true", "0"]
    );
    for expression in [
        "1 + true",
        "\"1\" + 2",
        "\"a\" + \"b\"",
        "\"a\" < \"b\"",
        "not 1",
        "true and 1",
        "false or 1",
    ] {
        fails(
            &format!("genesis: praise {expression} amen"),
            "implicit type conversions",
        );
    }
    fails("genesis: if 1: amen amen", "expected a boolean");
    fails("genesis: praise 1 / -0 amen", "division by zero");
    let huge = "9".repeat(309);
    fails(&format!("genesis: praise {huge} amen"), "finite f64");
    let large = "9".repeat(200);
    fails(
        &format!("genesis: praise {large} * {large} amen"),
        "finite f64",
    );
}

#[test]
fn evaluation_order_and_short_circuiting_preserve_effects() {
    assert_eq!(
        output(
            "commandment report(x): praise x; rejoice x amen\ngenesis: praise report(1) + report(2) * report(3); praise false and report(true); praise true or report(false) amen"
        ),
        ["1", "2", "3", "7", "false", "true"]
    );
}

#[test]
fn functions_support_zero_arguments_forward_calls_recursion_and_false_fallthrough() {
    assert_eq!(
        output(
            "let there be x = answer()\ncommandment answer(): rejoice 42 amen\ncommandment fact(n): if n == 0: rejoice 1 amen; rejoice n * fact(n - 1) amen\ncommandment empty(): amen\ngenesis: praise x; praise fact(5); praise empty() amen"
        ),
        ["42", "120", "false"]
    );
    fails(
        "commandment one(x): amen\ngenesis: one() amen",
        "expects 1 arguments",
    );
    fails("commandment one(x, x): amen", "distinct names");
    fails("genesis: rejoice 1 amen", "only valid inside a commandment");
}

#[test]
fn scopes_are_lexical_and_local_declarations_can_shadow_globals() {
    assert_eq!(
        output(
            "let there be x = 1\ncommandment read(): rejoice x amen\ngenesis: let there be x = 2; if true: let there be x = 3; praise x amen; praise x; praise read(); x = true; praise x amen"
        ),
        ["3", "2", "1", "true"]
    );
    fails(
        "genesis: if true: let there be x = 1 amen; praise x amen",
        "undefined name",
    );
    fails(
        "genesis: let there be x = 1; let there be x = 2 amen",
        "already declared",
    );
    fails("genesis: x = 1 amen", "undefined name");
    fails("let there be x", "require an initializer");
    fails("make it so that x", "require an initializer");
    fails("make it so that x = 1\ngenesis: x = 1 amen", "constant");
    fails("let there be x, x = 1", "already declared");
}

#[test]
fn multiple_bindings_and_assignment_chains_evaluate_the_value_once() {
    assert_eq!(
        output(
            "commandment value(): praise \"called\"; rejoice 7 amen\nlet there be x, y = value()\ngenesis: praise x; praise y; x = y = value() amen"
        ),
        ["called", "7", "7", "called"]
    );
    let result = run(
        "make it so that x = 1\nlet there be y = 2\ncommandment value(): praise \"called\"; rejoice 7 amen\ngenesis: x = y = value() amen",
    );
    assert!(result.error.is_some());
    assert!(result.output.is_empty());
}

#[test]
fn file_lifecycle_rejects_multiple_entry_points_but_allows_definition_files() {
    assert!(output("commandment value(): rejoice 1 amen").is_empty());
    assert!(output("").is_empty());
    fails("genesis: amen\ngenesis: amen", "only one genesis");
    fails(
        "let there be first = second\nlet there be second = 1",
        "undefined name",
    );
}

#[test]
fn repl_state_persists_and_recovers_after_runtime_errors() {
    let mut session = Interpreter::new();
    assert!(session.execute_repl("let there be x = 1").error.is_none());
    assert_eq!(session.execute_repl("x = x + 1; x").output, ["2"]);
    assert!(
        session
            .execute_repl("commandment bad(): let there be x = 99; rejoice 1 / 0 amen")
            .error
            .is_none()
    );
    let result = session.execute_repl("bad()");
    let error = result.error.unwrap();
    assert!(error.contains("commandment bad()"), "{error}");
    assert_eq!(session.execute_repl("x").output, ["2"]);
    assert_eq!(session.execute_repl("\"hello\"").output, ["\"hello\""]);
}

#[test]
fn prophecies_fire_on_false_to_true_transitions_in_registration_order() {
    assert_eq!(
        output(
            "let there be x = 0\nprophecy x == 1: praise \"first\" amen\nprophecy x == 1: praise \"second\" amen\ngenesis: x = 1; x = 1; x = 0; x = 1 amen"
        ),
        ["first", "second", "first", "second"]
    );
    assert!(
        output("let there be x = 1\nprophecy x == 1: praise \"initial\" amen\ngenesis: x = 1 amen")
            .is_empty()
    );
}

#[test]
fn prophecy_handlers_are_lexical_and_can_trigger_other_handlers() {
    assert_eq!(
        output(
            "let there be x, y = 0\nprophecy x == 1: y = 1 amen\nprophecy y == 1: praise x amen\ncommandment change(): let there be y = 99; x = 1; praise y amen\ngenesis: change() amen"
        ),
        ["1", "99"]
    );
    fails(
        "commandment condition(): rejoice true amen\nprophecy condition(): amen",
        "side-effect-free",
    );
    fails(
        "let there be x = 0\nprophecy x == 1: rejoice 1 amen\ncommandment change(): x = 1 amen\ngenesis: change() amen",
        "only valid inside a commandment",
    );
}

#[test]
fn execution_limits_report_errors_instead_of_hanging() {
    fails(
        "commandment recurse(): rejoice recurse() amen\ngenesis: recurse() amen",
        "call depth limit",
    );
    fails(
        "let there be x = 0\nprophecy x == 1: x = 2 amen\nprophecy x == 2: x = 1 amen\ngenesis: x = 1 amen",
        "execution limit",
    );
    let text = "a".repeat(65_536);
    fails(&format!("genesis: praise \"{text}\" amen"), "output limit");
}

#[test]
fn errors_preserve_prior_output_and_report_original_shebang_line_numbers() {
    let result =
        run("#!/usr/bin/env scripturec\ngenesis:\npraise \"before\"\nconfess \"oops\"\namen\n");
    assert_eq!(result.output, ["before"]);
    let error = result.error.unwrap();
    assert!(error.contains("line 4, column 1"), "{error}");
    assert!(error.contains("uncaught sin: oops"));
    fails("genesis:\npraise 1 praise 2\namen", "line 2");
}
