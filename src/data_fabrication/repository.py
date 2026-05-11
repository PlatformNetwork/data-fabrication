"""Persistence helpers for submissions, scores, and leaderboard views."""

from __future__ import annotations

import base64
import hashlib
import json
from datetime import UTC, datetime
from typing import Any

import aiosqlite

from .db import Database
from .evaluator.execution import SubmissionEvaluation
from .models import (
    LeaderboardEntry,
    StatsResponse,
    SubmissionCreate,
    SubmissionDetail,
    SubmissionResponse,
)


class DataFabricationRepository:
    """Repository layer over SQLite."""

    def __init__(self, database: Database, *, max_submission_size_bytes: int) -> None:
        self.database = database
        self.max_submission_size_bytes = max_submission_size_bytes

    async def create_submission(
        self,
        payload: SubmissionCreate,
        *,
        verified_hotkey: str | None = None,
    ) -> tuple[str, str | None, str | None]:
        hotkey = verified_hotkey or payload.resolved_hotkey
        if not hotkey:
            raise ValueError("missing hotkey")
        dataset_jsonl = payload.dataset_jsonl
        code = payload.resolved_code
        if payload.package_base64:
            decoded = _decode_base64(payload.package_base64)
            if _looks_like_python(payload.filename, decoded):
                code = decoded.decode("utf-8", errors="replace")
            else:
                dataset_jsonl = decoded.decode("utf-8", errors="replace")
        material = (dataset_jsonl or code or "").encode("utf-8")
        if not material:
            raise ValueError("submission requires dataset_jsonl, package_base64, or code")
        if len(material) > self.max_submission_size_bytes:
            raise ValueError("submission too large")
        code_hash = hashlib.sha256(material).hexdigest()
        submission_id = hashlib.sha256(f"{hotkey}:{code_hash}".encode()).hexdigest()[:32]
        await self.database.execute(
            """
            INSERT OR REPLACE INTO submissions (
                id, hotkey, code_hash, filename, harness_code, dataset_jsonl,
                status, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                submission_id,
                hotkey,
                code_hash,
                payload.filename,
                code,
                dataset_jsonl,
                "pending",
                _now(),
            ),
        )
        return submission_id, dataset_jsonl, code

    async def mark_evaluated(
        self,
        submission_id: str,
        evaluation: SubmissionEvaluation,
    ) -> None:
        result = evaluation.result
        execution = evaluation.execution
        await self.database.execute(
            """
            UPDATE submissions
            SET status = ?, score = ?, passed = ?, conversation_count = ?,
                total_messages = ?, size_bytes = ?, metrics_json = ?,
                violations_json = ?, stdout = ?, stderr = ?, error = ?,
                updated_at = ?
            WHERE id = ?
            """,
            (
                "completed" if result.error is None else "failed",
                result.score,
                1 if result.passed else 0,
                result.conversation_count,
                result.total_messages,
                result.size_bytes,
                evaluation.metrics_json(),
                json.dumps(evaluation.violations, separators=(",", ":")),
                "" if execution is None else execution.stdout[-64_000:],
                "" if execution is None else execution.stderr[-64_000:],
                result.error,
                _now(),
                submission_id,
            ),
        )

    async def get_submission(self, submission_id: str) -> SubmissionDetail | None:
        row = await self.database.fetchone(
            "SELECT * FROM submissions WHERE id = ?",
            (submission_id,),
        )
        return None if row is None else _detail_from_row(row)

    async def get_agent(self, hotkey: str) -> SubmissionDetail | None:
        row = await self.database.fetchone(
            """
            SELECT * FROM submissions
            WHERE hotkey = ?
            ORDER BY score DESC, updated_at DESC
            LIMIT 1
            """,
            (hotkey,),
        )
        return None if row is None else _detail_from_row(row)

    async def list_submissions(self) -> list[SubmissionResponse]:
        rows = await self.database.fetchall(
            "SELECT * FROM submissions ORDER BY created_at DESC LIMIT 100"
        )
        return [_response_from_row(row) for row in rows]

    async def leaderboard(self) -> list[LeaderboardEntry]:
        rows = await self.database.fetchall(
            """
            SELECT *
            FROM submissions s
            WHERE status = 'completed'
              AND score = (
                SELECT MAX(score) FROM submissions i
                WHERE i.hotkey = s.hotkey AND i.status = 'completed'
              )
            ORDER BY score DESC, updated_at DESC
            LIMIT 100
            """
        )
        return [
            LeaderboardEntry(
                rank=index + 1,
                hotkey=str(row["hotkey"]),
                score=float(row["score"]),
                submission_id=str(row["id"]),
                conversation_count=int(row["conversation_count"]),
                total_messages=int(row["total_messages"]),
            )
            for index, row in enumerate(rows)
        ]

    async def weights(self) -> dict[str, float]:
        entries = await self.leaderboard()
        return {entry.hotkey: entry.score for entry in entries}

    async def stats(self) -> StatsResponse:
        row = await self.database.fetchone(
            """
            SELECT
                COUNT(*) AS total,
                SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) AS completed,
                COUNT(DISTINCT hotkey) AS active_miners,
                COALESCE(MAX(score), 0.0) AS best_score
            FROM submissions
            """
        )
        assert row is not None
        return StatsResponse(
            total_submissions=int(row["total"] or 0),
            completed_submissions=int(row["completed"] or 0),
            active_miners=int(row["active_miners"] or 0),
            best_score=float(row["best_score"] or 0.0),
        )


def _response_from_row(row: aiosqlite.Row) -> SubmissionResponse:
    return SubmissionResponse(
        id=str(row["id"]),
        hotkey=str(row["hotkey"]),
        code_hash=str(row["code_hash"]),
        status=str(row["status"]),
        score=float(row["score"]),
        passed=bool(row["passed"]),
        conversation_count=int(row["conversation_count"]),
        total_messages=int(row["total_messages"]),
        error=row["error"],
    )


def _detail_from_row(row: aiosqlite.Row) -> SubmissionDetail:
    response = _response_from_row(row)
    return SubmissionDetail(
        **response.model_dump(),
        filename=row["filename"],
        metrics=_json(row["metrics_json"], {}),
        violations=_json(row["violations_json"], []),
        stdout=str(row["stdout"] or ""),
        stderr=str(row["stderr"] or ""),
    )


def _decode_base64(value: str) -> bytes:
    try:
        return base64.b64decode(value, validate=True)
    except Exception as exc:
        raise ValueError("package_base64 is invalid") from exc


def _looks_like_python(filename: str | None, payload: bytes) -> bool:
    if filename and filename.endswith(".py"):
        return True
    text = payload[:256].decode("utf-8", errors="ignore")
    return "import " in text or "print(" in text or "def " in text


def _json(value: str, default: Any) -> Any:
    try:
        return json.loads(value)
    except json.JSONDecodeError:
        return default


def _now() -> str:
    return datetime.now(UTC).isoformat()
