//! AST Validation and Dangerous Pattern Detection
//!
//! This module provides pattern detection for Python harness code using
//! proper AST parsing via rustpython-parser. It detects potentially dangerous
//! patterns (shell injection, network, filesystem, dangerous builtins)
//! without blocking execution - patterns are reported for LLM review.
//!
//! NOTE: "tout autorisé" for imports - we DETECT but don't BLOCK.
//! The harness code will be reviewed by LLM for anti-cheat.

use crate::error::{DataFabricationError, Result};
use rustpython_parser::ast::{self, Expr, Ranged, Stmt, Suite};
use rustpython_parser::Parse;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Severity level for dangerous patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    /// Critical: exec, eval, __import__, code execution possible
    Critical,
    /// Warning: potentially unsafe operation
    Warning,
    /// Informational: best practice violation
    Info,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Critical => write!(f, "critical"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
        }
    }
}

/// A security violation detected in Python code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityViolation {
    /// The pattern or builtin that was detected
    pub pattern: String,
    /// Severity of the violation
    pub severity: Severity,
    /// Line number (1-indexed) where the violation was found
    pub line: Option<usize>,
    /// Column number (0-indexed) where the violation was found
    pub column: Option<usize>,
}

impl SecurityViolation {
    /// Create a new security violation.
    pub fn new(
        pattern: impl Into<String>,
        severity: Severity,
        line: Option<usize>,
        column: Option<usize>,
    ) -> Self {
        Self {
            pattern: pattern.into(),
            severity,
            line,
            column,
        }
    }
}

impl fmt::Display for SecurityViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.line, self.column) {
            (Some(line), Some(col)) => {
                write!(
                    f,
                    "Security violation ({}) at line {} col {}: {}",
                    self.severity, line, col, self.pattern
                )
            }
            (Some(line), None) => {
                write!(
                    f,
                    "Security violation ({}) at line {}: {}",
                    self.severity, line, self.pattern
                )
            }
            _ => {
                write!(
                    f,
                    "Security violation ({}): {}",
                    self.severity, self.pattern
                )
            }
        }
    }
}

/// Dangerous Python builtins that can execute arbitrary code.
const DANGEROUS_BUILTINS_CRITICAL: &[&str] = &["exec", "eval", "__import__"];

/// Potentially unsafe Python builtins.
const DANGEROUS_BUILTINS_WARNING: &[&str] = &["compile"];

/// Critical shell injection patterns (module.attr calls).
const SHELL_PATTERNS_CRITICAL: &[(&str, &str)] = &[("os", "system"), ("os", "popen")];

/// Warning shell patterns.
const SHELL_PATTERNS_WARNING: &[(&str, &str)] = &[
    ("subprocess", "run"),
    ("subprocess", "call"),
    ("subprocess", "Popen"),
    ("subprocess", "check_output"),
    ("subprocess", "check_call"),
];

/// Warning network patterns.
const NETWORK_PATTERNS_WARNING: &[(&str, &str)] =
    &[("socket", "socket"), ("socket", "create_connection")];

/// Info network patterns (HTTP clients - don't block, just inform).
const NETWORK_PATTERNS_INFO: &[(&str, &str)] = &[
    ("urllib", "request"),
    ("requests", "get"),
    ("requests", "post"),
    ("requests", "put"),
    ("requests", "delete"),
    ("requests", "request"),
    ("httpx", "get"),
    ("httpx", "post"),
];

/// Warning filesystem patterns.
const FILESYSTEM_PATTERNS_WARNING: &[(&str, &str)] = &[("shutil", "rmtree"), ("os", "remove")];

/// Info filesystem patterns.
const FILESYSTEM_PATTERNS_INFO: &[(&str, &str)] = &[("os", "chdir"), ("pathlib", "Path")];

