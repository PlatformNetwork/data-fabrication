"""Platform-compatible challenge settings."""

from __future__ import annotations

from pathlib import Path
from urllib.parse import urlparse

from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict


class ChallengeSettings(BaseSettings):
    """Base runtime settings shared by Platform challenge services."""

    model_config = SettingsConfigDict(env_prefix="CHALLENGE_", extra="ignore")

    slug: str = "data-fabrication"
    name: str = "Data Fabrication"
    version: str = "0.1.0"
    api_version: str = "1.0"
    sdk_version: str = "1.0.0"
    database_url: str = "sqlite+aiosqlite:////data/data-fabrication.sqlite3"
    data_dir: str = "/data"
    shared_token: str | None = Field(default=None, repr=False)
    shared_token_file: str | None = Field(
        default="/run/secrets/platform/challenge_token",
        repr=False,
    )
    host: str = "0.0.0.0"
    port: int = 8080

    docker_enabled: bool = False
    docker_bin: str = "docker"
    docker_network: str = "none"
    docker_cpus: float = 2.0
    docker_memory: str = "4g"
    docker_memory_swap: str | None = "4g"
    docker_pids_limit: int = 512
    docker_read_only: bool = True
    docker_user: str | None = None
    docker_allowed_images: tuple[str, ...] = ("python:3.12-slim",)
    docker_backend: str = "cli"
    docker_broker_url: str | None = None
    docker_broker_token: str | None = None
    docker_broker_token_file: str | None = None

    @property
    def resolved_database_path(self) -> Path:
        """Resolve the SQLite database path from a sqlite+aiosqlite URL."""

        if self.database_url.startswith("sqlite+aiosqlite:///"):
            parsed = urlparse(self.database_url.replace("sqlite+aiosqlite", "sqlite", 1))
            if parsed.path:
                return Path(parsed.path)
        if self.database_url.startswith("sqlite:///"):
            parsed = urlparse(self.database_url)
            if parsed.path:
                return Path(parsed.path)
        return Path(self.database_url)
