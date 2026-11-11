use scripturec::{
    ParseError,
    diagnostic::{Diagnostic, terminal_text},
    interpreter::{self, Execution, Interpreter},
    parse_program, parse_repl,
};
use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, BufRead, IsTerminal, Read, Write},
    path::Path,
    process::ExitCode,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    match args.as_slice() {
        [] => repl(),
        [flag] if flag == "--help" || flag == "-h" => { print_help(); Ok(()) }
        [flag] if flag == "--version" || flag == "-V" => {
            println!("scripturec {}", env!("CARGO_PKG_VERSION")); Ok(())
        }
        [flag, file] if flag == "--check" => file_command(file, true),
        [flag, file] if flag == "--" => file_command(file, false),
        [file] if file == "-" || !file.to_string_lossy().starts_with('-') => file_command(file, false),
        _ => Err("usage: scripturec [--check] <file.scripture>\nRun scripturec without arguments to start the REPL.".into()),
    }
}

fn file_command(file: &OsString, check: bool) -> Result<(), String> {
    let path = Path::new(file);
    let label = terminal_text(&path.to_string_lossy());
    let source = if file == "-" {
        read_source(io::stdin().lock()).map_err(|error| format!("stdin: {error}"))?
    } else {
        let file = fs::File::open(path).map_err(|error| format!("{label}: {error}"))?;
        read_source(file).map_err(|error| format!("{label}: {error}"))?
    };
    if check {
        let program = parse_program(&source)
            .map_err(|error| format!("{}: {}", label, Diagnostic::from_parse(&source, error)))?;
        println!(
            "Parsed {} ({} top-level declarations).",
            label,
            program.declarations.len()
        );
        Ok(())
    } else {
        display(interpreter::run(&source)).map_err(|error| format!("{label}: {error}"))
    }
}

fn read_source(reader: impl Read) -> Result<String, String> {
    let mut bytes = Vec::new();
    reader
        .take((scripturec::MAX_SOURCE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > scripturec::MAX_SOURCE_BYTES {
        return Err(scripturec::SOURCE_LIMIT_MESSAGE.into());
    }
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

fn display(execution: Execution) -> Result<(), String> {
    let mut stdout = io::stdout().lock();
    for line in execution.output {
        writeln!(stdout, "{line}").map_err(|error| error.to_string())?;
    }
    stdout.flush().map_err(|error| error.to_string())?;
    execution.error.map_or(Ok(()), Err)
}

fn repl() -> Result<(), String> {
    let interactive = io::stdin().is_terminal();
    if interactive {
        println!("Scripture REPL — :help for commands, :quit or Ctrl-D to exit.");
    }
    let mut session = Interpreter::new();
    let mut input = io::stdin().lock();
    let mut submission = String::new();
    let mut had_error = false;
    loop {
        if interactive {
            print!(
                "{}",
                if submission.is_empty() {
                    "scripture> "
                } else {
                    "      ...> "
                }
            );
            io::stdout().flush().map_err(|error| error.to_string())?;
        }
        let mut line = Vec::new();
        let remaining = scripturec::MAX_SOURCE_BYTES - submission.len();
        let read = (&mut input)
            .take((remaining + 1) as u64)
            .read_until(b'\n', &mut line)
            .map_err(|error| error.to_string())?;
        if line.len() > remaining {
            return Err(scripturec::SOURCE_LIMIT_MESSAGE.into());
        }
        let line = String::from_utf8(line).map_err(|error| error.to_string())?;
        if read == 0 {
            if !submission.trim().is_empty()
                && let Err(error) = display(session.execute_repl(&submission))
            {
                eprintln!("{error}");
                had_error = true;
            }
            break;
        }
        match line.trim() {
            ":quit" | ":q" => break,
            ":cancel" => {
                submission.clear();
                continue;
            }
            ":reset" => {
                session = Interpreter::new();
                submission.clear();
                continue;
            }
            ":help" => {
                println!(
                    ":quit   Exit\n:reset  Clear all bindings and functions\n:cancel Discard incomplete input\n:help   Show commands\nFinish blocks with amen. Bare expressions display their values."
                );
                continue;
            }
            _ => {}
        }
        submission.push_str(&line);
        if matches!(
            parse_repl(&submission),
            Err(ParseError::UnrecognizedEof { .. })
        ) {
            continue;
        }
        if let Err(error) = display(session.execute_repl(&submission)) {
            eprintln!("{error}");
            had_error = true;
        }
        submission.clear();
    }
    if had_error && !interactive {
        Err("REPL input contained errors".into())
    } else {
        Ok(())
    }
}

fn print_help() {
    println!(
        "Usage: scripturec [FILE]\n\n  No arguments       Start the interactive REPL\n  FILE               Interpret a .scripture file\n  -                  Interpret a script from standard input\n  --check FILE       Check syntax without executing\n  --help             Show this help\n  --version          Show the version\n\nExecutable scripts can start with #!/usr/bin/env scripturec."
    );
}
