import json

from data_fabrication.evaluator.dataset import SchemaError, parse_jsonl
from data_fabrication.evaluator.scoring import evaluate_dataset


def _entry(index: int) -> dict[str, object]:
    return {
        "id": f"sample-{index}",
        "task": {
            "type": "bug_fix" if index == 1 else "test_generation",
            "difficulty": "medium" if index == 1 else "hard",
            "prompt": "Inspect repository code, call tools, and produce a verified patch.",
        },
        "tools": [{"name": "read"}, {"name": "grep"}, {"name": "bash"}],
        "messages": [
            {"role": "system", "content": "You are a software engineering agent."},
            {
                "role": "user",
                "content": "Find the parser bug, inspect relevant code, and include tests.",
            },
            {
                "role": "assistant",
                "content": (
                    "I will inspect files and tests before editing the parser implementation."
                ),
                "reasoning": (
                    "A grounded repository change needs evidence from code and "
                    "tests before writing a patch."
                ),
                "tool_calls": [{"name": "grep", "arguments": {"pattern": "parse"}}],
            },
            {"role": "tool", "name": "grep", "content": "src/parser.py:def parse(value):"},
            {
                "role": "assistant",
                "content": (
                    "The patch handles empty strings and adds a regression test for the parser."
                ),
                "reasoning": (
                    "The tool output identified the parser entrypoint and the final "
                    "answer includes validation."
                ),
            },
        ],
        "final": {
            "patch": "diff --git a/src/parser.py b/src/parser.py",
            "tests": ["pytest tests/test_parser.py"],
        },
        "metadata": {"benchmark_tags": ["swe", "debug", "test"]},
    }


VALID_JSONL = "\n".join(json.dumps(_entry(index)) for index in (1, 2))


def test_parse_jsonl_and_score_dataset() -> None:
    dataset = parse_jsonl(VALID_JSONL)
    assert dataset.metadata.conversation_count == 2
    assert dataset.metadata.total_messages == 10

    result = evaluate_dataset(dataset.conversations, size_bytes=dataset.metadata.size_bytes)
    assert result.score > 0.7
    assert result.passed
    assert result.metrics.agentic_score == 1.0
    assert result.metrics.function_call_score == 1.0


def test_invalid_jsonl_missing_messages() -> None:
    try:
        parse_jsonl('{"text":"missing messages"}')
    except SchemaError as exc:
        assert "missing messages" in str(exc)
    else:
        raise AssertionError("expected SchemaError")
