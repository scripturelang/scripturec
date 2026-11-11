//! Host-independent tree-walking interpreter used by the CLI, REPL, and Wasm.

use crate::{ast::*, diagnostic::Diagnostic, parse_program, parse_repl};
use serde::Serialize;
use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    fmt,
    rc::Rc,
};

type Scope = HashMap<String, Binding>;
const MAX_STEPS: usize = 100_000;
const MAX_CALL_DEPTH: usize = 64;
const MAX_EXPRESSION_DEPTH: usize = 128;
const MAX_OUTPUT_BYTES: usize = 65_536;
const MAX_NESTING: usize = 192;
const MAX_STORAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_BINDINGS: usize = 4096;
const MAX_DEFINITIONS: usize = 256;
const MAX_PENDING_EVENTS: usize = 4096;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    String(Rc<str>),
    Boolean(bool),
    Time(crate::moment::Moment),
}

impl Value {
    // Charge shared strings per binding as a conservative logical storage budget.
    fn storage_bytes(&self) -> usize {
        match self {
            Self::String(text) => text.len(),
            _ => 0,
        }
    }
}

fn scope_bytes(scope: &Scope) -> usize {
    scope
        .iter()
        .map(|(name, binding)| 128 + name.len() + binding.value.storage_bytes())
        .sum()
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) if *n == 0.0 => write!(f, "0"),
            Self::Number(n) => write!(f, "{n}"),
            Self::String(s) => f.write_str(s),
            Self::Boolean(b) => write!(f, "{b}"),
            Self::Time(time) => write!(f, "{time}"),
        }
    }
}

#[derive(Debug, Default, Serialize)]
pub struct Execution {
    pub output: Vec<String>,
    pub error: Option<String>,
}

#[derive(Clone)]
struct Binding {
    value: Value,
    constant: bool,
}
struct Function {
    parameters: Vec<String>,
    body: Block,
    source: Rc<str>,
}
struct Prophecy {
    condition: Expr,
    body: Block,
    source: Rc<str>,
    dependencies: BTreeSet<String>,
}

enum Flow {
    Continue,
    Return(Value),
}

/// A persistent REPL session. File and browser runs normally use a fresh instance.
#[derive(Default)]
pub struct Interpreter {
    globals: Scope,
    scopes: Vec<Scope>,
    functions: HashMap<String, Rc<Function>>,
    prophecies: Vec<Rc<Prophecy>>,
    prophecy_values: Vec<bool>,
    pending: VecDeque<usize>,
    dispatching: bool,
    source: Rc<str>,
    output: Vec<String>,
    output_bytes: usize,
    steps: usize,
    call_depth: usize,
    expression_depth: usize,
    nesting: usize,
    storage_bytes: usize,
    binding_count: usize,
}

