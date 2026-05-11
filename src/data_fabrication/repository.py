"""Persistence helpers for submissions, scores, and leaderboard views."""

from __future__ import annotations

import base64
import hashlib
import json
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import aiosqlite

from .db import Database
from .evaluator.artifacts import (
    ArtifactError,
    ArtifactFile,
    SubmissionArtifact,
    validate_and_extract_zip,
)
from .evaluator.execution import SubmissionEvaluation
from .models import (
    LeaderboardEntry,
    StatsResponse,
    SubmissionCreate,
    SubmissionDetail,
    SubmissionReport,
    SubmissionResponse,
    SubmissionSamples,
)


class DataFabricationRepository:
    """Repository layer over SQLite."""

    def __init__(
        self,
        database: Database,
        *,
        max_submission_size_bytes: int,
        artifact_root: Path,
        max_zip_files: int,
        max_zip_uncompressed_bytes: int,
    ) -> None:
        self.database = database
        self.max_submission_size_bytes = max_submission_size_bytes
        self.artifact_root = artifact_root
        self.max_zip_files = max_zip_files
        self.max_zip_uncompressed_bytes = max_zip_uncompressed_bytes

    async def create_submission(
        self,
        payload: SubmissionCreate,
        *,
        verified_hotkey: str | None = None,
        raw_package: bytes | None = None,
        filename: str | None = None,
    ) -> str:
        hotkey = verified_hotkey or payload.resolved_hotkey
        if not hotkey:
            raise ValueError("missing hotkey")
        package = raw_package if raw_package is not None else _decode_base64(payload.package_base64)
        filename = filename or payload.filename or "submission.zip"
        if not filename.lower().endswith(".zip"):
            raise ValueError("submission filename must end with .zip")
        if len(package) > self.max_submission_size_bytes:
            raise ValueError("submission too large")

        artifact_hash = hashlib.sha256(package).hexdigest()
        submission_id = hashlib.sha256(f"{hotkey}:{artifact_hash}".encode()).hexdigest()[:32]
        try:
            artifact = validate_and_extract_zip(
                zip_bytes=package,
                submission_id=submission_id,
                artifact_root=self.artifact_root,
                max_size_bytes=self.max_submission_size_bytes,
                max_files=self.max_zip_files,
                max_uncompressed_bytes=self.max_zip_uncompressed_bytes,
            )
        except ArtifactError as exc:
            raise ValueError(str(exc)) from exc

        await self.database.execute(
            """
            INSERT OR REPLACE INTO submissions (
                id, hotkey, code_hash, artifact_hash, filename, artifact_json,
                status, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                submission_id,
                hotkey,
                artifact_hash,
                artifact.artifact_hash,
                filename,
                artifact.to_json(),
                "pending",
                _now(),
            ),
        )
        return submission_id

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
                violations_json = ?, static_review_json = ?, judge_json = ?,
                plagiarism_json = ?, sample_jsonl = ?, stdout = ?, stderr = ?, error = ?,
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
                "{}" if evaluation.static_report is None else evaluation.static_report.to_json(),
                "{}" if evaluation.judge is None else json.dumps(evaluation.judge.to_dict()),
                "{}" if evaluation.plagiarism is None else json.dumps(evaluation.plagiarism),
                evaluation.sample_jsonl,
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

    async def prior_artifacts(self, submission_id: str, *, limit: int = 50) -> list[dict[str, Any]]:
        rows = await self.database.fetchall(
            """
            SELECT id, hotkey, artifact_json
            FROM submissions
            WHERE id != ? AND artifact_json != '{}'
            ORDER BY updated_at DESC
            LIMIT ?
            """,
            (submission_id, limit),
        )
        artifacts: list[dict[str, Any]] = []
        for row in rows:
            artifact = _json(row["artifact_json"], {})
            if isinstance(artifact, dict):
                artifact["submission_id"] = str(row["id"])
                artifact["hotkey"] = str(row["hotkey"])
                artifacts.append(artifact)
        return artifacts

    async def artifact(self, submission_id: str) -> SubmissionArtifact | None:
        row = await self.database.fetchone(
            "SELECT artifact_json FROM submissions WHERE id = ?",
            (submission_id,),
        )
        if row is None:
            return None
        data = _json(row["artifact_json"], {})
        if not isinstance(data, dict) or not data:
            return None
        return SubmissionArtifact(
            submission_id=submission_id,
            source_zip_path=Path(str(data["source_zip_path"])),
            workspace_path=Path(str(data["workspace_path"])),
            entrypoint=str(data["entrypoint"]),
            artifact_hash=str(data["artifact_hash"]),
            files=[
                ArtifactFile(
                    path=str(file["path"]),
                    size=int(file["size"]),
                    sha256=str(file["sha256"]),
                )
                for file in data.get("files", [])
                if isinstance(file, dict)
            ],
            manifest=dict(data.get("manifest") or {}),
        )

    async def report(self, submission_id: str) -> SubmissionReport | None:
        row = await self.database.fetchone(
            "SELECT * FROM submissions WHERE id = ?",
            (submission_id,),
        )
        if row is None:
            return None
        return SubmissionReport(
            submission_id=submission_id,
            metrics=_json(row["metrics_json"], {}),
            static_review=_json(row["static_review_json"], None),
            judge=_json(row["judge_json"], None),
            plagiarism=_json(row["plagiarism_json"], None),
            violations=_json(row["violations_json"], []),
        )

    async def samples(self, submission_id: str) -> SubmissionSamples | None:
        row = await self.database.fetchone(
            "SELECT sample_jsonl FROM submissions WHERE id = ?",
            (submission_id,),
        )
        if row is None:
            return None
        return SubmissionSamples(submission_id=submission_id, jsonl=str(row["sample_jsonl"] or ""))


def _response_from_row(row: aiosqlite.Row) -> SubmissionResponse:
    return SubmissionResponse(
        id=str(row["id"]),
        hotkey=str(row["hotkey"]),
        code_hash=str(row["code_hash"]),
        artifact_hash=row["artifact_hash"],
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
        static_review=_json(row["static_review_json"], None),
        judge=_json(row["judge_json"], None),
        plagiarism=_json(row["plagiarism_json"], None),
    )


def _decode_base64(value: str | None) -> bytes:
    if not value:
        raise ValueError("submission requires package_base64 ZIP artifact")
    try:
        return base64.b64decode(value, validate=True)
    except Exception as exc:
        raise ValueError("package_base64 is invalid") from exc


def _json(value: object, default: Any) -> Any:
    if value in {None, ""}:
        return default
    try:
        return json.loads(str(value))
    except json.JSONDecodeError:
        return default


def _now() -> str:
    return datetime.now(UTC).isoformat()