/// Validate Python source code for security violations.
///
/// This function parses the Python code using rustpython-parser and walks
/// the AST to detect:
/// - Dangerous builtin calls: exec(), eval(), __import__() (Critical)
/// - Potentially unsafe calls: compile() (Warning)
///
/// Note: Import statements are NOT blocked per policy ("tout autorisé").
/// Import validation should be done via LLM review.
///
/// # Arguments
/// * `source` - The Python source code to validate
///
/// # Returns
/// * `Ok(Vec<SecurityViolation>)` - List of violations found (empty if safe)
/// * `Err(DataFabricationError)` - Parse error or other error
///
/// # Example
/// ```ignore
/// use data_fabrication_core::ast_validation::validate_python_code;
///
/// let code = r#"
/// x = eval("1 + 1")
/// "#;
/// let violations = validate_python_code(code)?;
/// assert!(!violations.is_empty());
/// assert_eq!(violations[0].pattern, "eval");
/// ```
pub fn validate_python_code(source: &str) -> Result<Vec<SecurityViolation>> {
    let mut violations = Vec::new();

    // Parse the Python source code using rustpython-parser
    let ast =
        Suite::parse(source, "<embedded>").map_err(|e| DataFabricationError::SchemaError {
            message: format!("Failed to parse Python code: {}", e),
            line: None,
        })?;

    // Walk the AST and check for violations
    walk_statements(&ast, source, &mut violations);

    Ok(violations)
}

/// Walk through statements and check for security violations.
fn walk_statements(statements: &[Stmt], source: &str, violations: &mut Vec<SecurityViolation>) {
    for stmt in statements {
        check_statement(stmt, source, violations);
    }
}

/// Check a single statement for security violations.
fn check_statement(stmt: &Stmt, source: &str, violations: &mut Vec<SecurityViolation>) {
    match stmt {
        // Expression statements - check for dangerous function calls
        Stmt::Expr(expr_stmt) => {
            check_expression(&expr_stmt.value, source, violations);
        }

        // Assignment - check the value being assigned
        Stmt::Assign(assign_stmt) => {
            check_expression(&assign_stmt.value, source, violations);
        }

        // Augmented assignment (e.g., x += 1)
        Stmt::AugAssign(aug_assign) => {
            check_expression(&aug_assign.value, source, violations);
        }

        // AnnAssign (type-annotated assignment)
        Stmt::AnnAssign(ann_assign) => {
            if let Some(value) = &ann_assign.value {
                check_expression(value, source, violations);
            }
        }

        // Function definitions - check defaults and body
        Stmt::FunctionDef(func_def) => {
            check_function_args(&func_def.args, source, violations);
            walk_statements(&func_def.body, source, violations);
            for decorator in &func_def.decorator_list {
                check_expression(decorator, source, violations);
            }
        }

        // Async function definitions
        Stmt::AsyncFunctionDef(async_func_def) => {
            check_function_args(&async_func_def.args, source, violations);
            walk_statements(&async_func_def.body, source, violations);
            for decorator in &async_func_def.decorator_list {
                check_expression(decorator, source, violations);
            }
        }

        // Class definitions - check body and decorators
        Stmt::ClassDef(class_def) => {
            walk_statements(&class_def.body, source, violations);
            for decorator in &class_def.decorator_list {
                check_expression(decorator, source, violations);
            }
            for base in &class_def.bases {
                check_expression(base, source, violations);
            }
        }

        // If statements
        Stmt::If(if_stmt) => {
            check_expression(&if_stmt.test, source, violations);
            walk_statements(&if_stmt.body, source, violations);
            walk_statements(&if_stmt.orelse, source, violations);
        }

        // While loops
        Stmt::While(while_stmt) => {
            check_expression(&while_stmt.test, source, violations);
            walk_statements(&while_stmt.body, source, violations);
            walk_statements(&while_stmt.orelse, source, violations);
        }

        // For loops
        Stmt::For(for_stmt) => {
            check_expression(&for_stmt.iter, source, violations);
            walk_statements(&for_stmt.body, source, violations);
            walk_statements(&for_stmt.orelse, source, violations);
        }

        // Async for loops
        Stmt::AsyncFor(async_for) => {
            check_expression(&async_for.iter, source, violations);
            walk_statements(&async_for.body, source, violations);
            walk_statements(&async_for.orelse, source, violations);
        }

        // Return statements
        Stmt::Return(return_stmt) => {
            if let Some(value) = &return_stmt.value {
                check_expression(value, source, violations);
            }
        }

        // Raise statements
        Stmt::Raise(raise_stmt) => {
            if let Some(exc) = &raise_stmt.exc {
                check_expression(exc, source, violations);
            }
            if let Some(cause) = &raise_stmt.cause {
                check_expression(cause, source, violations);
            }
        }

        Stmt::Try(try_stmt) => {
            walk_statements(&try_stmt.body, source, violations);
            for handler in &try_stmt.handlers {
                if let ast::ExceptHandler::ExceptHandler(h) = handler {
                    walk_statements(&h.body, source, violations);
                }
            }
            walk_statements(&try_stmt.orelse, source, violations);
            walk_statements(&try_stmt.finalbody, source, violations);
        }

        Stmt::TryStar(try_star) => {
            walk_statements(&try_star.body, source, violations);
            for handler in &try_star.handlers {
                if let ast::ExceptHandler::ExceptHandler(h) = handler {
                    walk_statements(&h.body, source, violations);
                }
            }
            walk_statements(&try_star.orelse, source, violations);
            walk_statements(&try_star.finalbody, source, violations);
        }

        // With statements
        Stmt::With(with_stmt) => {
            for item in &with_stmt.items {
                check_expression(&item.context_expr, source, violations);
            }
            walk_statements(&with_stmt.body, source, violations);
        }

        // Async with statements
        Stmt::AsyncWith(async_with) => {
            for item in &async_with.items {
                check_expression(&item.context_expr, source, violations);
            }
            walk_statements(&async_with.body, source, violations);
        }

        // Match statements (Python 3.10+)
        Stmt::Match(match_stmt) => {
            check_expression(&match_stmt.subject, source, violations);
            for case in &match_stmt.cases {
                walk_statements(&case.body, source, violations);
                if let Some(guard) = &case.guard {
                    check_expression(guard, source, violations);
                }
            }
        }

        // Assert statements
        Stmt::Assert(assert_stmt) => {
            check_expression(&assert_stmt.test, source, violations);
            if let Some(msg) = &assert_stmt.msg {
                check_expression(msg, source, violations);
            }
        }

        // TypeAlias statements (Python 3.12+)
        Stmt::TypeAlias(type_alias) => {
            check_expression(&type_alias.value, source, violations);
        }

        // Delete statements
        Stmt::Delete(delete_stmt) => {
            for target in &delete_stmt.targets {
                check_expression(target, source, violations);
            }
        }

        _ => {}
    }
}

