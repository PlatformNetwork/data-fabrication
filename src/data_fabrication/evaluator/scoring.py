"""Dataset scoring logic ported from the original WASM challenge."""

from __future__ import annotations

from dataclasses import dataclass

from .dataset import ConversationEntry, conversation_hash, validate_role_sequence

FORMAT_WEIGHT = 0.10
QUALITY_WEIGHT = 0.25
ORIGINALITY_WEIGHT = 0.15
AGENTIC_WEIGHT = 0.15
REASONING_WEIGHT = 0.10
FUNCTION_CALL_WEIGHT = 0.10
CODING_RELEVANCE_WEIGHT = 0.08
VERIFIABILITY_WEIGHT = 0.05
DIVERSITY_WEIGHT = 0.02


@dataclass(frozen=True)
class DatasetQualityMetrics:
    format_score: float
    quality_score: float
    originality_score: float
    agentic_score: float = 0.0
    reasoning_score: float = 0.0
    function_call_score: float = 0.0
    coding_relevance_score: float = 0.0
    verifiability_score: float = 0.0
    diversity_score: float = 0.0


@dataclass(frozen=True)
class EvaluationResult:
    passed: bool
    score: float
    conversation_count: int
    total_messages: int
    size_bytes: int
    metrics: DatasetQualityMetrics
    error: str | None = None


def calculate_score(metrics: DatasetQualityMetrics) -> float:
    """Calculate the final score from quality metrics."""

    fmt = _clamp(metrics.format_score)
    quality = _clamp(metrics.quality_score)
    originality = _clamp(metrics.originality_score)
    return _clamp(
        fmt * FORMAT_WEIGHT
        + quality * QUALITY_WEIGHT
        + originality * ORIGINALITY_WEIGHT
        + _clamp(metrics.agentic_score) * AGENTIC_WEIGHT
        + _clamp(metrics.reasoning_score) * REASONING_WEIGHT
        + _clamp(metrics.function_call_score) * FUNCTION_CALL_WEIGHT
        + _clamp(metrics.coding_relevance_score) * CODING_RELEVANCE_WEIGHT
        + _clamp(metrics.verifiability_score) * VERIFIABILITY_WEIGHT
        + _clamp(metrics.diversity_score) * DIVERSITY_WEIGHT
    )


def aggregate_scores(scores: list[float]) -> float:
    if not scores:
        return 0.0
    return _clamp(sum(scores) / len(scores))


def to_weight(score: float, max_score: float) -> float:
    if max_score <= 0:
        return 0.0
    return _clamp(score / max_score)


def evaluate_dataset(
    conversations: list[ConversationEntry],
    *,
    size_bytes: int,
    min_conversations: int = 1,
    max_conversations: int = 10_000,
) -> EvaluationResult:
    """Score a parsed conversation dataset."""

    if not conversations:
        metrics = DatasetQualityMetrics(0.0, 0.0, 0.0)
        return EvaluationResult(False, 0.0, 0, 0, size_bytes, metrics, "no conversations")
    if len(conversations) < min_conversations:
        metrics = DatasetQualityMetrics(0.0, 0.0, 0.0)
        return EvaluationResult(False, 0.0, len(conversations), 0, size_bytes, metrics, "too small")
    if len(conversations) > max_conversations:
        metrics = DatasetQualityMetrics(0.0, 0.0, 0.0)
        return EvaluationResult(False, 0.0, len(conversations), 0, size_bytes, metrics, "too large")

    metrics = calculate_quality(conversations)
    plagiarism_score = heuristic_plagiarism_score(conversations)
    adjusted = DatasetQualityMetrics(
        format_score=metrics.format_score,
        quality_score=metrics.quality_score,
        originality_score=_clamp(metrics.originality_score * plagiarism_score),
        agentic_score=metrics.agentic_score,
        reasoning_score=metrics.reasoning_score,
        function_call_score=metrics.function_call_score,
        coding_relevance_score=metrics.coding_relevance_score,
        verifiability_score=metrics.verifiability_score,
        diversity_score=metrics.diversity_score,
    )
    final_score = calculate_score(adjusted)
    total_messages = sum(len(entry.messages) for entry in conversations)
    return EvaluationResult(
        passed=final_score >= 0.5,
        score=final_score,
        conversation_count=len(conversations),
        total_messages=total_messages,
        size_bytes=size_bytes,
        metrics=adjusted,
    )


