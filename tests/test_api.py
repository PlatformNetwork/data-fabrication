from pathlib import Path

from fastapi.testclient import TestClient

from data_fabrication.app import create_app
from data_fabrication.config import DataFabricationSettings


def _settings(tmp_path: Path) -> DataFabricationSettings:
    return DataFabricationSettings(
        database_url=f"sqlite+aiosqlite:///{tmp_path / 'test.sqlite3'}",
        shared_token="dev-secret",
        direct_subprocess_enabled=False,
    )


def test_submit_jsonl_and_weights(tmp_path: Path) -> None:
    app = create_app(_settings(tmp_path))
    jsonl = (
        '{"messages":[{"role":"user","content":"Explain how to create a high quality '
        'conversation dataset."},{"role":"assistant","content":"Use diverse prompts, detailed '
        'answers, clean schema, and repeated validation to keep the dataset useful."}]}'
    )
    with TestClient(app) as client:
        response = client.post("/submit", json={"hotkey": "5Abc", "dataset_jsonl": jsonl})
        assert response.status_code == 200
        submission_id = response.json()["id"]

        detail = client.get(f"/submissions/{submission_id}")
        assert detail.status_code == 200
        assert detail.json()["status"] == "completed"

        weights = client.get(
            "/internal/v1/get_weights",
            headers={
                "authorization": "Bearer dev-secret",
                "x-platform-challenge-slug": "data-fabrication",
            },
        )
        assert weights.status_code == 200
        assert weights.json()["weights"]["5Abc"] > 0.0


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