fn check_function_args(
    args: &ast::Arguments,
    source: &str,
    violations: &mut Vec<SecurityViolation>,
) {
    for arg in &args.args {
        if let Some(default) = &arg.default {
            check_expression(default, source, violations);
        }
    }
    for kwonly_arg in &args.kwonlyargs {
        if let Some(default) = &kwonly_arg.default {
            check_expression(default, source, violations);
        }
    }
}

/// Check an expression for security violations.
fn check_expression(expr: &Expr, source: &str, violations: &mut Vec<SecurityViolation>) {
    match expr {
        // Function call - check for dangerous builtins
        Expr::Call(call) => {
            check_dangerous_call(&call.func, source, violations);
            // Check for getattr() bypass with dangerous targets
            check_getattr_bypass(call, source, violations);
            // Check arguments
            for arg in &call.args {
                check_expression(arg, source, violations);
            }
            for kw in &call.keywords {
                check_expression(&kw.value, source, violations);
            }
        }

        // Attribute access - check base expression
        Expr::Attribute(attr) => {
            check_expression(&attr.value, source, violations);
        }

        // Binary operations
        Expr::BinOp(bin_op) => {
            check_expression(&bin_op.left, source, violations);
            check_expression(&bin_op.right, source, violations);
        }

        // Unary operations
        Expr::UnaryOp(unary_op) => {
            check_expression(&unary_op.operand, source, violations);
        }

        // Boolean operations (and, or)
        Expr::BoolOp(bool_op) => {
            for value in &bool_op.values {
                check_expression(value, source, violations);
            }
        }

        // Comparison operations
        Expr::Compare(compare) => {
            check_expression(&compare.left, source, violations);
            for comparator in &compare.comparators {
                check_expression(comparator, source, violations);
            }
        }

        // Lambda expressions
        Expr::Lambda(lambda) => {
            check_expression(&lambda.body, source, violations);
        }

        // If expression (ternary)
        Expr::IfExp(if_exp) => {
            check_expression(&if_exp.test, source, violations);
            check_expression(&if_exp.body, source, violations);
            check_expression(&if_exp.orelse, source, violations);
        }

        // List/Dict/Set comprehensions
        Expr::ListComp(list_comp) => {
            check_expression(&list_comp.elt, source, violations);
            for gen in &list_comp.generators {
                check_expression(&gen.iter, source, violations);
                for if_clause in &gen.ifs {
                    check_expression(if_clause, source, violations);
                }
            }
        }

        Expr::SetComp(set_comp) => {
            check_expression(&set_comp.elt, source, violations);
            for gen in &set_comp.generators {
                check_expression(&gen.iter, source, violations);
                for if_clause in &gen.ifs {
                    check_expression(if_clause, source, violations);
                }
            }
        }

        Expr::DictComp(dict_comp) => {
            check_expression(&dict_comp.key, source, violations);
            check_expression(&dict_comp.value, source, violations);
            for gen in &dict_comp.generators {
                check_expression(&gen.iter, source, violations);
                for if_clause in &gen.ifs {
                    check_expression(if_clause, source, violations);
                }
            }
        }

        Expr::GeneratorExp(gen_exp) => {
            check_expression(&gen_exp.elt, source, violations);
            for gen in &gen_exp.generators {
                check_expression(&gen.iter, source, violations);
                for if_clause in &gen.ifs {
                    check_expression(if_clause, source, violations);
                }
            }
        }

        // Await expression
        Expr::Await(await_expr) => {
            check_expression(&await_expr.value, source, violations);
        }

        // Yield expression
        Expr::Yield(yield_expr) => {
            if let Some(value) = &yield_expr.value {
                check_expression(value, source, violations);
            }
        }

        Expr::YieldFrom(yield_from) => {
            check_expression(&yield_from.value, source, violations);
        }

        // Named expression (walrus operator :=)
        Expr::NamedExpr(named_expr) => {
            check_expression(&named_expr.value, source, violations);
        }

        // Slice expression
        Expr::Slice(slice) => {
            if let Some(lower) = &slice.lower {
                check_expression(lower, source, violations);
            }
            if let Some(upper) = &slice.upper {
                check_expression(upper, source, violations);
            }
            if let Some(step) = &slice.step {
                check_expression(step, source, violations);
            }
        }

        // Subscript
        Expr::Subscript(subscript) => {
            check_expression(&subscript.value, source, violations);
            check_expression(&subscript.slice, source, violations);
        }

        // Starred expression
        Expr::Starred(starred) => {
            check_expression(&starred.value, source, violations);
        }

        // Tuple, List, Set
        Expr::Tuple(tuple) => {
            for elem in &tuple.elts {
                check_expression(elem, source, violations);
            }
        }

        Expr::List(list) => {
            for elem in &list.elts {
                check_expression(elem, source, violations);
            }
        }

        Expr::Set(set) => {
            for elem in &set.elts {
                check_expression(elem, source, violations);
            }
        }

        // Dict
        Expr::Dict(dict) => {
            for key in &dict.keys {
                if let Some(k) = key {
                    check_expression(k, source, violations);
                }
            }
            for value in &dict.values {
                check_expression(value, source, violations);
            }
        }

        // JoinedStr (f-string)
        Expr::JoinedStr(joined_str) => {
            for value in &joined_str.values {
                check_expression(value, source, violations);
            }
        }

        Expr::FormattedValue(formatted_value) => {
            check_expression(&formatted_value.value, source, violations);
            if let Some(format_spec) = &formatted_value.format_spec {
                check_expression(format_spec, source, violations);
            }
        }

        // Name, Constant - no dangerous calls in these
        Expr::Name(_) | Expr::Constant(_) => {}
    }
}

