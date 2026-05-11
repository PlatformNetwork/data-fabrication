"""Platform challenge response schemas."""

from __future__ import annotations

from pydantic import BaseModel, Field


class HealthResponse(BaseModel):
    status: str = "ok"
    slug: str
    version: str


class VersionResponse(BaseModel):
    api_version: str
    challenge_version: str
    sdk_version: str
    capabilities: list[str] = Field(default_factory=list)


class WeightsResponse(BaseModel):
    challenge_slug: str
    epoch: int
    weights: dict[str, float]
