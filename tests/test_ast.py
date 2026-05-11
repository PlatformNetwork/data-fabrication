from data_fabrication.evaluator.ast_similarity import (
    compare_structures,
    normalize_ast,
    plagiarism_status,
)
from data_fabrication.evaluator.ast_validation import Severity, validate_python_code


def test_ast_similarity_normalizes_variable_names() -> None:
    left = normalize_ast("x = 1\ny = x + 2\nprint(y)\n")
    right = normalize_ast("alpha = 1\nbeta = alpha + 2\nprint(beta)\n")
    score = compare_structures(left, right)
    assert score >= 90
    assert plagiarism_status(score) == "plagiarized"


def test_ast_validation_blocks_critical_calls() -> None:
    violations = validate_python_code('import os\nos.system("ls")\n')
    assert any(v.severity == Severity.critical and v.pattern == "os.system" for v in violations)


def test_ast_validation_warns_subprocess() -> None:
    violations = validate_python_code('import subprocess\nsubprocess.run(["echo", "ok"])\n')
    assert any(v.severity == Severity.warning for v in violations)
