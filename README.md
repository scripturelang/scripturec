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

## Browser playground

Build the browser runtime with Rust:

```sh
rustup target add wasm32-unknown-unknown # one-time setup
cargo web
```

`cargo web` builds the release Wasm library, runs the matching `wasm-bindgen` CLI,
and copies the playground examples into `web/pkg/`. It uses a compatible CLI from
`PATH` or installs one locally under `target/tools`. Once dependencies and tools
are cached, use `cargo web --offline`.

Serve `web/` with a static HTTP server. For example, with
[miniserve](https://github.com/svenstaro/miniserve):

```sh
cargo install miniserve --locked # one-time, or use your existing static server
miniserve web --index index.html --port 8080
```

Open [localhost:8080](http://localhost:8080). Opening `index.html` directly with
`file://` is not supported. To deploy, upload the contents of `web/`, including the
generated `pkg/` directory, to a static host.

HTML, CSS, and JavaScript changes only need a browser refresh. Re-run `cargo web`
after changing Rust code or examples. Generated files under `web/pkg/` and
`target/` are ignored by Git.

### Cloudflare hosting

The `scripturec` Worker serves `web/` as static assets. Workers Builds connects
directly to this repository and builds and deploys pushes to `main`:

- Root directory: repository root
- Build command: `npm run build`
- Deploy command: `npm run deploy`
- Preview builds: disabled

Cloudflare installs the pinned Node dependencies from `package-lock.json`.
The build script installs Rust 1.96.0 and its WebAssembly target, runs `cargo web`,
then packages the static assets with `cf build`. Deployment uses `cf deploy
--prebuilt`, so it publishes exactly that build. `cloudflare.config.ts` selects
the Worker and personal Cloudflare account; `wrangler.config.ts` supplies the
static asset directory to the build tool. No deployment runs in GitHub Actions.

To validate locally, run `npm ci`, `npm run build`, and
`npx cf deploy --prebuilt --dry-run`. Deployments require Cloudflare authentication;
credentials belong in Cloudflare's build settings, never in this repository.

Each run gets a fresh interpreter in a module worker. Stop terminates that worker.
Four guided examples in `web/lessons.js` introduce printing, variables, functions,
and prophecies, with a small experiment and expected output for each. A syntax
guide stays available below the workspace. On phones, Code and Output switch
views and Run stays at the bottom of the screen.

The page saves a separate local draft for each example and a personal workspace
when browser storage is available. Example changes and resets can be undone,
including after a reload. Existing drafts migrate to the personal workspace.
Runtime errors include output produced before the error, a readable diagnostic,
and a link to the source line; stopping a run discards its buffered output. Use
Ctrl/⌘ + Enter to run. Tab indents; Esc followed by Tab leaves the editor.

The bindings are also reusable from other static pages:

```html
<script type="module">
  import init, { run_script } from './pkg/scripturec.js';
  await init();
  const result = JSON.parse(await run_script('genesis: praise "Hello!" amen'));
  console.log(result.output, result.error);
</script>
```

`run_script(source)` returns a Promise resolving to JSON containing `output` (an array of printed lines)
and `error` (a diagnostic string or `null`). The playground calls it inside
`web/worker.js` to keep the page responsive during execution.

## Language status

Implemented: finite `f64` numbers, strings, booleans, strict type checks,
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
node scripts/test-playground.mjs
```

The Node checks require Node.js 22+. `test-web.mjs` compares native and Wasm
results and executes every guided lesson; `test-playground.mjs` checks draft
recovery, lesson navigation, mobile views, keyboard handling, diagnostics, and the
worker lifecycle with a simulated DOM. Rust tests cover parsing, execution, the CLI, and resource limits.

[CI](./.github/workflows/security.yml) runs tests and dependency audits on pushes,
pull requests, and a weekly schedule. Dependabot checks Cargo dependencies and
GitHub Actions weekly. To audit locally:

```sh
cargo install cargo-audit --version 0.23.0 --locked
cargo audit --file Cargo.lock
```
