"""Conversation JSONL schema and dataset quality helpers."""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from hashlib import sha256
from typing import Any, cast


class SchemaError(ValueError):
    """Raised when a JSONL conversation dataset is invalid."""


@dataclass(frozen=True)
class FunctionCall:
    name: str
    arguments: str


@dataclass(frozen=True)
class ToolCall:
    name: str
    arguments: dict[str, Any]
    id: str | None = None


@dataclass(frozen=True)
class Message:
    role: str
    content: str = ""
    name: str | None = None
    reasoning: str | None = None
    function_call: FunctionCall | None = None
    tool_calls: list[ToolCall] = field(default_factory=list)


@dataclass(frozen=True)
class ConversationEntry:
    id: str | None
    task: dict[str, Any]
    messages: list[Message]
    tools: list[dict[str, Any]] = field(default_factory=list)
    final: dict[str, Any] = field(default_factory=dict)
    function_calls: list[FunctionCall] | None = None
    thinking: str | None = None
    metadata: dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class DatasetMetadata:
    conversation_count: int
    total_messages: int
    size_bytes: int
    digest: str


@dataclass(frozen=True)
class ParsedDataset:
    conversations: list[ConversationEntry]
    metadata: DatasetMetadata


def parse_jsonl(content: str | bytes) -> ParsedDataset:
    """Parse a JSONL dataset into validated conversation entries."""

    raw = content.decode("utf-8") if isinstance(content, bytes) else content
    if not raw.strip():
        raise SchemaError("dataset is empty")
    entries: list[ConversationEntry] = []
    for index, line in enumerate(raw.splitlines(), start=1):
        if not line.strip():
            continue
        entries.append(_parse_line(line, index))
    if not entries:
        raise SchemaError("dataset contains no conversations")
    total_messages = sum(len(entry.messages) for entry in entries)
    encoded = raw.encode("utf-8")
    return ParsedDataset(
        conversations=entries,
        metadata=DatasetMetadata(
            conversation_count=len(entries),
            total_messages=total_messages,
            size_bytes=len(encoded),
            digest=sha256(encoded).hexdigest(),
        ),
    )


def validate_conversation(entry: ConversationEntry, line_num: int = 1) -> None:
    if not entry.messages:
        raise SchemaError(f"line {line_num}: messages array is empty")
    if len(entry.messages) < 2:
        raise SchemaError(f"line {line_num}: need at least 2 messages")
    if not validate_role_sequence(entry.messages):
        raise SchemaError(f"line {line_num}: invalid role sequence")
    for message in entry.messages:
        if not message.content and message.function_call is None and not message.tool_calls:
            raise SchemaError(f"line {line_num}: message content is empty")


def validate_role_sequence(messages: list[Message]) -> bool:
    if not messages:
        return False
    first = messages[0].role
    if first not in {"system", "user", "assistant"}:
        return False
    valid_pairs = {
        ("system", "user"),
        ("system", "assistant"),
        ("user", "assistant"),
        ("user", "function"),
        ("user", "tool"),
        ("assistant", "user"),
        ("assistant", "function"),
        ("assistant", "tool"),
        ("function", "user"),
        ("function", "assistant"),
        ("tool", "assistant"),
        ("tool", "user"),
    }
    return all(
        (prev.role, curr.role) in valid_pairs
        for prev, curr in zip(messages, messages[1:], strict=False)
    )


def conversation_hash(entry: ConversationEntry) -> str:
    payload = json.dumps(_conversation_to_dict(entry), sort_keys=True, separators=(",", ":"))
    return sha256(payload.encode("utf-8")).hexdigest()


