"""Python harness execution and dataset evaluation."""

from __future__ import annotations

import asyncio
import json
import tempfile
from dataclasses import asdict, dataclass
from pathlib import Path

from data_fabrication.config import DataFabricationSettings
from data_fabrication.sdk.executors.docker import (
    DockerExecutor,
    DockerLimits,
    DockerMount,
    DockerRunSpec,
)

from .ast_validation import Severity, validate_python_code
from .dataset import SchemaError, parse_jsonl
from .scoring import EvaluationResult, evaluate_dataset


@dataclass(frozen=True)
class HarnessExecutionResult:
    stdout: str
    stderr: str
    returncode: int
    duration_seconds: float
    timed_out: bool = False


@dataclass(frozen=True)
class SubmissionEvaluation:
    result: EvaluationResult
    execution: HarnessExecutionResult | None
    violations: list[dict[str, object]]

    def metrics_json(self) -> str:
        return json.dumps(
            {
                "result": asdict(self.result),
                "execution": None if self.execution is None else asdict(self.execution),
                "violations": self.violations,
            },
            separators=(",", ":"),
        )


class DataFabricationEvaluator:
    """Evaluate direct JSONL datasets or Python harness submissions."""

    def __init__(self, settings: DataFabricationSettings) -> None:
        self.settings = settings

    async def evaluate(
        self,
        *,
        submission_id: str,
        dataset_jsonl: str | None,
        harness_code: str | None,
    ) -> SubmissionEvaluation:
        if dataset_jsonl is None and harness_code is None:
            raise ValueError("submission requires dataset_jsonl or harness code")
        violations: list[dict[str, object]] = []
        execution: HarnessExecutionResult | None = None
        content = dataset_jsonl
        if harness_code is not None:
            validation = validate_python_code(harness_code)
            violations = [
                {
                    "pattern": violation.pattern,
                    "severity": violation.severity.value,
                    "line": violation.line,
                    "column": violation.column,
                }
                for violation in validation
            ]
            critical = [v for v in validation if v.severity == Severity.critical]
            if critical:
                result = EvaluationResult(
                    passed=False,
                    score=0.0,
                    conversation_count=0,
                    total_messages=0,
                    size_bytes=0,
                    metrics=evaluate_dataset([], size_bytes=0).metrics,
                    error=f"critical security violation: {critical[0].pattern}",
                )
                return SubmissionEvaluation(result, None, violations)
            execution = await self._execute_harness(submission_id, harness_code)
            if execution.returncode != 0 or execution.timed_out:
                result = EvaluationResult(
                    passed=False,
                    score=0.0,
                    conversation_count=0,
                    total_messages=0,
                    size_bytes=0,
                    metrics=evaluate_dataset([], size_bytes=0).metrics,
                    error=execution.stderr[-2000:] or "harness execution failed",
                )
                return SubmissionEvaluation(result, execution, violations)
            content = execution.stdout
        assert content is not None
        try:
            parsed = parse_jsonl(content)
        except SchemaError as exc:
            result = EvaluationResult(
                passed=False,
                score=0.0,
                conversation_count=0,
                total_messages=0,
                size_bytes=len(content.encode("utf-8")),
                metrics=evaluate_dataset([], size_bytes=len(content.encode("utf-8"))).metrics,
                error=str(exc),
            )
            return SubmissionEvaluation(result, execution, violations)
        result = evaluate_dataset(
            parsed.conversations,
            size_bytes=parsed.metadata.size_bytes,
            min_conversations=self.settings.min_conversations,
            max_conversations=self.settings.max_conversations,
        )
        return SubmissionEvaluation(result, execution, violations)

    async def _execute_harness(self, submission_id: str, code: str) -> HarnessExecutionResult:
        if self.settings.docker_enabled:
            return await asyncio.to_thread(self._execute_harness_docker, submission_id, code)
        if self.settings.direct_subprocess_enabled:
            return await self._execute_harness_subprocess(code)
        return HarnessExecutionResult(
            stdout="",
            stderr=(
                "harness execution requires Docker or "
                "DATA_FABRICATION_DIRECT_SUBPROCESS_ENABLED=true"
            ),
            returncode=78,
            duration_seconds=0.0,
        )

    def _execute_harness_docker(self, submission_id: str, code: str) -> HarnessExecutionResult:
        with tempfile.TemporaryDirectory(prefix=f"data-fabrication-{submission_id[:12]}-") as tmp:
            workspace = Path(tmp)
            harness = workspace / "harness.py"
            harness.write_text(code, encoding="utf-8")
            executor = DockerExecutor(
                challenge=self.settings.slug,
                docker_bin=self.settings.docker_bin,
                allowed_images=tuple(self.settings.docker_allowed_images),
                backend=self.settings.docker_backend,
                broker_url=self.settings.docker_broker_url,
                broker_token=self.settings.docker_broker_token,
                broker_token_file=self.settings.docker_broker_token_file,
            )
            result = executor.run(
                DockerRunSpec(
                    image=self.settings.docker_allowed_images[0].rstrip("*"),
                    command=("python", "/workspace/harness.py"),
                    mounts=(DockerMount(workspace, "/workspace"),),
                    workdir="/workspace",
                    labels={
                        "platform.job": submission_id,
                        "platform.task": "dataset-generation",
                    },
                    limits=DockerLimits(
                        cpus=self.settings.docker_cpus,
                        memory=self.settings.docker_memory,
                        memory_swap=self.settings.docker_memory_swap,
                        pids_limit=self.settings.docker_pids_limit,
                        network=self.settings.docker_network,
                        read_only=self.settings.docker_read_only,
                        user=self.settings.docker_user,
                    ),
                ),
                self.settings.evaluation_timeout_seconds,
            )
        return HarnessExecutionResult(
            stdout=result.stdout,
            stderr=result.stderr,
            returncode=result.returncode,
            duration_seconds=0.0,
            timed_out=result.timed_out,
        )

    async def _execute_harness_subprocess(self, code: str) -> HarnessExecutionResult:
        with tempfile.TemporaryDirectory(prefix="data-fabrication-dev-") as tmp:
            harness = Path(tmp) / "harness.py"
            harness.write_text(code, encoding="utf-8")
            started = asyncio.get_running_loop().time()
            proc = await asyncio.create_subprocess_exec(
                "python3",
                str(harness),
                cwd=tmp,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
            )
            try:
                stdout, stderr = await asyncio.wait_for(
                    proc.communicate(),
                    timeout=self.settings.evaluation_timeout_seconds,
                )
                timed_out = False
            except TimeoutError:
                proc.kill()
                stdout, stderr = await proc.communicate()
                timed_out = True
        duration = asyncio.get_running_loop().time() - started
        return HarnessExecutionResult(
            stdout=stdout.decode("utf-8", errors="replace")[-self.settings.max_output_size_bytes :],
            stderr=stderr.decode("utf-8", errors="replace")[-self.settings.max_output_size_bytes :],
            returncode=proc.returncode or 124 if timed_out else proc.returncode or 0,
            duration_seconds=duration,
            timed_out=timed_out,
        )
