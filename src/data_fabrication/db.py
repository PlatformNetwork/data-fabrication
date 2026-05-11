"""SQLite database wrapper."""

from __future__ import annotations

import asyncio
from collections.abc import AsyncIterator
from pathlib import Path

import aiosqlite


class Database:
    """Small async SQLite wrapper used by the challenge repository."""

    def __init__(self, path: Path) -> None:
        self.path = path
        self._connection: aiosqlite.Connection | None = None
        self._lock = asyncio.Lock()

    async def init(self) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self._connection = await aiosqlite.connect(self.path)
        self._connection.row_factory = aiosqlite.Row
        await self._connection.executescript(
            """
            CREATE TABLE IF NOT EXISTS submissions (
                id TEXT PRIMARY KEY,
                hotkey TEXT NOT NULL,
                code_hash TEXT NOT NULL,
                filename TEXT,
                harness_code TEXT,
                dataset_jsonl TEXT,
                status TEXT NOT NULL,
                score REAL NOT NULL DEFAULT 0.0,
                passed INTEGER NOT NULL DEFAULT 0,
                conversation_count INTEGER NOT NULL DEFAULT 0,
                total_messages INTEGER NOT NULL DEFAULT 0,
                size_bytes INTEGER NOT NULL DEFAULT 0,
                metrics_json TEXT NOT NULL DEFAULT '{}',
                violations_json TEXT NOT NULL DEFAULT '[]',
                stdout TEXT NOT NULL DEFAULT '',
                stderr TEXT NOT NULL DEFAULT '',
                error TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS idx_submissions_hotkey ON submissions(hotkey);
            CREATE INDEX IF NOT EXISTS idx_submissions_status_score
                ON submissions(status, score DESC);
            """
        )
        await self._connection.commit()

    async def close(self) -> None:
        if self._connection is not None:
            await self._connection.close()
            self._connection = None

    async def execute(self, sql: str, params: tuple[object, ...] = ()) -> None:
        async with self._lock:
            connection = self._require_connection()
            await connection.execute(sql, params)
            await connection.commit()

    async def fetchone(self, sql: str, params: tuple[object, ...] = ()) -> aiosqlite.Row | None:
        async with self._lock:
            cursor = await self._require_connection().execute(sql, params)
            return await cursor.fetchone()

    async def fetchall(self, sql: str, params: tuple[object, ...] = ()) -> list[aiosqlite.Row]:
        async with self._lock:
            cursor = await self._require_connection().execute(sql, params)
            return list(await cursor.fetchall())

    async def session_dependency(self) -> AsyncIterator[Database]:
        yield self

    def _require_connection(self) -> aiosqlite.Connection:
        if self._connection is None:
            raise RuntimeError("database is not initialized")
        return self._connection
