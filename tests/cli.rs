use std::process::Command;

#[test]
fn check_command_parses_without_executing() {
    let output = Command::new(env!("CARGO_BIN_EXE_scripturec"))
        .args(["--check", "examples/example01.scripture"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Parsed examples/example01.scripture"));
    assert!(!stdout.contains("Hallelujah"));
}

#[test]
fn check_command_fails_for_invalid_syntax_missing_files_and_bad_arguments() {
    for args in [
        vec!["--check", "tests/fixtures/missing_separator.scripture"],
        vec!["--check", "tests/fixtures/nonexistent.scripture"],
        vec!["--check"],
        vec!["--check", "examples/example01.scripture", "extra"],
        vec!["--unknown"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_scripturec"))
            .args(&args)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {args:?}");
        assert!(!output.stderr.is_empty());
        assert!(output.stdout.is_empty());
    }
}

fn piped(args: &[&str], source: &str) -> std::process::Output {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(env!("CARGO_BIN_EXE_scripturec"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn no_arguments_start_a_persistent_multiline_repl() {
    let result = piped(
        &[],
        "let there be x = 2\ncommandment twice(n):\nrejoice n * 2\namen\ntwice(x)\nx = 3\nx\n:quit\n",
    );
    assert!(result.status.success(), "{:?}", result.stderr);
    assert_eq!(String::from_utf8(result.stdout).unwrap(), "4\n3\n");
}

#[test]
fn file_and_stdin_modes_execute_scripts_and_fail_on_errors() {
    let result = Command::new(env!("CARGO_BIN_EXE_scripturec"))
        .arg("examples/example01.scripture")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(String::from_utf8(result.stdout).unwrap(), "Hallelujah\n");
    let result = piped(
        &["-"],
        "#!/usr/bin/env scripturec\ngenesis: praise 5 / 2 amen",
    );
    assert!(result.status.success());
    assert_eq!(String::from_utf8(result.stdout).unwrap(), "2.5\n");
    let result = piped(&["-"], "genesis: praise 1 / 0 amen");
    assert!(!result.status.success());
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("division by zero")
    );
}

#[test]
fn repl_recovers_from_errors_and_reports_incomplete_eof() {
    let result = piped(&[], "1 / 0\n2 + 2\n");
    assert!(!result.status.success());
    assert_eq!(String::from_utf8(result.stdout).unwrap(), "4\n");
    let result = piped(&[], "genesis:\npraise 1\n");
    assert!(!result.status.success());
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("unexpected end of input")
    );
    let result = piped(&[], "if true:\n:cancel\n2\n:reset\n3\n");
    assert!(result.status.success());
    assert_eq!(String::from_utf8(result.stdout).unwrap(), "2\n3\n");
}

#[test]
fn pray_runs_in_scripts_and_repl_and_check_does_not_sleep() {
    for (args, source) in [
        (vec!["-"], "genesis: pray 0.001; praise 42 amen"),
        (vec![], "pray 0.001\npraise 42\n"),
    ] {
        let result = piped(&args, source);
        assert!(result.status.success(), "{:?}", result.stderr);
        assert_eq!(String::from_utf8(result.stdout).unwrap(), "42\n");
    }
    let result = piped(&["--check", "-"], "genesis: pray -1 amen");
    assert!(result.status.success(), "{:?}", result.stderr);
    let result = piped(&["-"], "genesis: pray -1 amen");
    assert!(!result.status.success());
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("non-negative seconds")
    );
}

#[cfg(unix)]
#[test]
fn executable_shebang_invokes_scripturec_from_path() {
    use std::os::unix::fs::PermissionsExt;
    let path = std::env::temp_dir().join(format!(
        "scripturec-shebang-{}.scripture",
        std::process::id()
    ));
    std::fs::write(
        &path,
        "#!/usr/bin/env scripturec\ngenesis: praise \"executable\" amen\n",
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let binary_dir = std::path::Path::new(env!("CARGO_BIN_EXE_scripturec"))
        .parent()
        .unwrap();
    let mut paths = vec![binary_dir.to_path_buf()];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let result = Command::new(&path)
        .env("PATH", std::env::join_paths(paths).unwrap())
        .output();
    std::fs::remove_file(&path).unwrap();
    let result = result.unwrap();
    assert!(result.status.success(), "{:?}", result.stderr);
    assert_eq!(String::from_utf8(result.stdout).unwrap(), "executable\n");
}
