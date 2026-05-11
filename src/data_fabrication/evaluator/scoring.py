"""Dataset scoring logic ported from the original WASM challenge."""

from __future__ import annotations

from dataclasses import dataclass

from .dataset import ConversationEntry, conversation_hash, validate_role_sequence

FORMAT_WEIGHT = 0.2
QUALITY_WEIGHT = 0.4
ORIGINALITY_WEIGHT = 0.4


@dataclass(frozen=True)
class DatasetQualityMetrics:
    format_score: float
    quality_score: float
    originality_score: float


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
    return fmt * FORMAT_WEIGHT + quality * QUALITY_WEIGHT + originality * ORIGINALITY_WEIGHT


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