/// Check if a function call is to a dangerous builtin or module method.
fn check_dangerous_call(func: &Expr, source: &str, violations: &mut Vec<SecurityViolation>) {
    match func {
        Expr::Name(name) => {
            let func_name = name.id.as_str();

            for builtin in DANGEROUS_BUILTINS_CRITICAL {
                if func_name == *builtin {
                    let (line, column) = get_location(func, source);
                    violations.push(SecurityViolation::new(
                        func_name.to_string(),
                        Severity::Critical,
                        line,
                        column,
                    ));
                    return;
                }
            }

            for builtin in DANGEROUS_BUILTINS_WARNING {
                if func_name == *builtin {
                    let (line, column) = get_location(func, source);
                    violations.push(SecurityViolation::new(
                        func_name.to_string(),
                        Severity::Warning,
                        line,
                        column,
                    ));
                    return;
                }
            }
        }

        Expr::Attribute(attr) => {
            if let Some((module_name, attr_name)) = extract_module_attr(attr) {
                let full_name = format!("{}.{}", module_name, attr_name);

                for (mod_name, method) in SHELL_PATTERNS_CRITICAL {
                    if module_name == *mod_name && attr_name == *method {
                        let (line, column) = get_location(func, source);
                        violations.push(SecurityViolation::new(
                            full_name,
                            Severity::Critical,
                            line,
                            column,
                        ));
                        return;
                    }
                }

                for (mod_name, method) in SHELL_PATTERNS_WARNING {
                    if module_name == *mod_name && attr_name == *method {
                        let (line, column) = get_location(func, source);
                        violations.push(SecurityViolation::new(
                            full_name,
                            Severity::Warning,
                            line,
                            column,
                        ));
                        return;
                    }
                }

                for (mod_name, method) in NETWORK_PATTERNS_WARNING {
                    if module_name == *mod_name && attr_name == *method {
                        let (line, column) = get_location(func, source);
                        violations.push(SecurityViolation::new(
                            full_name,
                            Severity::Warning,
                            line,
                            column,
                        ));
                        return;
                    }
                }

                for (mod_name, method) in NETWORK_PATTERNS_INFO {
                    if module_name == *mod_name && attr_name == *method {
                        let (line, column) = get_location(func, source);
                        violations.push(SecurityViolation::new(
                            full_name,
                            Severity::Info,
                            line,
                            column,
                        ));
                        return;
                    }
                }

                for (mod_name, method) in FILESYSTEM_PATTERNS_WARNING {
                    if module_name == *mod_name && attr_name == *method {
                        let (line, column) = get_location(func, source);
                        violations.push(SecurityViolation::new(
                            full_name,
                            Severity::Warning,
                            line,
                            column,
                        ));
                        return;
                    }
                }

                for (mod_name, method) in FILESYSTEM_PATTERNS_INFO {
                    if module_name == *mod_name && attr_name == *method {
                        let (line, column) = get_location(func, source);
                        violations.push(SecurityViolation::new(
                            full_name,
                            Severity::Info,
                            line,
                            column,
                        ));
                        return;
                    }
                }
            }
        }

        _ => {}
    }
}

