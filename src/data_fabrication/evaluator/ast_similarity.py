"""AST normalization and structural similarity for Python harness submissions."""

from __future__ import annotations

import ast
import builtins
from dataclasses import dataclass
from enum import StrEnum
from hashlib import sha256

BUILTINS = set(dir(builtins)) | {
    "None",
    "True",
    "False",
    "if",
    "else",
    "elif",
    "for",
    "while",
    "def",
    "class",
    "return",
    "yield",
}


class PlagiarismStatus(StrEnum):
    clean = "clean"
    needs_llm_verification = "needs_llm_verification"
    plagiarized = "plagiarized"


@dataclass(frozen=True)
class NormalizedAst:
    source: str
    node_sequence: list[str]


@dataclass(frozen=True)
class ComparisonResult:
    left_index: int
    right_index: int
    score: int
    status: PlagiarismStatus


@dataclass(frozen=True)
class PlagiarismReport:
    comparisons: list[ComparisonResult]
    suspicious_indices: list[int]
    plagiarized_indices: list[int]


def normalize_ast(source: str) -> NormalizedAst:
    tree = ast.parse(source)
    normalizer = _AstNormalizer()
    normalizer.visit(tree)
    return NormalizedAst(source=source, node_sequence=normalizer.node_sequence)


def hash_structure(normalized: NormalizedAst) -> str:
    digest = sha256()
    for node in normalized.node_sequence:
        digest.update(node.encode("utf-8"))
        digest.update(b"\0")
    return digest.hexdigest()


def compare_structures(left: NormalizedAst, right: NormalizedAst) -> int:
    if not left.node_sequence and not right.node_sequence:
        return 100
    if not left.node_sequence or not right.node_sequence:
        return 0
    lcs = _lcs_length(left.node_sequence, right.node_sequence)
    return min(100, int(lcs / max(len(left.node_sequence), len(right.node_sequence)) * 100))


def plagiarism_status(score: int) -> PlagiarismStatus:
    if score >= 97:
        return PlagiarismStatus.plagiarized
    if score >= 30:
        return PlagiarismStatus.needs_llm_verification
    return PlagiarismStatus.clean


def check_plagiarism(sources: list[str]) -> PlagiarismReport:
    normalized = [normalize_ast(source) for source in sources]
    comparisons: list[ComparisonResult] = []
    suspicious: set[int] = set()
    plagiarized: set[int] = set()
    for i, left in enumerate(normalized):
        for j, right in enumerate(normalized[i + 1 :], start=i + 1):
            score = compare_structures(left, right)
            status = plagiarism_status(score)
            comparisons.append(ComparisonResult(i, j, score, status))
            if status == PlagiarismStatus.plagiarized:
                plagiarized.update({i, j})
            elif status == PlagiarismStatus.needs_llm_verification:
                suspicious.update({i, j})
    return PlagiarismReport(
        comparisons=comparisons,
        suspicious_indices=sorted(suspicious),
        plagiarized_indices=sorted(plagiarized),
    )


class _AstNormalizer(ast.NodeVisitor):
    def __init__(self) -> None:
        self.var_map: dict[str, str] = {}
        self.var_counter = 0
        self.node_sequence: list[str] = []

    def generic_visit(self, node: ast.AST) -> None:
        self.node_sequence.append(type(node).__name__)
        super().generic_visit(node)

    def visit_Name(self, node: ast.Name) -> None:
        self.node_sequence.append(f"Name({self._normalize_name(node.id)})")

    def visit_arg(self, node: ast.arg) -> None:
        self.node_sequence.append(f"Arg({self._normalize_name(node.arg)})")
        self.generic_visit(node)

    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
        self.node_sequence.append("FunctionDef")
        self.node_sequence.append(self._normalize_name(node.name))
        self.generic_visit(node)

    def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef) -> None:
        self.node_sequence.append("AsyncFunctionDef")
        self.node_sequence.append(self._normalize_name(node.name))
        self.generic_visit(node)

    def visit_ClassDef(self, node: ast.ClassDef) -> None:
        self.node_sequence.append("ClassDef")
        self.node_sequence.append(self._normalize_name(node.name))
        self.generic_visit(node)

    def visit_Constant(self, node: ast.Constant) -> None:
        if isinstance(node.value, str):
            self.node_sequence.append("Constant(str)")
        else:
            self.node_sequence.append("Constant")

    def _normalize_name(self, name: str) -> str:
        if name in BUILTINS or name.startswith("__"):
            return name
        if name not in self.var_map:
            self.var_map[name] = f"var_{self.var_counter}"
            self.var_counter += 1
        return self.var_map[name]


def _lcs_length(left: list[str], right: list[str]) -> int:
    prev = [0] * (len(right) + 1)
    curr = [0] * (len(right) + 1)
    for i in range(1, len(left) + 1):
        for j in range(1, len(right) + 1):
            if left[i - 1] == right[j - 1]:
                curr[j] = prev[j - 1] + 1
            else:
                curr[j] = max(prev[j], curr[j - 1])
        prev, curr = curr, prev
    return prev[len(right)]
