"""Runtime settings for Data Fabrication."""

from __future__ import annotations

import os
from typing import Any

from pydantic_settings import SettingsConfigDict

from .sdk.config import ChallengeSettings


class DataFabricationSettings(ChallengeSettings):
    """Data Fabrication service configuration."""

    model_config = SettingsConfigDict(env_prefix="DATA_FABRICATION_", extra="ignore")

    max_submission_size_bytes: int = 4 * 1024 * 1024
    max_output_size_bytes: int = 10 * 1024 * 1024
    evaluation_timeout_seconds: int = 300
    max_conversations: int = 10_000
    min_conversations: int = 1
    llm_plagiarism_enabled: bool = False
    llm_endpoint: str = "http://localhost:11434/api/chat"
    llm_model: str = "llama3"
    llm_timeout_seconds: float = 60.0
    llm_max_retries: int = 3
    public_submissions_enabled: bool = True
    direct_subprocess_enabled: bool = False

    def model_post_init(self, __context: Any) -> None:
        """Accept Platform orchestrator CHALLENGE_* env vars in addition to project vars."""

        fallbacks: dict[str, tuple[str, ...]] = {
            "slug": ("PLATFORM_CHALLENGE_SLUG", "CHALLENGE_SLUG"),
            "name": ("CHALLENGE_NAME",),
            "version": ("CHALLENGE_VERSION",),
            "database_url": ("CHALLENGE_DATABASE_URL",),
            "data_dir": ("CHALLENGE_DATA_DIR",),
            "shared_token": ("CHALLENGE_SHARED_TOKEN",),
            "shared_token_file": ("CHALLENGE_SHARED_TOKEN_FILE",),
            "host": ("CHALLENGE_HOST",),
            "port": ("CHALLENGE_PORT",),
            "docker_enabled": ("CHALLENGE_DOCKER_ENABLED",),
            "docker_bin": ("CHALLENGE_DOCKER_BIN",),
            "docker_network": ("CHALLENGE_DOCKER_NETWORK",),
            "docker_cpus": ("CHALLENGE_DOCKER_CPUS",),
            "docker_memory": ("CHALLENGE_DOCKER_MEMORY",),
            "docker_memory_swap": ("CHALLENGE_DOCKER_MEMORY_SWAP",),
            "docker_pids_limit": ("CHALLENGE_DOCKER_PIDS_LIMIT",),
            "docker_read_only": ("CHALLENGE_DOCKER_READ_ONLY",),
            "docker_user": ("CHALLENGE_DOCKER_USER",),
            "docker_allowed_images": ("CHALLENGE_DOCKER_ALLOWED_IMAGES",),
            "docker_backend": ("CHALLENGE_DOCKER_BACKEND",),
            "docker_broker_url": ("CHALLENGE_DOCKER_BROKER_URL",),
            "docker_broker_token": ("CHALLENGE_DOCKER_BROKER_TOKEN",),
            "docker_broker_token_file": ("CHALLENGE_DOCKER_BROKER_TOKEN_FILE",),
        }
        for field, env_names in fallbacks.items():
            for env_name in env_names:
                if env_name in os.environ:
                    setattr(self, field, _coerce_env(os.environ[env_name], getattr(self, field)))
                    break


def _coerce_env(value: str, current: object) -> object:
    if isinstance(current, bool):
        return value.lower() in {"1", "true", "yes", "on"}
    if isinstance(current, int) and not isinstance(current, bool):
        return int(value)
    if isinstance(current, float):
        return float(value)
    if isinstance(current, tuple):
        return tuple(item.strip() for item in value.split(",") if item.strip())
    return value


settings = DataFabricationSettings()
