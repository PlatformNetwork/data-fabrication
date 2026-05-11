"""Static harness review before sandboxed execution."""

from __future__ import annotations

import ast
import base64
import hashlib
import json
import re
from dataclasses import asdict, dataclass, field
from pathlib import Path

from .ast_similarity import hash_structure, normalize_ast
from .ast_validation import SecurityViolation, Severity, validate_python_code

SUSPICIOUS_TEXT = (
    "/etc/passwd",
    "/proc/",
    "DATA_FABRICATION_SHARED_TOKEN",
    "CHALLENGE_SHARED_TOKEN",
    "BEGIN PRIVATE KEY",
)


@dataclass(frozen=True)
class FileReview:
    path: str
    structure_hash: str | None
    functions: list[str]
    imports: list[str]
    violations: list[SecurityViolation]
    suspicious_patterns: list[str] = field(default_factory=list)
    line_count: int = 0


@dataclass(frozen=True)
class StaticReviewReport:
    files: list[FileReview]
    critical_count: int
    warning_count: int
    suspicious_count: int
    harness_hash: str
    accepted: bool

    def to_dict(self) -> dict[str, object]:
        return {
            "files": [
                {
                    **asdict(file),
                    "violations": [
                        {
                            "pattern": violation.pattern,
                            "severity": violation.severity.value,
                            "line": violation.line,
                            "column": violation.column,
                        }
                        for violation in file.violations
                    ],
                }
                for file in self.files
            ],
            "critical_count": self.critical_count,
            "warning_count": self.warning_count,
            "suspicious_count": self.suspicious_count,
            "harness_hash": self.harness_hash,
            "accepted": self.accepted,
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), separators=(",", ":"))


def review_workspace(workspace_path: Path) -> StaticReviewReport:
    """Review all Python files in an extracted harness workspace."""

    reviews: list[FileReview] = []
    for path in sorted(workspace_path.rglob("*.py")):
        source = path.read_text(encoding="utf-8", errors="replace")
        relative = path.relative_to(workspace_path).as_posix()
        violations = _validate_source(source)
        suspicious = _suspicious_patterns(source)
        functions, imports = _symbols(source)
        structure_hash: str | None = None
        try:
            structure_hash = hash_structure(normalize_ast(source))
        except SyntaxError:
            violations.append(SecurityViolation("syntax_error", Severity.critical))
        reviews.append(
            FileReview(
                path=relative,
                structure_hash=structure_hash,
                functions=functions,
                imports=imports,
                violations=violations,
                suspicious_patterns=suspicious,
                line_count=len(source.splitlines()),
            )
        )

    critical_count = sum(
        1
        for review in reviews
        for violation in review.violations
        if violation.severity == Severity.critical
    )
    warning_count = sum(
        1
        for review in reviews
        for violation in review.violations
        if violation.severity == Severity.warning
    )
    suspicious_count = sum(len(review.suspicious_patterns) for review in reviews)
    harness_hash = _harness_hash(reviews)
    return StaticReviewReport(
        files=reviews,
        critical_count=critical_count,
        warning_count=warning_count,
        suspicious_count=suspicious_count,
        harness_hash=harness_hash,
        accepted=critical_count == 0 and suspicious_count == 0,
    )


def _validate_source(source: str) -> list[SecurityViolation]:
    try:
        return validate_python_code(source)
    except SyntaxError:
        return [SecurityViolation("syntax_error", Severity.critical)]


def _symbols(source: str) -> tuple[list[str], list[str]]:
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return [], []
    functions = [
        node.name
        for node in ast.walk(tree)
        if isinstance(node, ast.FunctionDef | ast.AsyncFunctionDef)
    ]
    imports: list[str] = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            imports.extend(alias.name for alias in node.names)
        elif isinstance(node, ast.ImportFrom) and node.module:
            imports.append(node.module)
    return sorted(functions), sorted(set(imports))


def _suspicious_patterns(source: str) -> list[str]:
    patterns = [pattern for pattern in SUSPICIOUS_TEXT if pattern in source]
    for candidate in re.findall(r"[A-Za-z0-9+/=]{512,}", source):
        try:
            decoded = base64.b64decode(candidate, validate=True)
            if len(decoded) > 256 and decoded.count(b"\x00") < 3:
                patterns.append("large_base64_blob")
                break
        except Exception:
            continue
    return patterns


def _harness_hash(reviews: list[FileReview]) -> str:
    pieces = [
        f"{review.path}:{review.structure_hash or ''}:{','.join(review.functions)}"
        for review in reviews
    ]
    return hashlib.sha256("\n".join(pieces).encode("utf-8")).hexdigest()