/// Extract module and attribute name from an Attribute expression.
fn extract_module_attr(attr: &ast::ExprAttribute) -> Option<(String, String)> {
    let attr_name = attr.attr.as_str().to_string();

    match &*attr.value {
        Expr::Name(module_name) => Some((module_name.id.as_str().to_string(), attr_name)),
        Expr::Attribute(inner_attr) => {
            extract_module_attr(inner_attr).map(|(base_module, _)| (base_module, attr_name))
        }
        _ => None,
    }
}

/// Check for getattr() bypass attempts that access dangerous functions.
fn check_getattr_bypass(
    call: &ast::ExprCall,
    source: &str,
    violations: &mut Vec<SecurityViolation>,
) {
    if !matches!(&*call.func, Expr::Name(name) if name.id.as_str() == "getattr") {
        return;
    }

    if call.args.len() < 2 {
        return;
    }

    let module_name = match &call.args[0] {
        Expr::Name(name) => name.id.as_str().to_string(),
        _ => return,
    };

    let attr_name = match &call.args[1] {
        Expr::Constant(constant) => {
            if let ast::Constant::Str(s) = &constant.value {
                s.to_string()
            } else {
                return;
            }
        }
        _ => return,
    };

    if module_name == "__builtins__" {
        for builtin in DANGEROUS_BUILTINS_CRITICAL {
            if attr_name == *builtin {
                let (line, column) = get_location(&call.func, source);
                violations.push(SecurityViolation::new(
                    format!("getattr(__builtins__, '{}')", attr_name),
                    Severity::Critical,
                    line,
                    column,
                ));
                return;
            }
        }
        for builtin in DANGEROUS_BUILTINS_WARNING {
            if attr_name == *builtin {
                let (line, column) = get_location(&call.func, source);
                violations.push(SecurityViolation::new(
                    format!("getattr(__builtins__, '{}')", attr_name),
                    Severity::Warning,
                    line,
                    column,
                ));
                return;
            }
        }
    }

    for (mod_name, method) in SHELL_PATTERNS_CRITICAL {
        if module_name == *mod_name && attr_name == *method {
            let (line, column) = get_location(&call.func, source);
            violations.push(SecurityViolation::new(
                format!("getattr({}, '{}')", module_name, attr_name),
                Severity::Critical,
                line,
                column,
            ));
            return;
        }
    }

    for (mod_name, method) in SHELL_PATTERNS_WARNING {
        if module_name == *mod_name && attr_name == *method {
            let (line, column) = get_location(&call.func, source);
            violations.push(SecurityViolation::new(
                format!("getattr({}, '{}')", module_name, attr_name),
                Severity::Warning,
                line,
                column,
            ));
            return;
        }
    }

    for (mod_name, method) in NETWORK_PATTERNS_WARNING {
        if module_name == *mod_name && attr_name == *method {
            let (line, column) = get_location(&call.func, source);
            violations.push(SecurityViolation::new(
                format!("getattr({}, '{}')", module_name, attr_name),
                Severity::Warning,
                line,
                column,
            ));
            return;
        }
    }

    for (mod_name, method) in NETWORK_PATTERNS_INFO {
        if module_name == *mod_name && attr_name == *method {
            let (line, column) = get_location(&call.func, source);
            violations.push(SecurityViolation::new(
                format!("getattr({}, '{}')", module_name, attr_name),
                Severity::Info,
                line,
                column,
            ));
            return;
        }
    }

    for (mod_name, method) in FILESYSTEM_PATTERNS_WARNING {
        if module_name == *mod_name && attr_name == *method {
            let (line, column) = get_location(&call.func, source);
            violations.push(SecurityViolation::new(
                format!("getattr({}, '{}')", module_name, attr_name),
                Severity::Warning,
                line,
                column,
            ));
            return;
        }
    }

    for (mod_name, method) in FILESYSTEM_PATTERNS_INFO {
        if module_name == *mod_name && attr_name == *method {
            let (line, column) = get_location(&call.func, source);
            violations.push(SecurityViolation::new(
                format!("getattr({}, '{}')", module_name, attr_name),
                Severity::Info,
                line,
                column,
            ));
            return;
        }
    }
}

