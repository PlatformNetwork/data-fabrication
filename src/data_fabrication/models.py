"""API schemas for Data Fabrication."""

from __future__ import annotations

from pydantic import BaseModel, Field


class SubmissionCreate(BaseModel):
    """Miner submission containing a base64-encoded ZIP harness."""

    hotkey: str | None = Field(default=None, min_length=1, max_length=128)
    miner_hotkey: str | None = Field(default=None, min_length=1, max_length=128)
    package_base64: str | None = Field(default=None, min_length=1, repr=False)
    filename: str | None = Field(default=None, max_length=256)
    signature: str | None = Field(default=None, max_length=512)

    @property
    def resolved_hotkey(self) -> str:
        return self.hotkey or self.miner_hotkey or ""


class SubmissionResponse(BaseModel):
    id: str
    hotkey: str
    code_hash: str
    status: str
    score: float
    passed: bool
    conversation_count: int
    total_messages: int
    artifact_hash: str | None = None
    error: str | None = None


class SubmissionDetail(SubmissionResponse):
    filename: str | None = None
    metrics: dict
    violations: list[dict]
    stdout: str = ""
    stderr: str = ""
    static_review: dict | None = None
    judge: dict | None = None
    plagiarism: dict | None = None


class SubmissionReport(BaseModel):
    submission_id: str
    metrics: dict
    static_review: dict | None = None
    judge: dict | None = None
    plagiarism: dict | None = None
    violations: list[dict]


class SubmissionSamples(BaseModel):
    submission_id: str
    jsonl: str


class LeaderboardEntry(BaseModel):
    rank: int
    hotkey: str
    score: float
    submission_id: str
    conversation_count: int
    total_messages: int


class StatsResponse(BaseModel):
    total_submissions: int
    completed_submissions: int
    active_miners: int
    best_score: float


class StatusResponse(BaseModel):
    slug: str
    status: str
    evaluation_enabled: bool
    upload_enabled: bool
    total_submissions: int


class DatasetResponse(BaseModel):
    dataset_schema: str = Field(default="agentic-coding-conversation-jsonl", alias="schema")
    min_conversations: int
    max_conversations: int
    required_fields: list[str] = ["task", "tools", "messages", "final"]
    roles: list[str] = ["system", "user", "assistant", "tool", "function"]


class DatasetConsensusResponse(BaseModel):
    status: str
    validator_count: int
    agreement_level: float


class WeightResponse(BaseModel):
    weights: dict[str, float]
