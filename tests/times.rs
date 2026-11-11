use scripturec::interpreter::{Interpreter, run};

fn output(source: &str) -> Vec<String> {
    let result = run(source);
    assert!(result.error.is_none(), "{result:?}");
    result.output
}

#[test]
fn times_normalize_offsets_and_retain_subsecond_precision() {
    for (input, expected) in [
        ("2026-12-24T18:00:00-06:00", "2026-12-25T00:00:00Z"),
        ("2026-01-01T00:15:00+01:00", "2025-12-31T23:15:00Z"),
        ("2000-02-29T12:00:00.120000000Z", "2000-02-29T12:00:00.12Z"),
        (
            "1969-12-31T23:59:59.999999999Z",
            "1969-12-31T23:59:59.999999999Z",
        ),
        ("0000-01-01T00:00:00Z", "0000-01-01T00:00:00Z"),
        (
            "9999-12-31T23:59:59.999999999Z",
            "9999-12-31T23:59:59.999999999Z",
        ),
    ] {
        assert_eq!(
            output(&format!(
                "genesis: praise time of our lord(\"{input}\") amen"
            )),
            [expected]
        );
    }
}

#[test]
fn comparisons_and_subtraction_use_instants_not_strings() {
    assert_eq!(
        output(
            r#"
        let there be first = time of our lord("2026-12-25T00:00:00.000000001Z")
        let there be same = time of our lord("2026-12-24T18:00:00.000000001-06:00")
        let there be later = time of our lord("2026-12-25T00:00:00.000000002Z")
        genesis:
            praise first == same
            praise first != same
            praise first < later
            praise later > first
            praise first < same
            praise first > same
            praise later - first
            praise first - later
            praise first - same
            praise first == "2026-12-25T00:00:00.000000001Z"
            praise first == 0
            praise first == false
            praise time of our lord("1970-01-01T00:00:00.25Z") - time of our lord("1969-12-31T23:59:59Z")
        amen
    "#
        ),
        [
            "true",
            "false",
            "true",
            "true",
            "false",
            "false",
            "0.000000001",
            "-0.000000001",
            "0",
            "false",
            "false",
            "false",
            "1.25"
        ]
    );
}

#[test]
fn invalid_timestamps_report_errors_and_repl_recovers() {
    for (stamp, message) in [
        ("", "valid RFC 3339"),
        ("2026-12-25", "valid RFC 3339"),
        ("2026-12-25T00:00:00", "timezone"),
        ("2026-02-29T00:00:00Z", "valid RFC 3339"),
        ("1900-02-29T00:00:00Z", "valid RFC 3339"),
        ("2026-04-31T00:00:00Z", "valid RFC 3339"),
        ("2026-13-01T00:00:00Z", "valid RFC 3339"),
        ("2026-12-25T24:00:00Z", "valid RFC 3339"),
        ("2026-12-25T00:00:00+24:00", "valid RFC 3339"),
        ("2026-12-25T00:00:00Zextra", "valid RFC 3339"),
        ("2016-12-31T23:59:60Z", "leap seconds"),
        ("2026-12-25T00:00:00.1234567890Z", "9 fractional"),
        ("0000-01-01T00:00:00+00:01", "UTC year range"),
        ("9999-12-31T23:59:59-00:01", "UTC year range"),
        ("é🦀", "valid RFC 3339"),
    ] {
        let mut session = Interpreter::new();
        let result = session.execute_repl(&format!("time of our lord(\"{stamp}\")"));
        assert!(
            result.error.as_deref().is_some_and(|e| e.contains(message)),
            "{stamp}: {result:?}"
        );
        let recovered = session.execute_repl("time of our lord(\"1970-01-01T00:00:00Z\")");
        assert!(recovered.error.is_none(), "{recovered:?}");
        assert_eq!(recovered.output, ["1970-01-01T00:00:00Z"]);
    }
}

#[test]
fn time_arguments_and_operations_are_strictly_typed() {
    for expression in [
        "time of our lord(1)",
        "time of our lord(true)",
        "time of our lord(time of our lord())",
        "time of our lord(1, 2)",
        "time of our lord() + 1",
        "1 + time of our lord()",
        "time of our lord() - 1",
        "1 - time of our lord()",
        "time of our lord() < 1",
        "1 > time of our lord()",
        "time of our lord() * time of our lord()",
        "time of our lord() / time of our lord()",
        "+time of our lord()",
        "-time of our lord()",
        "not time of our lord()",
        "time of our lord() and true",
    ] {
        let result = run(&format!("genesis: praise {expression} amen"));
        assert!(result.error.is_some(), "accepted {expression}");
    }
}

#[test]
fn timestamp_arguments_evaluate_once_and_times_pass_through_functions() {
    assert_eq!(
        output(
            r#"
        let there be calls = 0
        commandment stamp():
            calls = calls + 1
            rejoice "2026-12-25T00:00:00Z"
        amen
        commandment identity(value): rejoice value amen
        genesis:
            let there be first, second = time of our lord(stamp())
            first = second = identity(first)
            praise first
            praise second == first
            praise calls
        amen
    "#
        ),
        ["2026-12-25T00:00:00Z", "true", "1"]
    );
}

#[test]
fn now_captures_the_host_clock() {
    use std::time::{SystemTime, UNIX_EPOCH};
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i128;
    let result = output("genesis: praise time of our lord() amen");
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i128;
    let captured =
        time::OffsetDateTime::parse(&result[0], &time::format_description::well_known::Rfc3339)
            .unwrap()
            .unix_timestamp_nanos();
    assert!((before..=after).contains(&captured), "{result:?}");
}

#[test]
fn time_conditionals_handle_before_at_and_after_the_deadline() {
    // Assert runtime behavior without coupling it to the example's editable prose.
    let source = r#"genesis:
        let there be now = time of our lord()
        let there be deadline = time of our lord("2026-12-25T00:00:00Z")
        praise now < deadline
        if now < deadline:
            praise deadline - now
        else:
            praise now - deadline
        amen
    amen"#;
    for (stamp, expected) in [
        ("2026-12-24T23:00:00Z", ["true", "3600"]),
        ("2026-12-25T00:00:00Z", ["false", "0"]),
        ("2026-12-26T00:00:00Z", ["false", "86400"]),
    ] {
        let fixed_clock = source.replace(
            "time of our lord()",
            &format!("time of our lord(\"{stamp}\")"),
        );
        assert_eq!(output(&fixed_clock), expected);
        let example = include_str!("../examples/example11.scripture").replace(
            "time of our lord()",
            &format!("time of our lord(\"{stamp}\")"),
        );
        output(&example);
    }
}

#[test]
fn prophecies_watch_stored_times_but_do_not_read_the_clock() {
    assert_eq!(
        output(
            r#"
        let there be current = time of our lord("2026-12-24T23:00:00Z")
        make it so that deadline = time of our lord("2026-12-25T00:00:00Z")
        prophecy current == deadline: praise "The hour has come" amen
        genesis:
            current = time of our lord("2026-12-24T18:00:00-06:00")
            current = time of our lord("2026-12-25T00:00:00Z")
        amen
    "#
        ),
        ["The hour has come"]
    );
    for expression in [
        "time of our lord()",
        "time of our lord(\"2026-12-25T00:00:00Z\")",
    ] {
        let result = run(&format!("prophecy {expression} == {expression}: amen"));
        assert!(
            result
                .error
                .unwrap()
                .contains("constructors are not supported in prophecy conditions")
        );
    }
}
