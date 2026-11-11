# Scripture semantics

The CLI, REPL, and browser playground share the execution rules described here.
Tokens are defined in [src/lexer.rs](./src/lexer.rs) and syntax in
[src/grammar.lalrpop](./src/grammar.lalrpop).

## Values and operations

Scripture is dynamically typed, with three value types: numbers, strings, and
booleans. A mutable variable can hold a value of a different type after an
assignment; its declaration does not fix its type.

Numbers use a single representation: finite IEEE 754 binary64 floating-point
values (`f64` in Rust). There is no separate integer type. For example, `5 / 2`
produces `2.5`, and `1` and `1.0` represent the same numeric value. Decimal
arithmetic can involve rounding, and sufficiently large integers lose precision.

There are no implicit conversions between types. Conditions must evaluate to
booleans; logical conjunction, disjunction, and negation accept booleans and
produce booleans. For example, `if 1:` is a runtime type error.

Arithmetic operators (`+`, `-`, `*`, `/`) accept numbers only. Unary plus
preserves a numeric value and unary minus negates it. The `+` operator does not
concatenate strings: both `"5" + 2` and `"a" + "b"` are runtime type errors.

Equality compares numbers numerically, strings by their exact contents, and
booleans by their truth values. Values of different types are unequal, so
`1 == "1"` and `1 == true` are false. Inequality is the logical inverse of
equality. Ordering (`<`, `>`) accepts numbers only; other operand types produce
a runtime type error. All comparisons produce booleans.

Division by zero and arithmetic results that are not finite are runtime errors.
NaN and positive or negative infinity are not language values. Numeric literals
outside the finite binary64 range are errors as well. The parser preserves numeric
spellings; the interpreter validates and converts them when evaluating literals.

## Interpreter metadata

`_scripture_version_` and `_scripture_credits_` are reserved string expressions,
available in scripts, the REPL, and WebAssembly. `_scripture_version_` evaluates
to the package version in `Cargo.toml`. `_scripture_credits_` evaluates to
`Scripture brought to you by jcjimenez`.
Use them directly, without quotes or parentheses, wherever a string value is
accepted. They cannot be declared, assigned, shadowed, or used as function or
parameter names. Quoting either spelling produces ordinary literal text.

## Evaluation order

Operands are evaluated from left to right within the grouping established by
the parser. Function arguments are also evaluated from left to right.

For example, `first(1) + second(2) * third(3)` calls `first`, then `second`, then
`third`; it multiplies the last two results before adding the first result.
Parentheses change grouping without reversing operand evaluation order.

`and` and `or` short-circuit: `and` skips its right operand when its left operand
is false, and `or` skips its right operand when its left operand is true.

## Assignment

An assignment chain evaluates its right-hand value once, then stores it in the
target variables from right to left. For example, `first = second = produce(7)`
calls `produce` once, assigns its result to `second`, then assigns that same
result to `first`.

## Bindings and scope

Variables and constants can be declared at the top level or inside blocks. Every
binding requires an initializer. The initializer is evaluated once before any of
the declared names become visible, and each name receives the resulting value.
For example, `let there be x, y = produce(7)` calls `produce` once.

Scope is lexical. Names resolve through the innermost enclosing block to the
global scope. A declaration may shadow an outer binding, but duplicate names in
the same scope are errors, including duplicates in a single declaration.
Assignment updates the nearest existing binding; it never implicitly declares a
name. Every assignment to a constant is an error, even if the value is unchanged.
All targets in an assignment chain are checked before evaluating its right side.

Functions see globals, their parameters, and their own enclosing blocks, never
the caller's local variables. Parameters are mutable local bindings. The function
body shares their scope; conditional bodies introduce child scopes. Variables and
functions cannot share a global name. A local value may shadow a function name,
but attempting to call that value is an error. Functions are not first-class values.

## Functions and control flow

Functions accept exactly their declared number of arguments. Empty parameter and
argument lists are allowed. Functions may call themselves or other functions.
Unprefixed calls are expressions; `preach` invokes a function as a statement and
discards its result.

`rejoice` returns immediately from the current function, including from nested
conditionals. Functions that finish without executing `rejoice` return `false`.
`rejoice` outside a function is an error. Assignments and conditionals are
statements and do not produce expression values.

## Sleeping

`pray expression` evaluates its operand once and pauses execution for that many
seconds before continuing with the next statement. For example, `pray 0.25`
pauses for a quarter second; `pray (delay * 2)` uses a computed duration. Like
`preach`, it is a statement and produces no value or output.

The operand must be a finite, non-negative number representable as a Rust
`Duration` (less than 2^64 seconds, rounded to nanoseconds). Zero is a no-op.
Strings, booleans, negative durations, and out-of-range durations produce a
runtime error at the operand. Unit suffixes and multiple duration arguments are
not supported; use arithmetic to express minutes or hours.