impl Interpreter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Execute a program, yielding to host timers in Wasm builds.
    /// Native builds block during sleep. If this future is dropped before
    /// completion, discard the interpreter rather than reusing its partial state.
    pub async fn execute_program_async(&mut self, source: &str) -> Execution {
        if let Err(execution) = self.begin(source) {
            return execution;
        }
        let result = match parse_program(source) {
            Ok(program) => self.program(&program).await,
            Err(error) => Err(Diagnostic::from_parse(source, error)),
        };
        self.finish(result)
    }

    /// Execute a submission with the same timer and cancellation behavior as
    /// `execute_program_async`, retaining definitions between completed calls.
    pub async fn execute_repl_async(&mut self, source: &str) -> Execution {
        if let Err(execution) = self.begin(source) {
            return execution;
        }
        let result = async {
            let items =
                parse_repl(source).map_err(|error| Diagnostic::from_parse(source, error))?;
            for item in &items {
                match item {
                    ReplItem::Declaration(declaration) => self.declaration(declaration).await?,
                    ReplItem::Statement(statement) => {
                        if let StatementKind::Expression(expr) = &statement.value {
                            let value = self.eval(expr).await?;
                            let display = match value {
                                Value::String(s) => format!("{s:?}"),
                                other => other.to_string(),
                            };
                            self.print(display, &statement.span)?;
                        } else {
                            Box::pin(self.statement(statement)).await?;
                        }
                    }
                }
            }
            Ok(())
        }
        .await;
        self.finish(result)
    }

    /// Execute natively, blocking the calling thread during `pray`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn execute_program(&mut self, source: &str) -> Execution {
        complete_native(self.execute_program_async(source))
    }

    /// Execute a native REPL submission, retaining session state.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn execute_repl(&mut self, source: &str) -> Execution {
        complete_native(self.execute_repl_async(source))
    }

    fn begin(&mut self, source: &str) -> Result<(), Execution> {
        if source.len() > crate::MAX_SOURCE_BYTES {
            return Err(Execution {
                output: vec![],
                error: Some(crate::SOURCE_LIMIT_MESSAGE.into()),
            });
        }
        self.source = Rc::from(source);
        self.output.clear();
        self.output_bytes = 0;
        self.steps = 0;
        Ok(())
    }

    fn reserve_bindings(
        &mut self,
        bytes: usize,
        count: usize,
        span: &Span,
    ) -> Result<(), Diagnostic> {
        if self.binding_count + count > MAX_BINDINGS {
            return Err(self.error(span, "binding limit reached (4096 live bindings)"));
        }
        self.check_storage(self.storage_bytes + bytes, span)?;
        self.storage_bytes += bytes;
        self.binding_count += count;
        Ok(())
    }

    fn release_scope(&mut self, scope: &Scope) {
        self.storage_bytes -= scope_bytes(scope);
        self.binding_count -= scope.len();
    }

    fn check_storage(&self, bytes: usize, span: &Span) -> Result<(), Diagnostic> {
        if bytes > MAX_STORAGE_BYTES {
            Err(self.error(span, "storage limit reached (8 MiB logical storage)"))
        } else {
            Ok(())
        }
    }

    fn reserve_definition(&mut self, span: &Span) -> Result<(), Diagnostic> {
        if self.functions.len() + self.prophecies.len() >= MAX_DEFINITIONS {
            return Err(self.error(
                span,
                "definition limit reached (256 functions and prophecies)",
            ));
        }
        // Conservatively account for retained source, cloned syntax, strings and containers.
        // Each definition pays for the full source even when that source is shared.
        let text = &self.source[span.clone()];
        let bytes =
            self.source.len() + 2 * text.len() + 512 * crate::lexer::Lexer::new(text).count();
        self.check_storage(self.storage_bytes + bytes, span)?;
        self.storage_bytes += bytes;
        Ok(())
    }

    fn enter(&mut self, span: &Span) -> Result<(), Diagnostic> {
        if self.nesting >= MAX_NESTING {
            return Err(self.error(
                span,
                "execution nesting limit reached (192 blocks and expressions)",
            ));
        }
        self.nesting += 1;
        Ok(())
    }

    fn finish(&mut self, result: Result<(), Diagnostic>) -> Execution {
        // A failed submission retains completed mutations, but not pending events.
        self.pending.clear();
        self.dispatching = false;
        Execution {
            output: std::mem::take(&mut self.output),
            error: result.err().map(|error| error.to_string()),
        }
    }

    fn error(&self, span: &Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            source: self.source.clone(),
            span: span.clone(),
            message: message.into(),
        }
    }

    fn tick(&mut self, span: &Span) -> Result<(), Diagnostic> {
        self.steps += 1;
        if self.steps > MAX_STEPS {
            Err(self.error(span, "execution limit reached (100000 steps)"))
        } else {
            Ok(())
        }
    }

    async fn program(&mut self, program: &Program) -> Result<(), Diagnostic> {
        let mut genesis = None;
        // Functions are registered before global initializers, enabling recursion
        // and forward calls. Variable initialization remains in source order.
        for declaration in &program.declarations {
            match &declaration.value {
                DeclarationKind::Commandment { .. } => self.declaration(declaration).await?,
                DeclarationKind::Genesis(body) => {
                    if genesis.is_some() {
                        return Err(self.error(
                            &declaration.span,
                            "a file may contain only one genesis block",
                        ));
                    }
                    genesis = Some(body);
                }
                _ => {}
            }
        }
        for declaration in &program.declarations {
            if matches!(
                declaration.value,
                DeclarationKind::Variable { .. } | DeclarationKind::Constant { .. }
            ) {
                self.declaration(declaration).await?;
            }
        }
        for declaration in &program.declarations {
            if matches!(declaration.value, DeclarationKind::Prophecy { .. }) {
                self.declaration(declaration).await?;
            }
        }
        if let Some(body) = genesis {
            self.block(body, true).await?;
        }
        Ok(())
    }

    async fn declaration(&mut self, declaration: &Declaration) -> Result<(), Diagnostic> {
        self.tick(&declaration.span)?;
        match &declaration.value {
            DeclarationKind::Variable { names, initializer }
            | DeclarationKind::Constant { names, initializer } => {
                let constant = matches!(declaration.value, DeclarationKind::Constant { .. });
                let Some(initializer) = initializer else {
                    return Err(
                        self.error(&declaration.span, "declarations require an initializer")
                    );
                };
                let mut seen = BTreeSet::new();
                for name in names {
                    if !seen.insert(name)
                        || self.current_scope().contains_key(name)
                        || (self.scopes.is_empty() && self.functions.contains_key(name))
                    {
                        return Err(self.error(
                            &declaration.span,
                            format!("{name:?} is already declared in this scope"),
                        ));
                    }
                }
                let value = self.eval(initializer).await?;
                let bytes = names
                    .iter()
                    .map(|name| 128 + name.len() + value.storage_bytes())
                    .sum();
                self.reserve_bindings(bytes, names.len(), &declaration.span)?;
                let scope = self.scopes.last_mut().unwrap_or(&mut self.globals);
                for name in names {
                    scope.insert(
                        name.clone(),
                        Binding {
                            value: value.clone(),
                            constant,
                        },
                    );
                }
            }
            DeclarationKind::Commandment {
                name,
                parameters,
                body,
            } => {
                if self.globals.contains_key(name) || self.functions.contains_key(name) {
                    return Err(
                        self.error(&declaration.span, format!("{name:?} is already declared"))
                    );
                }
                let unique: BTreeSet<_> = parameters.iter().collect();
                if unique.len() != parameters.len() {
                    return Err(self.error(
                        &declaration.span,
                        "function parameters must have distinct names",
                    ));
                }
                self.reserve_definition(&declaration.span)?;
                self.functions.insert(
                    name.clone(),
                    Rc::new(Function {
                        parameters: parameters.clone(),
                        body: body.clone(),
                        source: self.source.clone(),
                    }),
                );
            }
            DeclarationKind::Genesis(body) => {
                self.block(body, true).await?;
            }
            DeclarationKind::Prophecy { condition, body } => {
                let mut dependencies = BTreeSet::new();
                self.dependencies(condition, &mut dependencies)?;
                let value = self.eval(condition).await?;
                let last = self.boolean(value, &condition.span)?;
                self.reserve_definition(&declaration.span)?;
                self.prophecies.push(Rc::new(Prophecy {
                    condition: condition.clone(),
                    body: body.clone(),
                    source: self.source.clone(),
                    dependencies,
                }));
                self.prophecy_values.push(last);
            }
        }
        Ok(())
    }

    fn current_scope(&self) -> &Scope {
        self.scopes.last().unwrap_or(&self.globals)
    }

    fn lookup(&self, name: &str) -> Option<&Binding> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .or_else(|| self.globals.get(name))
    }

    async fn block(&mut self, body: &Block, new_scope: bool) -> Result<Flow, Diagnostic> {
        self.enter(
            &body
                .first()
                .map_or(0..0, |statement| statement.span.clone()),
        )?;
        if new_scope {
            self.scopes.push(Scope::new());
        }
        let result = async {
            for statement in body {
                if let Flow::Return(value) = Box::pin(self.statement(statement)).await? {
                    return Ok(Flow::Return(value));
                }
            }
            Ok(Flow::Continue)
        }
        .await;
        if new_scope {
            let scope = self.scopes.pop().unwrap();
            self.release_scope(&scope);
        }
        self.nesting -= 1;
        result
    }

    async fn statement(&mut self, statement: &Statement) -> Result<Flow, Diagnostic> {
        self.tick(&statement.span)?;
        match &statement.value {
            StatementKind::Declaration(declaration) => self.declaration(declaration).await?,
            StatementKind::Expression(expr) | StatementKind::Preach(expr) => {
                self.eval(expr).await?;
            }
            StatementKind::Pray(expr) => {
                let value = self.eval(expr).await?;
                let seconds = self.number(value, &expr.span)?;
                let duration =
                    std::time::Duration::try_from_secs_f64(seconds).map_err(|_| {
                        self.error(&expr.span,
                        "pray requires non-negative seconds within the supported duration range")
                    })?;
                crate::sleep::sleep(duration).await;
            }
            StatementKind::Praise(expr) => {
                let value = self.eval(expr).await?;
                self.print(value.to_string(), &statement.span)?;
            }
            StatementKind::Return(expr) => {
                if self.call_depth == 0 {
                    return Err(self.error(
                        &statement.span,
                        "rejoice is only valid inside a commandment",
                    ));
                }
                return Ok(Flow::Return(self.eval(expr).await?));
            }
            StatementKind::Throw(expr) => {
                let value = self.eval(expr).await?;
                return Err(self.error(&statement.span, format!("uncaught sin: {value}")));
            }
            StatementKind::Conditional { condition, body } => {
                let value = self.eval(condition).await?;
                if self.boolean(value, &condition.span)? {
                    return self.block(body, true).await;
                }
            }
            StatementKind::Assignment { targets, value } => {
                // Validate every target before evaluating the RHS or changing a binding.
                for name in targets {
                    match self.lookup(name) {
                        None => {
                            return Err(
                                self.error(&statement.span, format!("undefined name {name:?}"))
                            );
                        }
                        Some(binding) if binding.constant => {
                            return Err(self.error(
                                &statement.span,
                                format!("cannot assign to constant {name:?}"),
                            ));
                        }
                        _ => {}
                    }
                }
                let value = self.eval(value).await?;
                let unique: BTreeSet<_> = targets.iter().collect();
                let previous: usize = unique
                    .iter()
                    .map(|name| self.lookup(name).unwrap().value.storage_bytes())
                    .sum();
                let next = self.storage_bytes - previous + unique.len() * value.storage_bytes();
                self.check_storage(next, &statement.span)?;
                self.storage_bytes = next;
                let mut changed = BTreeSet::new();
                for name in targets.iter().rev() {
                    if let Some(binding) = self
                        .scopes
                        .iter_mut()
                        .rev()
                        .find_map(|scope| scope.get_mut(name))
                    {
                        binding.value = value.clone();
                    } else if let Some(binding) = self.globals.get_mut(name) {
                        if binding.value != value {
                            changed.insert(name.clone());
                        }
                        binding.value = value.clone();
                    }
                }
                self.notify(&changed).await?;
            }
        }
        Ok(Flow::Continue)
    }

    fn print(&mut self, text: String, span: &Span) -> Result<(), Diagnostic> {
        self.output_bytes += text.len() + 1;
        if self.output_bytes > MAX_OUTPUT_BYTES {
            return Err(self.error(span, "output limit reached (64 KiB)"));
        }
        self.output.push(text);
        Ok(())
    }

    async fn eval(&mut self, expr: &Expr) -> Result<Value, Diagnostic> {
        self.tick(&expr.span)?;
        if self.expression_depth >= MAX_EXPRESSION_DEPTH {
            return Err(self.error(&expr.span, "expression nesting limit reached"));
        }
        self.enter(&expr.span)?;
        self.expression_depth += 1;
        // Box recursive futures after checking the execution depth budget.
        let result = Box::pin(self.eval_inner(expr)).await;
        self.expression_depth -= 1;
        self.nesting -= 1;
        result
    }

    async fn eval_inner(&mut self, expr: &Expr) -> Result<Value, Diagnostic> {
        match &expr.value {
            ExprKind::Number(text) => {
                let number: f64 = text
                    .parse()
                    .map_err(|_| self.error(&expr.span, "invalid number"))?;
                self.finite(number, &expr.span)
            }
            ExprKind::String(text) => Ok(Value::String(Rc::from(text.as_str()))),
            ExprKind::Boolean(value) => Ok(Value::Boolean(*value)),
            ExprKind::Time(arguments) => {
                let result = match arguments.as_slice() {
                    [] => crate::moment::Moment::now(),
                    [argument] => {
                        let Value::String(text) = self.eval(argument).await? else {
                            return Err(self.error(&argument.span,
                                "time of our lord expects a timestamp string; implicit type conversions are not allowed"));
                        };
                        return crate::moment::Moment::parse(&text)
                            .map(Value::Time)
                            .map_err(|message| self.error(&argument.span, message));
                    }
                    _ => {
                        return Err(
                            self.error(&expr.span, "time of our lord expects 0 or 1 arguments")
                        );
                    }
                };
                result
                    .map(Value::Time)
                    .map_err(|message| self.error(&expr.span, message))
            }
            ExprKind::Identifier(name) => self
                .lookup(name)
                .map(|binding| binding.value.clone())
                .ok_or_else(|| self.error(&expr.span, format!("undefined name {name:?}"))),
            ExprKind::Grouped(inner) => self.eval(inner).await,
            ExprKind::Unary { operator, operand } => {
                let value = self.eval(operand).await?;
                match operator {
                    UnaryOperator::Not => Ok(Value::Boolean(!self.boolean(value, &operand.span)?)),
                    UnaryOperator::Positive => {
                        Ok(Value::Number(self.number(value, &operand.span)?))
                    }
                    UnaryOperator::Negative => {
                        Ok(Value::Number(-self.number(value, &operand.span)?))
                    }
                }
            }
            ExprKind::Binary {
                left,
                operator,
                right,
            } => {
                use BinaryOperator::*;
                let lhs = self.eval(left).await?;
                match operator {
                    And if !self.boolean(lhs.clone(), &left.span)? => {
                        return Ok(Value::Boolean(false));
                    }
                    Or if self.boolean(lhs.clone(), &left.span)? => {
                        return Ok(Value::Boolean(true));
                    }
                    _ => {}
                }
                let rhs = self.eval(right).await?;
                match operator {
                    Equal => Ok(Value::Boolean(lhs == rhs)),
                    NotEqual => Ok(Value::Boolean(lhs != rhs)),
                    And | Or => Ok(Value::Boolean(self.boolean(rhs, &right.span)?)),
                    Subtract | Less | Greater
                        if matches!((&lhs, &rhs), (Value::Time(_), Value::Time(_))) =>
                    {
                        let (Value::Time(lhs), Value::Time(rhs)) = (lhs, rhs) else {
                            unreachable!()
                        };
                        match operator {
                            Subtract => self.finite(lhs.seconds_since(rhs), &expr.span),
                            Less => Ok(Value::Boolean(lhs < rhs)),
                            Greater => Ok(Value::Boolean(lhs > rhs)),
                            _ => unreachable!(),
                        }
                    }
                    _ => {
                        let lhs = self.number(lhs, &left.span)?;
                        let rhs = self.number(rhs, &right.span)?;
                        match operator {
                            Add => self.finite(lhs + rhs, &expr.span),
                            Subtract => self.finite(lhs - rhs, &expr.span),
                            Multiply => self.finite(lhs * rhs, &expr.span),
                            Divide if rhs == 0.0 => Err(self.error(&expr.span, "division by zero")),
                            Divide => self.finite(lhs / rhs, &expr.span),
                            Less => Ok(Value::Boolean(lhs < rhs)),
                            Greater => Ok(Value::Boolean(lhs > rhs)),
                            _ => unreachable!(),
                        }
                    }
                }
            }
            ExprKind::Call { name, arguments } => self.call(name, arguments, &expr.span).await,
        }
    }

    fn number(&self, value: Value, span: &Span) -> Result<f64, Diagnostic> {
        match value {
            Value::Number(number) => Ok(number),
            _ => Err(self.error(
                span,
                "expected a number; implicit type conversions are not allowed",
            )),
        }
    }

    fn boolean(&self, value: Value, span: &Span) -> Result<bool, Diagnostic> {
        match value {
            Value::Boolean(boolean) => Ok(boolean),
            _ => Err(self.error(
                span,
                "expected a boolean; implicit type conversions are not allowed",
            )),
        }
    }

    fn finite(&self, value: f64, span: &Span) -> Result<Value, Diagnostic> {
        if value.is_finite() {
            Ok(Value::Number(value))
        } else {
            Err(self.error(span, "number is outside the finite f64 range"))
        }
    }

    async fn call(
        &mut self,
        name: &str,
        arguments: &[Expr],
        span: &Span,
    ) -> Result<Value, Diagnostic> {
        if self.lookup(name).is_some() {
            return Err(self.error(
                span,
                format!("{name:?} is a value, not a callable commandment"),
            ));
        }
        let function = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| self.error(span, format!("undefined commandment {name:?}")))?;
        if function.parameters.len() != arguments.len() {
            return Err(self.error(
                span,
                format!(
                    "{name} expects {} arguments, received {}",
                    function.parameters.len(),
                    arguments.len()
                ),
            ));
        }
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(self.error(span, "call depth limit reached (64 calls)"));
        }
        let mut parameters = Scope::new();
        let result = async {
            for (name, argument) in function.parameters.iter().zip(arguments) {
                let value = self.eval(argument).await?;
                self.reserve_bindings(128 + name.len() + value.storage_bytes(), 1, &argument.span)?;
                parameters.insert(
                    name.clone(),
                    Binding {
                        value,
                        constant: false,
                    },
                );
            }
            Ok(())
        }
        .await;
        if let Err(error) = result {
            self.release_scope(&parameters);
            return Err(error);
        }
        // Function bodies see globals and their own lexical scopes, not callers' locals.
        let saved_scopes = std::mem::replace(&mut self.scopes, vec![parameters]);
        let saved_source = std::mem::replace(&mut self.source, function.source.clone());
        self.call_depth += 1;
        let result = self.block(&function.body, false).await;
        self.call_depth -= 1;
        for scope in std::mem::replace(&mut self.scopes, saved_scopes) {
            self.release_scope(&scope);
        }
        self.source = saved_source;
        match result? {
            Flow::Return(value) => Ok(value),
            Flow::Continue => Ok(Value::Boolean(false)),
        }
    }

    fn dependencies(&self, expr: &Expr, names: &mut BTreeSet<String>) -> Result<(), Diagnostic> {
        match &expr.value {
            ExprKind::Identifier(name) => { names.insert(name.clone()); }
            ExprKind::Unary { operand, .. } | ExprKind::Grouped(operand) => self.dependencies(operand, names)?,
            ExprKind::Binary { left, right, .. } => {
                self.dependencies(left, names)?;
                self.dependencies(right, names)?;
            }
            ExprKind::Call { .. } => return Err(self.error(&expr.span, "prophecy conditions must be side-effect-free; function calls are not supported here")),
            ExprKind::Time(_) => return Err(self.error(&expr.span, "time constructors are not supported in prophecy conditions; assign the time to a global variable first")),
            _ => {}
        }
        Ok(())
    }

    async fn notify(&mut self, changed: &BTreeSet<String>) -> Result<(), Diagnostic> {
        if changed.is_empty() {
            return Ok(());
        }
        let saved_scopes = std::mem::take(&mut self.scopes);
        let saved_source = self.source.clone();
        let saved_call_depth = std::mem::replace(&mut self.call_depth, 0);
        let result = async {
            // Update all conditions before running handlers, in registration order.
            for index in 0..self.prophecies.len() {
                let prophecy = self.prophecies[index].clone();
                if prophecy.dependencies.is_disjoint(changed) {
                    continue;
                }
                self.source = prophecy.source.clone();
                let value = self.eval(&prophecy.condition).await?;
                let now = self.boolean(value, &prophecy.condition.span)?;
                if now && !self.prophecy_values[index] {
                    if self.pending.len() >= MAX_PENDING_EVENTS {
                        return Err(self.error(
                            &prophecy.condition.span,
                            "pending event limit reached (4096 events)",
                        ));
                    }
                    self.pending.push_back(index);
                }
                self.prophecy_values[index] = now;
            }
            if !self.dispatching {
                self.dispatching = true;
                while let Some(index) = self.pending.pop_front() {
                    let prophecy = self.prophecies[index].clone();
                    self.source = prophecy.source.clone();
                    self.tick(&prophecy.condition.span)?;
                    self.block(&prophecy.body, true).await?;
                }
                self.dispatching = false;
            }
            Ok(())
        }
        .await;
        self.scopes = saved_scopes;
        self.source = saved_source;
        self.call_depth = saved_call_depth;
        result
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run(source: &str) -> Execution {
    Interpreter::new().execute_program(source)
}

/// Native operations, including sleep, complete during the first poll. Keeping
/// this adapter native-only prevents accidentally blocking a browser event loop.
#[cfg(not(target_arch = "wasm32"))]
fn complete_native(future: impl std::future::Future<Output = Execution>) -> Execution {
    use std::task::{Context, Poll, Waker};
    match std::pin::pin!(future)
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(result) => result,
        Poll::Pending => unreachable!("native interpreter operations are synchronous"),
    }
}
