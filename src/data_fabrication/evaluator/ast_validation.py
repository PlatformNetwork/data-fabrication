"""Python AST validation and dangerous pattern detection."""

from __future__ import annotations

import ast
from dataclasses import dataclass
from enum import StrEnum


class Severity(StrEnum):
    critical = "critical"
    warning = "warning"
    info = "info"


@dataclass(frozen=True)
class SecurityViolation:
    pattern: str
    severity: Severity
    line: int | None = None
    column: int | None = None


CRITICAL_BUILTINS = {"exec", "eval", "__import__"}
WARNING_BUILTINS = {"compile"}
CRITICAL_CALLS = {("os", "system"), ("os", "popen")}
WARNING_CALLS = {
    ("subprocess", "run"),
    ("subprocess", "call"),
    ("subprocess", "Popen"),
    ("subprocess", "check_output"),
    ("subprocess", "check_call"),
    ("socket", "socket"),
    ("socket", "create_connection"),
    ("shutil", "rmtree"),
    ("os", "remove"),
}
INFO_CALLS = {("urllib", "request"), ("requests", "get"), ("httpx", "get")}


def validate_python_code(source: str) -> list[SecurityViolation]:
    """Parse Python source and report dangerous calls."""

    tree = ast.parse(source)
    visitor = _SecurityVisitor()
    visitor.visit(tree)
    return visitor.violations


def has_critical_violation(source: str) -> bool:
    return any(v.severity == Severity.critical for v in validate_python_code(source))


class _SecurityVisitor(ast.NodeVisitor):
    def __init__(self) -> None:
        self.violations: list[SecurityViolation] = []

    def visit_Call(self, node: ast.Call) -> None:
        self._check_call(node)
        self.generic_visit(node)

    def _check_call(self, node: ast.Call) -> None:
        name = _call_name(node.func)
        if not name:
            return
        if name in CRITICAL_BUILTINS:
            self._add(name, Severity.critical, node)
            return
        if name in WARNING_BUILTINS:
            self._add(name, Severity.warning, node)
            return
        if name == "getattr":
            self._check_getattr(node)
            return
        parts = tuple(name.split("."))
        if len(parts) >= 2:
            pair = (parts[0], parts[-1])
            if pair in CRITICAL_CALLS:
                self._add(name, Severity.critical, node)
            elif pair in WARNING_CALLS:
                self._add(name, Severity.warning, node)
            elif pair in INFO_CALLS:
                self._add(name, Severity.info, node)

    def _check_getattr(self, node: ast.Call) -> None:
        if len(node.args) < 2:
            return
        module = _call_name(node.args[0])
        attr = node.args[1].value if isinstance(node.args[1], ast.Constant) else None
        if not isinstance(module, str) or not isinstance(attr, str):
            return
        full = f"getattr({module}, {attr!r})"
        if module == "__builtins__" and attr in CRITICAL_BUILTINS:
            self._add(full, Severity.critical, node)
        elif module == "__builtins__" and attr in WARNING_BUILTINS:
            self._add(full, Severity.warning, node)
        elif (module, attr) in CRITICAL_CALLS:
            self._add(full, Severity.critical, node)
        elif (module, attr) in WARNING_CALLS:
            self._add(full, Severity.warning, node)

    def _add(self, pattern: str, severity: Severity, node: ast.AST) -> None:
        self.violations.append(
            SecurityViolation(
                pattern=pattern,
                severity=severity,
                line=getattr(node, "lineno", None),
                column=getattr(node, "col_offset", None),
            )
        )


def _call_name(node: ast.AST) -> str | None:
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        base = _call_name(node.value)
        return f"{base}.{node.attr}" if base else node.attr
    return None
