import json

from data_fabrication.evaluator.dataset import SchemaError, parse_jsonl
from data_fabrication.evaluator.scoring import evaluate_dataset

VALID_JSONL = "\n".join(
    [
        json.dumps(
            {
                "messages": [
                    {
                        "role": "user",
                        "content": "Describe a robust synthetic data task in detail.",
                    },
                    {
                        "role": "assistant",
                        "content": (
                            "A robust task includes a clear prompt, a grounded answer, "
                            "constraints, and enough diverse content to support training."
                        ),
                    },
                ]
            }
        ),
        json.dumps(
            {
                "messages": [
                    {"role": "system", "content": "You are helpful."},
                    {
                        "role": "user",
                        "content": "Create a reasoning example for arithmetic.",
                    },
                    {
                        "role": "assistant",
                        "content": (
                            "First decompose the problem, then calculate each term, "
                            "and finally verify the result with an inverse check."
                        ),
                    },
                ]
            }
        ),
    ]
)


def test_parse_jsonl_and_score_dataset() -> None:
    dataset = parse_jsonl(VALID_JSONL)
    assert dataset.metadata.conversation_count == 2
    assert dataset.metadata.total_messages == 5

    result = evaluate_dataset(dataset.conversations, size_bytes=dataset.metadata.size_bytes)
    assert result.score > 0.5
    assert result.passed


def test_invalid_jsonl_missing_messages() -> None:
    try:
        parse_jsonl('{"text":"missing messages"}')
    except SchemaError as exc:
        assert "missing messages" in str(exc)
    else:
        raise AssertionError("expected SchemaError")