Native execution sleeps the calling thread. Browser execution awaits a timer,
allowing the event loop to run; `run_script` therefore returns a Promise. Sleep
may last longer than requested due to scheduling and timer precision. Sleeping
does not trigger prophecies or flush buffered output. The browser's ten-second
run timeout and Stop button still apply while sleeping.

## Program lifecycle and REPL

A file may have at most one `genesis` block. Execution registers all functions
first, initializes top-level bindings in source order, registers prophecies in
source order, then runs `genesis`. Functions can be called before their textual
declaration, but reading a global before it has been initialized is an error.
A file without `genesis` initializes its declarations and then finishes.

REPL submissions execute in input order within a persistent global environment.
Definitions remain available until the session is reset. Bare expressions display
their values; a submitted `genesis` block runs immediately. Same-scope redeclaration
remains an error in the REPL. Each browser Run starts a fresh environment.

## Output and errors

`praise` appends one output line. Strings print their contents, booleans print
`true` or `false`, and numbers use their floating-point display representation
(with both zero signs printed as `0`). Bare string expressions in the REPL display
with quotes to distinguish their value from printed text.

`confess` evaluates its operand and reports an uncaught `sin` containing the value's
printed representation. Syntax or runtime errors stop the current file or REPL
submission and report a source location. Earlier output and completed mutations
are retained; scopes and function-call state are unwound. A subsequent REPL
submission can continue. Pending prophecy events are discarded after an error.
Exception catching remains unimplemented.

## Prophecies

A prophecy watches the global names referenced by its condition. The condition
must produce a boolean and may use literals, names, operators, and parentheses.
Function calls in conditions are currently rejected to keep evaluation free of
side effects. All syntactically referenced names are dependencies, including
names in short-circuited operands.

Registration evaluates the initial condition without running the handler. After
an assignment changes a watched global value, the interpreter reevaluates affected
conditions and queues handlers whose condition transitions from false to true.
Assigning an equal value does not notify observers. An assignment chain finishes
all its stores before conditions are reevaluated.

Conditions are checked in registration order before queued handlers execute.
Handlers run in that order in their own local scopes with access to globals;
they do not inherit locals or return context from the code that caused the event.
Handler assignments may queue further handlers. Already queued handlers run even
if an earlier handler changes their condition again. Events are processed until
the queue is empty or an error/limit stops execution.

## Execution limits

Each file or REPL submission is limited to 100,000 execution steps and 64 KiB of
output. Inputs are limited to 128 KiB of UTF-8 and 4,096 tokens. File and stdin
readers stop at the byte limit plus one; the REPL applies that limit to the entire
pending submission. Oversized CLI input ends the command or REPL without waiting
for EOF. Library execution rejects oversized source before retaining a copy.

Syntax is limited to 32 nested blocks and 64 nodes of expression depth, including
operator chains, unary operators, grouping, and calls. These checks apply during
parsing, including to unused functions and unreachable statements, so cloning and
dropping syntax trees also remain bounded. Calls are limited to 64 nested function
invocations, expression evaluation to 128 active nodes, and combined execution to
192 active blocks and expressions. The combined counter spans calls and prophecy
dispatch and is not reset when entering a handler.

An interpreter session has an 8 MiB logical storage budget, at most
4,096 live bindings, 256 functions and prophecies combined, and 4,096 pending events.
Strings share immutable storage when copied into bindings or passed as arguments.
Each binding is charged its string length, name length, and
128 bytes of overhead. Each retained definition is charged the complete source,
twice its declaration's byte length, and 512 bytes per declaration token to cover
syntax and container overhead. This accounting is intentionally conservative; the
budget is not a measurement or hard cap on process resident memory.

Local scopes and parameters return their storage and binding budgets when they
exit, including on errors. Assignments adjust the charge for the replaced value.
Globals and definitions remain charged across REPL submissions until `:reset`;
fresh file/browser executions get fresh budgets. Failed bulk declarations and
assignments do not partially install their target values; side effects already
completed while evaluating an initializer or argument are retained.

Exceeding a limit reports an error rather than continuing. Diagnostics escape
terminal control characters and bidirectional overrides, truncate long messages,
and show a bounded source excerpt. Ordinary `praise` output remains literal text.

The browser additionally terminates its worker after ten seconds. Stop terminates
it immediately. Output is returned at the end of a run; terminating a worker before
it returns discards its buffered output.

## Not implemented

Strings have no escape syntax, and there is no general comment syntax. Exception
catching with `absolve` is not implemented, although the lexer reserves the keyword. Function calls in prophecy conditions, string
concatenation, and explicit type conversions are also unsupported.