def calculate_quality(conversations: list[ConversationEntry]) -> DatasetQualityMetrics:
    if not conversations:
        return DatasetQualityMetrics(0.0, 0.0, 0.0)
    return DatasetQualityMetrics(
        format_score=_format_score(conversations),
        quality_score=_content_quality(conversations),
        originality_score=_originality_score(conversations),
        agentic_score=_agentic_score(conversations),
        reasoning_score=_reasoning_score(conversations),
        function_call_score=_function_call_score(conversations),
        coding_relevance_score=_coding_relevance_score(conversations),
        verifiability_score=_verifiability_score(conversations),
        diversity_score=_dataset_diversity_score(conversations),
    )


def heuristic_plagiarism_score(conversations: list[ConversationEntry]) -> float:
    if not conversations:
        return 0.0
    penalties = 0.0
    for entry in conversations:
        assistant = [m.content for m in entry.messages if m.role == "assistant"]
        short_count = sum(1 for content in assistant if len(content) < 20)
        generic_count = sum(1 for content in assistant if _is_generic(content))
        if assistant and short_count > len(assistant) / 2:
            penalties += 0.1
        if assistant and generic_count > len(assistant) / 3:
            penalties += 0.1
    return _clamp(1.0 - penalties / len(conversations))


def _format_score(conversations: list[ConversationEntry]) -> float:
    valid = 0
    for entry in conversations:
        if len(entry.messages) < 2:
            continue
        if not validate_role_sequence(entry.messages):
            continue
        if not all(message.content or message.function_call for message in entry.messages):
            continue
        valid += 1
    return _clamp(valid / len(conversations))


def _content_quality(conversations: list[ConversationEntry]) -> float:
    return _clamp(sum(_conversation_quality(entry) for entry in conversations) / len(conversations))


def _conversation_quality(entry: ConversationEntry) -> float:
    message_count = len(entry.messages)
    turn_score = 0.3 if 4 <= message_count <= 20 else 0.2 if message_count >= 2 else 0.0
    total_len = sum(len(message.content) for message in entry.messages)
    avg_len = total_len / max(message_count, 1)
    length_score = 0.3 if 50 <= avg_len <= 500 else 0.15 if avg_len >= 10 else 0.0
    diversity_score = _response_diversity(entry) * 0.4
    return _clamp(turn_score + length_score + diversity_score)


def _response_diversity(entry: ConversationEntry) -> float:
    contents = [message.content for message in entry.messages if message.content]
    if len(contents) < 2:
        return 0.0
    unique: list[str] = []
    for content in contents:
        if not any(
            content == previous or _near_duplicate(content, previous) for previous in unique
        ):
            unique.append(content)
    uniqueness = len(unique) / len(contents)
    if uniqueness < 0.5:
        return uniqueness * 0.5
    return uniqueness


def _originality_score(conversations: list[ConversationEntry]) -> float:
    hashes = {conversation_hash(entry) for entry in conversations}
    uniqueness = len(hashes) / len(conversations)
    penalty = _internal_similarity_penalty(conversations)
    return _clamp(uniqueness * (1.0 - penalty))


def _agentic_score(conversations: list[ConversationEntry]) -> float:
    if not conversations:
        return 0.0
    scored = 0.0
    for entry in conversations:
        has_tools = bool(entry.tools)
        has_tool_calls = any(
            message.tool_calls or message.function_call for message in entry.messages
        )
        has_tool_results = any(message.role in {"tool", "function"} for message in entry.messages)
        scored += (float(has_tools) + float(has_tool_calls) + float(has_tool_results)) / 3.0
    return _clamp(scored / len(conversations))


def _reasoning_score(conversations: list[ConversationEntry]) -> float:
    assistant_messages = [
        message
        for entry in conversations
        for message in entry.messages
        if message.role == "assistant"
    ]
    if not assistant_messages:
        return 0.0
    useful = [
        message
        for message in assistant_messages
        if message.reasoning and len(message.reasoning.split()) >= 8
    ]
    return _clamp(len(useful) / len(assistant_messages))