/// Get the line and column for an expression from byte offset.
fn get_location(expr: &Expr, source: &str) -> (Option<usize>, Option<usize>) {
    use rustpython_parser::text_size::TextSize;

    let range = expr.range();
    let start_offset: TextSize = range.start();

    let mut line = 1usize;
    let mut col = 0usize;
    let mut byte_pos: usize = 0;
    let target_pos: usize = start_offset.into();

    for ch in source.chars() {
        if byte_pos >= target_pos {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
        byte_pos += ch.len_utf8();
    }

    (Some(line), Some(col))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_safe_code_passes() {
        let code = r#"
def hello():
    print("Hello, world!")
    return 42
"#;
        let violations = validate_python_code(code).unwrap();
        assert!(
            violations.is_empty(),
            "Expected no violations, got: {:?}",
            violations
        );
    }

    #[test]
    fn test_ast_exec_detected_as_critical() {
        let code = r#"
exec("print('hello')")
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "exec");
        assert_eq!(violations[0].severity, Severity::Critical);
        assert!(violations[0].line.is_some());
    }

    #[test]
    fn test_ast_eval_detected_as_critical() {
        let code = r#"
result = eval("1 + 1")
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "eval");
        assert_eq!(violations[0].severity, Severity::Critical);
    }

    #[test]
    fn test_ast_compile_detected_as_warning() {
        let code = r#"
code = compile("x = 1", "<string>", "exec")
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "compile");
        assert_eq!(violations[0].severity, Severity::Warning);
    }

    #[test]
    fn test_ast_import_detected_as_critical() {
        let code = r#"
mod = __import__("os")
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "__import__");
        assert_eq!(violations[0].severity, Severity::Critical);
    }

    #[test]
    fn test_ast_multiple_violations() {
        let code = r#"
x = eval("1 + 1")
exec("print('hello')")
code = compile("x = 1", "<string>", "exec")
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 3);

        // Check that all violations are detected
        let patterns: Vec<&str> = violations.iter().map(|v| v.pattern.as_str()).collect();
        assert!(patterns.contains(&"eval"));
        assert!(patterns.contains(&"exec"));
        assert!(patterns.contains(&"compile"));

        // Check severities
        for v in &violations {
            if v.pattern == "compile" {
                assert_eq!(v.severity, Severity::Warning);
            } else {
                assert_eq!(v.severity, Severity::Critical);
            }
        }
    }

    #[test]
    fn test_ast_dangerous_call_in_nested_context() {
        let code = r#"
def foo():
    x = [eval("1") for i in range(10)]
    return x
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "eval");
        assert_eq!(violations[0].severity, Severity::Critical);
    }

    #[test]
    fn test_ast_import_allowed() {
        // Imports should be allowed per policy ("tout autorisé")
        let code = r#"
import os
import subprocess
from sys import path
"#;
        let violations = validate_python_code(code).unwrap();
        assert!(
            violations.is_empty(),
            "Imports should be allowed, got violations: {:?}",
            violations
        );
    }

    #[test]
    fn test_ast_dangerous_call_in_class() {
        let code = r#"
class Foo:
    def dangerous(self):
        exec("print('bad')")
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "exec");
        assert_eq!(violations[0].severity, Severity::Critical);
    }

    #[test]
    fn test_ast_dangerous_call_in_lambda() {
        let code = r#"
fn = lambda: eval("1 + 1")
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "eval");
        assert_eq!(violations[0].severity, Severity::Critical);
    }

    #[test]
    fn test_ast_dangerous_call_in_default_arg() {
        let code = r#"
def foo(x=eval("1")):
    return x
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "eval");
        assert_eq!(violations[0].severity, Severity::Critical);
    }

    #[test]
    fn test_ast_valid_parse_error() {
        let code = r#"
def foo(:
    return 1
"#;
        let result = validate_python_code(code);
        assert!(result.is_err());
    }

    #[test]
    fn test_ast_security_violation_display() {
        let v = SecurityViolation::new("exec", Severity::Critical, Some(10), Some(5));
        assert_eq!(
            format!("{}", v),
            "Security violation (critical) at line 10 col 5: exec"
        );

        let v2 = SecurityViolation::new("eval", Severity::Warning, Some(20), None);
        assert_eq!(
            format!("{}", v2),
            "Security violation (warning) at line 20: eval"
        );

        let v3 = SecurityViolation::new("compile", Severity::Info, None, None);
        assert_eq!(format!("{}", v3), "Security violation (info): compile");
    }

    #[test]
    fn test_ast_severity_display() {
        assert_eq!(format!("{}", Severity::Critical), "critical");
        assert_eq!(format!("{}", Severity::Warning), "warning");
        assert_eq!(format!("{}", Severity::Info), "info");
    }

    #[test]
    fn test_ast_dangerous_in_dict() {
        let code = r#"
x = {"key": eval("value")}
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "eval");
    }

    #[test]
    fn test_ast_dangerous_in_decorator() {
        let code = r#"
@decorator(eval("test"))
def foo():
    pass
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "eval");
    }

    #[test]
    fn test_ast_dangerous_in_try_except() {
        let code = r#"
try:
    x = eval("1")
except:
    exec("y = 2")
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_ast_dangerous_in_comprehension_condition() {
        let code = r#"
x = [i for i in range(10) if eval("i > 5")]
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "eval");
    }

    #[test]
    fn test_ast_os_system_detected_as_critical() {
        let code = r#"
import os
os.system("ls -la")
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "os.system");
        assert_eq!(violations[0].severity, Severity::Critical);
        assert!(violations[0].line.is_some());
    }

    #[test]
    fn test_ast_os_popen_detected_as_critical() {
        let code = r#"
import os
os.popen("ls")
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "os.popen");
        assert_eq!(violations[0].severity, Severity::Critical);
    }

    #[test]
    fn test_ast_subprocess_run_detected_as_warning() {
        let code = r#"
import subprocess
subprocess.run(["ls"])
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "subprocess.run");
        assert_eq!(violations[0].severity, Severity::Warning);
    }

    #[test]
    fn test_ast_subprocess_call_detected_as_warning() {
        let code = r#"
import subprocess
subprocess.call(["echo", "test"])
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "subprocess.call");
        assert_eq!(violations[0].severity, Severity::Warning);
    }

    #[test]
    fn test_ast_socket_socket_detected_as_warning() {
        let code = r#"
import socket
s = socket.socket()
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "socket.socket");
        assert_eq!(violations[0].severity, Severity::Warning);
    }

    #[test]
    fn test_ast_requests_get_detected_as_info() {
        let code = r#"
import requests
requests.get("http://example.com")
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "requests.get");
        assert_eq!(violations[0].severity, Severity::Info);
    }

    #[test]
    fn test_ast_shutil_rmtree_detected_as_warning() {
        let code = r#"
import shutil
shutil.rmtree("/path/to/dir")
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "shutil.rmtree");
        assert_eq!(violations[0].severity, Severity::Warning);
    }

    #[test]
    fn test_ast_getattr_builtin_exec_detected_as_critical() {
        let code = r#"
fn = getattr(__builtins__, 'exec')
fn("print('hello')")
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert!(violations[0].pattern.contains("getattr"));
        assert!(violations[0].pattern.contains("exec"));
        assert_eq!(violations[0].severity, Severity::Critical);
    }

    #[test]
    fn test_ast_getattr_os_system_detected_as_critical() {
        let code = r#"
import os
fn = getattr(os, 'system')
fn("ls")
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert!(violations[0].pattern.contains("getattr"));
        assert!(violations[0].pattern.contains("os"));
        assert!(violations[0].pattern.contains("system"));
        assert_eq!(violations[0].severity, Severity::Critical);
    }

    #[test]
    fn test_ast_getattr_subprocess_run_detected_as_warning() {
        let code = r#"
import subprocess
fn = getattr(subprocess, 'run')
fn(["ls"])
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 1);
        assert!(violations[0].pattern.contains("getattr"));
        assert!(violations[0].pattern.contains("subprocess"));
        assert!(violations[0].pattern.contains("run"));
        assert_eq!(violations[0].severity, Severity::Warning);
    }

    #[test]
    fn test_ast_getattr_dynamic_not_flagged() {
        let code = r#"
import os
attr_name = "something_safe"
fn = getattr(os, attr_name)
"#;
        let violations = validate_python_code(code).unwrap();
        assert!(
            violations.is_empty(),
            "Dynamic getattr should not be flagged"
        );
    }

    #[test]
    fn test_ast_module_attr_combined_patterns() {
        let code = r#"
import os
import subprocess
import socket

os.system("ls")
subprocess.run(["cat", "file.txt"])
socket.socket()
"#;
        let violations = validate_python_code(code).unwrap();
        assert_eq!(violations.len(), 3);

        let patterns: Vec<&str> = violations.iter().map(|v| v.pattern.as_str()).collect();
        assert!(patterns.contains(&"os.system"));
        assert!(patterns.contains(&"subprocess.run"));
        assert!(patterns.contains(&"socket.socket"));

        for v in &violations {
            if v.pattern == "os.system" {
                assert_eq!(v.severity, Severity::Critical);
            } else {
                assert_eq!(v.severity, Severity::Warning);
            }
        }
    }
}
