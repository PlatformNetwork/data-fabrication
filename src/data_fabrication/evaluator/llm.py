"""Optional LLM plagiarism audit client."""

from __future__ import annotations

import asyncio
import json
from dataclasses import dataclass
from typing import Any

import httpx


@dataclass(frozen=True)
class PlagiarismVerdict:
    is_plagiarism: bool
    confidence: float
    reasoning: str
    audit: dict[str, Any]


@dataclass(frozen=True)
class LlmInferenceConfig:
    endpoint: str
    model: str
    timeout_seconds: float = 60.0
    max_retries: int = 3
    retry_delay_seconds: float = 1.0


class LlmInference:
    """Retry-enabled JSON LLM client for semantic plagiarism checks."""

    def __init__(self, config: LlmInferenceConfig) -> None:
        self.config = config

    async def evaluate_plagiarism(
        self, code_a: str, code_b: str, similarity: float
    ) -> PlagiarismVerdict:
        last_error: Exception | None = None
        delay = self.config.retry_delay_seconds
        for attempt in range(max(self.config.max_retries, 1)):
            try:
                verdict = await self._evaluate_once(code_a, code_b, similarity)
                _validate_verdict(verdict)
                return verdict
            except Exception as exc:
                last_error = exc
                if attempt + 1 < max(self.config.max_retries, 1):
                    await asyncio.sleep(delay)
                    delay *= 2
        raise RuntimeError(f"LLM plagiarism audit failed: {last_error}")

    async def _evaluate_once(
        self, code_a: str, code_b: str, similarity: float
    ) -> PlagiarismVerdict:
        payload = {
            "model": self.config.model,
            "stream": False,
            "messages": [{"role": "user", "content": _prompt(code_a, code_b, similarity)}],
        }
        async with httpx.AsyncClient(timeout=self.config.timeout_seconds) as client:
            response = await client.post(self.config.endpoint, json=payload)
            response.raise_for_status()
        return _parse_response(response.text)


def _prompt(code_a: str, code_b: str, similarity: float) -> str:
    return f"""You are a plagiarism detection expert. Analyze these two Python code snippets.

Code A:
```python
{code_a}
```

Code B:
```python
{code_b}
```

Structural similarity: {similarity:.1f}%

Return one JSON object:
{{
  "is_plagiarism": boolean,
  "confidence": number,
  "reasoning": "one sentence",
  "audit": {{
    "structural_match": number,
    "logic_flow_similarity": boolean,
    "variable_patterns": "short text",
    "comments_analysis": "short text",
    "code_origin": "short text",
    "recommendation": "CONFIRM_PLAGIARISM|CLEAR|FLAG_FOR_REVIEW"
  }}
}}
"""


def _parse_response(body: str) -> PlagiarismVerdict:
    start = body.find("{")
    end = body.rfind("}")
    if start < 0 or end < start:
        raise ValueError("LLM response did not contain a JSON object")
    data = json.loads(body[start : end + 1])
    return PlagiarismVerdict(
        is_plagiarism=bool(data.get("is_plagiarism")),
        confidence=float(data.get("confidence", 0.0)),
        reasoning=str(data.get("reasoning", "")),
        audit=dict(data.get("audit") or {}),
    )


def _validate_verdict(verdict: PlagiarismVerdict) -> None:
    if not 0.0 <= verdict.confidence <= 1.0:
        raise ValueError("LLM confidence out of range")
    if not verdict.reasoning:
        raise ValueError("LLM reasoning is empty")