def _parse_line(line: str, line_num: int) -> ConversationEntry:
    try:
        payload = json.loads(line)
    except json.JSONDecodeError as exc:
        raise SchemaError(f"line {line_num}: invalid JSON: {exc}") from exc
    if not isinstance(payload, dict):
        raise SchemaError(f"line {line_num}: expected JSON object")
    if "messages" not in payload:
        raise SchemaError(f"line {line_num}: missing messages field")
    messages = payload["messages"]
    if not isinstance(messages, list):
        raise SchemaError(f"line {line_num}: messages must be a list")
    task_raw = payload.get("task")
    tools_raw = payload.get("tools")
    final_raw = payload.get("final")
    metadata = payload.get("metadata")
    entry = ConversationEntry(
        id=payload.get("id") if isinstance(payload.get("id"), str) else None,
        task=cast(dict[str, Any], task_raw) if isinstance(task_raw, dict) else {},
        messages=[_parse_message(message, line_num) for message in messages],
        tools=cast(list[dict[str, Any]], tools_raw) if isinstance(tools_raw, list) else [],
        final=cast(dict[str, Any], final_raw) if isinstance(final_raw, dict) else {},
        function_calls=_parse_function_calls(payload.get("function_calls"), line_num),
        thinking=payload.get("thinking") if isinstance(payload.get("thinking"), str) else None,
        metadata=metadata if isinstance(metadata, dict) else {},
    )
    validate_conversation(entry, line_num)
    return entry


def _parse_message(payload: object, line_num: int) -> Message:
    if not isinstance(payload, dict):
        raise SchemaError(f"line {line_num}: message must be an object")
    role = payload.get("role")
    if not isinstance(role, str) or not role:
        raise SchemaError(f"line {line_num}: message role is required")
    content = payload.get("content", "")
    if content is None:
        content = ""
    if not isinstance(content, str):
        raise SchemaError(f"line {line_num}: message content must be a string")
    return Message(
        role=role,
        content=content,
        name=payload.get("name") if isinstance(payload.get("name"), str) else None,
        reasoning=payload.get("reasoning") if isinstance(payload.get("reasoning"), str) else None,
        function_call=_parse_function_call(payload.get("function_call"), line_num),
        tool_calls=_parse_tool_calls(payload.get("tool_calls"), line_num),
    )


def _parse_function_call(payload: object, line_num: int) -> FunctionCall | None:
    if payload is None:
        return None
    if not isinstance(payload, dict):
        raise SchemaError(f"line {line_num}: function_call must be an object")
    name = payload.get("name")
    arguments = payload.get("arguments", "")
    if not isinstance(name, str) or not isinstance(arguments, str):
        raise SchemaError(f"line {line_num}: invalid function_call fields")
    return FunctionCall(name=name, arguments=arguments)


def _parse_function_calls(payload: object, line_num: int) -> list[FunctionCall] | None:
    if payload is None:
        return None
    if not isinstance(payload, list):
        raise SchemaError(f"line {line_num}: function_calls must be a list")
    return [
        function_call
        for item in payload
        if (function_call := _parse_function_call(item, line_num)) is not None
    ]


def _parse_tool_calls(payload: object, line_num: int) -> list[ToolCall]:
    if payload is None:
        return []
    if not isinstance(payload, list):
        raise SchemaError(f"line {line_num}: tool_calls must be a list")
    tool_calls: list[ToolCall] = []
    for item in payload:
        if not isinstance(item, dict):
            raise SchemaError(f"line {line_num}: tool call must be an object")
        name = item.get("name")
        arguments = item.get("arguments", {})
        if not isinstance(name, str):
            raise SchemaError(f"line {line_num}: tool call name is required")
        if not isinstance(arguments, dict):
            raise SchemaError(f"line {line_num}: tool call arguments must be an object")
        tool_calls.append(
            ToolCall(
                name=name,
                arguments=arguments,
                id=item.get("id") if isinstance(item.get("id"), str) else None,
            )
        )
    return tool_calls


def _conversation_to_dict(entry: ConversationEntry) -> dict[str, object]:
    return {
        "id": entry.id,
        "task": entry.task,
        "messages": [
            {
                "role": message.role,
                "content": message.content,
                "name": message.name,
                "reasoning": message.reasoning,
                "function_call": None
                if message.function_call is None
                else {
                    "name": message.function_call.name,
                    "arguments": message.function_call.arguments,
                },
                "tool_calls": [
                    {
                        "id": tool_call.id,
                        "name": tool_call.name,
                        "arguments": tool_call.arguments,
                    }
                    for tool_call in message.tool_calls
                ],
            }
            for message in entry.messages
        ],
        "tools": entry.tools,
        "final": entry.final,
        "function_calls": None
        if entry.function_calls is None
        else [
            {"name": function_call.name, "arguments": function_call.arguments}
            for function_call in entry.function_calls
        ],
        "thinking": entry.thinking,
        "metadata": entry.metadata,
    }