def _function_call_score(conversations: list[ConversationEntry]) -> float:
    if not conversations:
        return 0.0
    allowed = {"read", "grep", "bash", "list_dir", "edit", "test", "python"}
    scores: list[float] = []
    for entry in conversations:
        declared = {str(tool.get("name")) for tool in entry.tools if isinstance(tool, dict)}
        calls = [call for message in entry.messages for call in message.tool_calls]
        function_calls = [
            message.function_call for message in entry.messages if message.function_call is not None
        ]
        if not calls and not function_calls:
            scores.append(0.0)
            continue
        valid_calls = sum(
            1
            for call in calls
            if call.name in allowed or call.name in declared or bool(call.arguments)
        )
        valid_functions = sum(1 for call in function_calls if bool(call.name))
        total = len(calls) + len(function_calls)
        scores.append((valid_calls + valid_functions) / total)
    return _clamp(sum(scores) / len(scores))


def _coding_relevance_score(conversations: list[ConversationEntry]) -> float:
    keywords = {
        "code",
        "bug",
        "test",
        "patch",
        "repo",
        "function",
        "class",
        "debug",
        "refactor",
        "swe",
        "benchmark",
    }
    if not conversations:
        return 0.0
    scores = []
    for entry in conversations:
        text = " ".join(
            [
                str(entry.task.get("type", "")),
                str(entry.task.get("prompt", "")),
                " ".join(message.content for message in entry.messages),
                str(entry.metadata.get("benchmark_tags", "")),
            ]
        ).lower()
        hits = sum(1 for keyword in keywords if keyword in text)
        scores.append(min(1.0, hits / 4.0))
    return _clamp(sum(scores) / len(scores))


def _verifiability_score(conversations: list[ConversationEntry]) -> float:
    if not conversations:
        return 0.0
    scores = []
    for entry in conversations:
        final = entry.final
        has_patch = bool(final.get("patch") or final.get("answer"))
        tests = final.get("tests")
        has_tests = isinstance(tests, list) and bool(tests)
        has_grounding = any(message.role in {"tool", "function"} for message in entry.messages)
        scores.append((float(has_patch) + float(has_tests) + float(has_grounding)) / 3.0)
    return _clamp(sum(scores) / len(scores))


def _dataset_diversity_score(conversations: list[ConversationEntry]) -> float:
    if not conversations:
        return 0.0
    task_types = {str(entry.task.get("type", "")) for entry in conversations if entry.task}
    difficulties = {str(entry.task.get("difficulty", "")) for entry in conversations if entry.task}
    tool_names = {
        call.name
        for entry in conversations
        for message in entry.messages
        for call in message.tool_calls
    }
    return _clamp(
        min(1.0, len(task_types) / 4.0) * 0.4
        + min(1.0, len(difficulties) / 3.0) * 0.2
        + min(1.0, len(tool_names) / 3.0) * 0.4
    )


def _internal_similarity_penalty(conversations: list[ConversationEntry]) -> float:
    if len(conversations) < 2:
        return 0.0
    similarities: list[float] = []
    for i, left in enumerate(conversations):
        for right in conversations[i + 1 :]:
            similarities.append(_conversation_similarity(left, right))
    if not similarities:
        return 0.0
    avg = sum(similarities) / len(similarities)
    if avg > 0.8:
        return avg * 0.5
    if avg > 0.5:
        return avg * 0.2
    return 0.0


def _conversation_similarity(left: ConversationEntry, right: ConversationEntry) -> float:
    if not left.messages or not right.messages:
        return 0.0
    length_ratio = min(len(left.messages), len(right.messages)) / max(
        len(left.messages), len(right.messages)
    )
    if length_ratio < 0.5:
        return 0.0
    left_words = set(left.messages[0].content.lower().split())
    right_words = set(right.messages[0].content.lower().split())
    if not left_words or not right_words:
        return 0.0
    return len(left_words & right_words) / len(left_words | right_words)


def _near_duplicate(left: str, right: str) -> bool:
    if not left or not right:
        return False
    length_ratio = min(len(left), len(right)) / max(len(left), len(right))
    if length_ratio < 0.9:
        return False
    min_len = min(len(left), len(right))
    prefix_match = sum(1 for a, b in zip(left[:min_len], right[:min_len], strict=True) if a == b)
    return prefix_match / min_len > 0.9


def _is_generic(content: str) -> bool:
    lower = content.lower()
    phrases = (
        "i understand",
        "that makes sense",
        "thank you for sharing",
        "as an ai language model",
        "i hope this helps",
    )
    return any(phrase in lower for phrase in phrases)


def _clamp(value: float) -> float:
    if value != value:
        return 0.0
    return max(0.0, min(1.0, value))
