from __future__ import annotations

import base64
import io
import json
import zipfile
from pathlib import Path

from fastapi.testclient import TestClient

from data_fabrication.app import create_app
from data_fabrication.config import DataFabricationSettings


def _settings(tmp_path: Path) -> DataFabricationSettings:
    return DataFabricationSettings(
        database_url=f"sqlite+aiosqlite:///{tmp_path / 'test.sqlite3'}",
        artifact_root=str(tmp_path / "artifacts"),
        shared_token="dev-secret",
        direct_subprocess_enabled=True,
        evaluation_timeout_seconds=30,
    )


def _zip_harness() -> bytes:
    dataset = {
        "id": "sample-1",
        "task": {
            "type": "bug_fix",
            "difficulty": "medium",
            "prompt": "Debug a Python function and provide a tested patch.",
        },
        "tools": [
            {"name": "read", "description": "Read files"},
            {"name": "grep", "description": "Search files"},
        ],
        "messages": [
            {"role": "system", "content": "You are a coding agent."},
            {
                "role": "user",
                "content": "The repo has a failing test for a parser. Find and fix it.",
            },
            {
                "role": "assistant",
                "content": "I will inspect the parser and tests before proposing a patch.",
                "reasoning": (
                    "The failing behavior must be grounded in repository evidence "
                    "before changing code."
                ),
                "tool_calls": [{"name": "grep", "arguments": {"pattern": "Parser"}}],
            },
            {"role": "tool", "name": "grep", "content": "src/parser.py: def parse(value):"},
            {
                "role": "assistant",
                "content": "Patch parse to handle empty strings and add regression coverage.",
                "reasoning": (
                    "The tool result shows the parser entrypoint, so the fix should "
                    "be minimal and tested."
                ),
            },
        ],
        "final": {
            "patch": "diff --git a/src/parser.py b/src/parser.py",
            "tests": ["pytest tests/test_parser.py"],
        },
        "metadata": {"benchmark_tags": ["swe", "debug", "test"]},
    }
    harness = f"""
import json
import os

output = os.environ.get("DATA_FABRICATION_OUTPUT")
line = json.dumps({dataset!r})
if output:
    with open(output, "w", encoding="utf-8") as handle:
        handle.write(line + "\\n")
else:
    print(line)
"""
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("harness.py", harness)
        archive.writestr("data_fabrication.yaml", "entrypoint: harness.py\n")
    return buffer.getvalue()


def test_submit_zip_harness_and_weights(tmp_path: Path) -> None:
    app = create_app(_settings(tmp_path))
    with TestClient(app) as client:
        response = client.post(
            "/submit",
            json={
                "hotkey": "5Abc",
                "filename": "submission.zip",
                "package_base64": base64.b64encode(_zip_harness()).decode("ascii"),
            },
        )
        assert response.status_code == 200
        submission_id = response.json()["id"]

        detail = client.get(f"/submissions/{submission_id}")
        assert detail.status_code == 200
        body = detail.json()
        assert body["status"] == "completed"
        assert body["score"] > 0.5
        assert body["static_review"]["accepted"] is True

        report = client.get(f"/v1/submissions/{submission_id}/report")
        assert report.status_code == 200
        assert report.json()["judge"]["overall_score"] > 0.0

        samples = client.get(f"/v1/submissions/{submission_id}/samples")
        assert samples.status_code == 200
        assert json.loads(samples.json()["jsonl"].splitlines()[0])["task"]["type"] == "bug_fix"

        weights = client.get(
            "/internal/v1/get_weights",
            headers={
                "authorization": "Bearer dev-secret",
                "x-platform-challenge-slug": "data-fabrication",
            },
        )
        assert weights.status_code == 200
        assert weights.json()["weights"]["5Abc"] > 0.0


def test_internal_bridge_accepts_raw_zip(tmp_path: Path) -> None:
    app = create_app(_settings(tmp_path))
    with TestClient(app) as client:
        response = client.post(
            "/internal/v1/bridge/submissions",
            content=_zip_harness(),
            headers={
                "authorization": "Bearer dev-secret",
                "x-platform-challenge-slug": "data-fabrication",
                "x-platform-verified-hotkey": "5Bridge",
                "x-submission-filename": "submission.zip",
                "content-type": "application/zip",
            },
        )
        assert response.status_code == 200
        detail = client.get(f"/submissions/{response.json()['id']}")
        assert detail.json()["status"] == "completed"


def test_internal_auth_rejects_bad_token(tmp_path: Path) -> None:
    app = create_app(_settings(tmp_path))
    with TestClient(app) as client:
        response = client.get(
            "/internal/v1/get_weights",
            headers={
                "authorization": "Bearer wrong",
                "x-platform-challenge-slug": "data-fabrication",
            },
        )
        assert response.status_code == 401
