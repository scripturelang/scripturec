# Scripture

Scripture is a small, experimental programming language with scripture-inspired
syntax. Its Rust interpreter runs `.scripture` files, provides a REPL, and compiles
to WebAssembly for the browser playground.

```text
genesis:
    praise "Hallelujah"
amen
```

Newlines separate statements; use `;` for multiple statements on one line and
parentheses to continue expressions across lines. `genesis` is the main block,
`praise` prints a value, and `amen` closes a block.

## History

I first published a version of Scripture in 2009. The original project is preserved
in the [Google Code archive](https://codesite-archive.appspot.com/archive/p/scripture-lang/).

The archived source appears to stop short of where I got with that implementation,
which was closer to the language here today. This repository revisits that work
with a Rust interpreter, a REPL, and a browser playground.

## Build and install

With a recent stable Rust toolchain, run:

```sh
cargo build --locked
cargo install --path . --locked
```

The binary is built at `target/debug/scripturec`. Installation puts it in Cargo's
binary directory, normally `~/.cargo/bin`, which needs to be on your `PATH`.
To run without installing, use `cargo run -- ...`.

The build generates the parser with LALRPOP and the lexer with Logos.

## REPL

```sh
scripturec
```

The REPL keeps variables, functions, and prophecies between submissions. Bare
expressions display their values. Incomplete blocks and parenthesized expressions
show a continuation prompt until complete.

```text
scripture> let there be value = 21
scripture> value * 2
42
scripture> commandment double(n):
      ...>     rejoice n * 2
      ...> amen
scripture> double(value)
42
```

Use `:help`, `:reset` to clear the session, `:cancel` to discard incomplete input,
and `:quit` or Ctrl-D to exit. Syntax and runtime errors end the current submission;
completed mutations are retained. Oversized input ends the REPL. When input is
piped in, prompts are omitted and any error yields a failing exit code.

## Script files

```sh
scripturec examples/example01.scripture
scripturec --check examples/example01.scripture
scripturec --help
```

Passing a path executes the file. `--check` checks syntax only, without executing
or validating runtime semantics. Syntax errors, runtime errors, and unreadable
files produce a nonzero exit code. Use `scripturec -` to execute a complete program
from standard input, or `scripturec -- ./-filename.scripture` for a path beginning
with a dash.

On Unix-like systems, executable scripts can begin with a shebang:

```text
#!/usr/bin/env scripturec
genesis:
    praise "Hallelujah"
amen
```

Save this as `hello.scripture`, then run:

```sh
chmod +x hello.scripture
./hello.scripture
```

The shebang must be the first line, and `scripturec` must be on your `PATH`.
See [examples/executable.scripture](./examples/executable.scripture).

## Pausing execution

Use `pray` as a statement to pause for a number of seconds, like shell `sleep`:

```text
genesis:
    praise "Before"
    pray 0.5
    praise "After"
amen
```

The duration can be any numeric expression, including a variable or function
call. Fractional seconds and zero are supported; negative durations and other
value types are errors. Durations use seconds only (no shell unit suffixes).
`pray` produces no value or output and works in scripts, the REPL, and the browser.
The playground's ten-second run timeout includes time spent sleeping, and output
remains buffered until execution finishes.

## Times

Use `time of our lord()` to capture the current moment, or supply a timestamp
string with an explicit timezone. Times print in UTC, compare by instant, and
subtract to give elapsed seconds:

```text
genesis:
    let there be now = time of our lord()
    praise now

    let there be vigil = time of our lord("2026-12-24T23:00:00Z")
    let there be christmas = time of our lord("2026-12-25T00:00:00Z")
    praise vigil < christmas
    praise christmas - vigil
amen
```

The last two lines print `true` and `3600`. Invalid dates and timestamps without
a timezone are errors. See [example11.scripture](./examples/example11.scripture)
and the [time semantics](./SEMANTICS.md#times) for precision and supported operations.

## Interpreter metadata

Two reserved expressions expose interpreter information as strings:

```text
genesis:
    praise _scripture_version_
    praise _scripture_credits_
amen
```

These print the interpreter's version and
`Scripture brought to you by jcjimenez`. They also work directly in the REPL and
in the browser. Both are reserved and cannot be reassigned or shadowed.

## WebAssembly runtime

The static website and browser playground live in
[scripturelang/www](https://github.com/scripturelang/www). This repository builds
its runtime independently:

```sh
rustup target add wasm32-unknown-unknown # one-time setup
cargo web
```

`cargo web` builds the release Wasm library and runs the matching `wasm-bindgen`
CLI. The standalone `pkg/` bundle contains `scripturec_bg.wasm`, its JavaScript
loader, helper snippets, TypeScript declarations, and an ES module `package.json`.
It uses a compatible CLI from `PATH` or installs one locally under `target/tools`.
Once dependencies and tools are cached, use `cargo web --offline`.

Generated files under `pkg/` and `target/` are ignored by Git. Building the runtime
does not change or deploy the website. When a runtime is ready, install it from
the website checkout (assuming the repositories are siblings):

```sh
cd ../www
make update
```

`make update` validates the complete bundle, stores it under `web/pkg/<hash>/`,
and selects it through a manifest. Unchanged bundles reuse the same hash; older
bundles remain available to existing sessions. Review and commit the new bundle
and manifest with the website changes. The website uses this checked-in copy and
needs no Rust toolchain to build or deploy. See its README for local serving and
hosting instructions.

The bindings are also reusable from other static pages:

```html
<script type="module">
  import init, { run_script } from './pkg/scripturec.js';
  await init();
  const result = JSON.parse(await run_script('genesis: praise "Hello!" amen'));
  console.log(result.output, result.error);
</script>
```

`run_script(source)` returns a Promise resolving to JSON containing `output`
(an array of printed lines) and `error` (a diagnostic string or `null`). The
playground runs it inside a module worker to keep the page responsive.

## Language status

Implemented: finite `f64` numbers, strings, booleans, UTC times, strict type checks,
arithmetic and comparisons, short-circuit logic, mutable variables, constants,
lexical local scopes, functions and recursion, returns, conditionals, printing,
assignment chains, sleeping with `pray`, and reactive `prophecy` handlers.

The [Logos definitions](./src/lexer.rs) and
[LALRPOP grammar](./src/grammar.lalrpop) define accepted syntax.
[SEMANTICS.md](./SEMANTICS.md) describes execution rules and limits.
The [examples](./examples/) run on both native and Wasm builds.

`confess` reports an uncaught exception; catching
with `absolve` is not implemented. Strings do not process
escapes, and general comment syntax is not defined. Prophecy conditions currently
exclude function calls.

## Development and checks

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
cargo web
cargo build --locked
node scripts/test-web.mjs
```

The Node checks require Node.js 22+. `test-web.mjs` compares native and Wasm
results, errors, limits, and asynchronous execution. Website lesson and UI checks
live in `www`. Rust tests cover parsing, execution, the CLI, and resource limits.

[CI](./.github/workflows/security.yml) runs tests and dependency audits on pushes,
pull requests, and a weekly schedule. Dependabot checks Cargo dependencies and
GitHub Actions weekly. To audit locally:

```sh
cargo install cargo-audit --version 0.23.0 --locked
cargo audit --file Cargo.lock
```
