"""Python ZIP harness execution and dataset evaluation."""

from __future__ import annotations

import asyncio
import json
import os
import shlex
from dataclasses import asdict, dataclass, replace
from pathlib import Path
from typing import Any

from data_fabrication.config import DataFabricationSettings
from data_fabrication.sdk.executors.docker import (
    DockerExecutor,
    DockerLimits,
    DockerMount,
    DockerRunSpec,
)

from .artifacts import SubmissionArtifact
from .ast_similarity import compare_structures, normalize_ast, plagiarism_status
from .dataset import SchemaError, parse_jsonl
from .judge import JudgeResult, LlmDatasetJudge
from .llm import LlmInference, LlmInferenceConfig
from .scoring import EvaluationResult, evaluate_dataset
from .static_review import StaticReviewReport, review_workspace


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
    static_report: StaticReviewReport | None = None
    judge: JudgeResult | None = None
    plagiarism: dict[str, Any] | None = None
    sample_jsonl: str = ""

    def metrics_json(self) -> str:
        return json.dumps(
            {
                "result": asdict(self.result),
                "execution": None if self.execution is None else asdict(self.execution),
                "violations": self.violations,
                "static_review": None
                if self.static_report is None
                else self.static_report.to_dict(),
                "judge": None if self.judge is None else self.judge.to_dict(),
                "plagiarism": self.plagiarism,
            },
            separators=(",", ":"),
        )


class DataFabricationEvaluator:
    """Evaluate ZIP Python harness submissions."""

    def __init__(self, settings: DataFabricationSettings) -> None:
        self.settings = settings
        self.judge = LlmDatasetJudge(settings)

    async def evaluate(
        self,
        *,
        submission_id: str,
        artifact: SubmissionArtifact,
        prior_artifacts: list[dict[str, Any]] | None = None,
    ) -> SubmissionEvaluation:
        static_report = review_workspace(artifact.workspace_path)
        violations = _violations_from_report(static_report)
        if not static_report.accepted:
            return SubmissionEvaluation(
                result=_failure_result(
                    f"static review rejected harness {static_report.harness_hash}",
                    size_bytes=0,
                ),
                execution=None,
                violations=violations,
                static_report=static_report,
            )

        execution = await self._execute_harness(
            submission_id,
            artifact.workspace_path,
            artifact.entrypoint,
        )
        if execution.returncode != 0 or execution.timed_out:
            error = (
                execution.stderr[-2000:] or execution.stdout[-2000:] or "harness execution failed"
            )
            return SubmissionEvaluation(
                result=_failure_result(
                    error,
                    size_bytes=0,
                ),
                execution=execution,
                violations=violations,
                static_report=static_report,
            )

        content = execution.stdout
        try:
            parsed = parse_jsonl(content)
        except SchemaError as exc:
            return SubmissionEvaluation(
                result=_failure_result(str(exc), size_bytes=len(content.encode("utf-8"))),
                execution=execution,
                violations=violations,
                static_report=static_report,
                sample_jsonl=_sample_jsonl(content, self.settings.sample_preview_limit),
            )

        result = evaluate_dataset(
            parsed.conversations,
            size_bytes=parsed.metadata.size_bytes,
            min_conversations=self.settings.min_conversations,
            max_conversations=self.settings.max_conversations,
        )
        sample_jsonl = _sample_jsonl(content, self.settings.sample_preview_limit)
        plagiarism = await self._nearest_plagiarism_report(
            artifact.workspace_path,
            prior_artifacts or [],
        )
        result = _apply_plagiarism_penalty(result, plagiarism)
        judge = await self.judge.evaluate(
            workspace_path=artifact.workspace_path,
            entrypoint=artifact.entrypoint,
            dataset_result=result,
            static_report=static_report,
            sample_jsonl=sample_jsonl,
        )
        result = _combine_judge_score(result, judge)
        return SubmissionEvaluation(
            result=result,
            execution=execution,
            violations=violations,
            static_report=static_report,
            judge=judge,
            plagiarism=plagiarism,
            sample_jsonl=sample_jsonl,
        )

    async def _execute_harness(
        self,
        submission_id: str,
        workspace_path: Path,
        entrypoint: str,
    ) -> HarnessExecutionResult:
        if self.settings.docker_enabled:
            return await asyncio.to_thread(
                self._execute_harness_docker,
                submission_id,
                workspace_path,
                entrypoint,
            )
        if self.settings.direct_subprocess_enabled:
            return await self._execute_harness_subprocess(workspace_path, entrypoint)
        return HarnessExecutionResult(
            stdout="",
            stderr=(
                "harness execution requires Docker or "
                "DATA_FABRICATION_DIRECT_SUBPROCESS_ENABLED=true"
            ),
            returncode=78,
            duration_seconds=0.0,
        )

    def _execute_harness_docker(
        self,
        submission_id: str,
        workspace_path: Path,
        entrypoint: str,
    ) -> HarnessExecutionResult:
        executor = DockerExecutor(
            challenge=self.settings.slug,
            docker_bin=self.settings.docker_bin,
            allowed_images=tuple(self.settings.docker_allowed_images),
            backend=self.settings.docker_backend,
            broker_url=self.settings.docker_broker_url,
            broker_token=self.settings.docker_broker_token,
            broker_token_file=self.settings.docker_broker_token_file,
        )
        command = _runner_shell_command(entrypoint, self.settings.runner_output_filename)
        result = executor.run(
            DockerRunSpec(
                image=self.settings.docker_allowed_images[0].rstrip("*"),
                command=("sh", "-c", command),
                mounts=(DockerMount(workspace_path, "/workspace"),),
                workdir="/workspace",
                env={
                    "DATA_FABRICATION_SEED": str(self.settings.runner_seed),
                    "DATA_FABRICATION_OUTPUT": f"/tmp/{self.settings.runner_output_filename}",
                    "PYTHONUNBUFFERED": "1",
                },
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
            stdout=result.stdout[-self.settings.max_output_size_bytes :],
            stderr=result.stderr,
            returncode=result.returncode,
            duration_seconds=0.0,
            timed_out=result.timed_out,
        )

    async def _execute_harness_subprocess(
        self,
        workspace_path: Path,
        entrypoint: str,
    ) -> HarnessExecutionResult:
        started = asyncio.get_running_loop().time()
        output_path = workspace_path / self.settings.runner_output_filename
        env = {
            **os.environ,
            "DATA_FABRICATION_SEED": str(self.settings.runner_seed),
            "DATA_FABRICATION_OUTPUT": str(output_path),
            "PYTHONUNBUFFERED": "1",
        }
        proc = await asyncio.create_subprocess_exec(
            "python3",
            entrypoint,
            cwd=workspace_path,
            env=env,
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
        output = stdout.decode("utf-8", errors="replace")
        if output_path.is_file() and output_path.stat().st_size > 0:
            output = output_path.read_text(encoding="utf-8", errors="replace")
        return HarnessExecutionResult(
            stdout=output[-self.settings.max_output_size_bytes :],
            stderr=stderr.decode("utf-8", errors="replace")[-self.settings.max_output_size_bytes :],
            returncode=proc.returncode or 124 if timed_out else proc.returncode or 0,
            duration_seconds=duration,
            timed_out=timed_out,
        )

    async def _nearest_plagiarism_report(
        self,
        workspace_path: Path,
        prior_artifacts: list[dict[str, Any]],
    ) -> dict[str, Any] | None:
        current_source = _combined_python_source(workspace_path)
        if not current_source.strip():
            return None
        current_ast = normalize_ast(current_source)
        best: dict[str, Any] | None = None
        best_source = ""
        for artifact in prior_artifacts:
            previous_path = Path(str(artifact.get("workspace_path") or ""))
            if not previous_path.is_dir():
                continue
            previous_source = _combined_python_source(previous_path)
            if not previous_source.strip():
                continue
            try:
                score = compare_structures(current_ast, normalize_ast(previous_source))
            except SyntaxError:
                continue
            if best is None or score > int(best["score"]):
                best = {
                    "submission_id": artifact.get("submission_id"),
                    "hotkey": artifact.get("hotkey"),
                    "score": score,
                    "status": plagiarism_status(score).value,
                }
                best_source = previous_source
        if best is None:
            return None
        if self.settings.llm_plagiarism_enabled and int(best["score"]) >= 30:
            best["llm"] = await self._llm_plagiarism_audit(
                current_source,
                best_source,
                float(best["score"]),
            )
        return best

    async def _llm_plagiarism_audit(
        self,
        current_source: str,
        previous_source: str,
        score: float,
    ) -> dict[str, Any]:
        client = LlmInference(
            LlmInferenceConfig(
                endpoint=self.settings.llm_endpoint,
                model=self.settings.llm_model,
                timeout_seconds=self.settings.llm_timeout_seconds,
                max_retries=self.settings.llm_max_retries,
            )
        )
        verdict = await client.evaluate_plagiarism(previous_source, current_source, score)
        return asdict(verdict)


def _runner_shell_command(entrypoint: str, output_filename: str) -> str:
    quoted_entrypoint = shlex.quote(entrypoint)
    quoted_output = shlex.quote(f"/tmp/{output_filename}")
    return (
        f"python {quoted_entrypoint} > /tmp/stdout.jsonl; status=$?; "
        "if [ $status -ne 0 ]; then cat /tmp/stdout.jsonl; exit $status; fi; "
        f"if [ -s {quoted_output} ]; then cat {quoted_output}; else cat /tmp/stdout.jsonl; fi"
    )


def _violations_from_report(report: StaticReviewReport) -> list[dict[str, object]]:
    violations: list[dict[str, object]] = []
    for file_review in report.files:
        for violation in file_review.violations:
            violations.append(
                {
                    "file": file_review.path,
                    "pattern": violation.pattern,
                    "severity": violation.severity.value,
                    "line": violation.line,
                    "column": violation.column,
                }
            )
        for pattern in file_review.suspicious_patterns:
            violations.append(
                {
                    "file": file_review.path,
                    "pattern": pattern,
                    "severity": "warning",
                    "line": None,
                    "column": None,
                }
            )
    return violations


def _failure_result(error: str, *, size_bytes: int) -> EvaluationResult:
    return EvaluationResult(
        passed=False,
        score=0.0,
        conversation_count=0,
        total_messages=0,
        size_bytes=size_bytes,
        metrics=evaluate_dataset([], size_bytes=size_bytes).metrics,
        error=error,
    )


def _sample_jsonl(content: str, limit: int) -> str:
    lines = [line for line in content.splitlines() if line.strip()]
    return "\n".join(lines[:limit])


def _combined_python_source(workspace_path: Path) -> str:
    chunks = []
    for path in sorted(workspace_path.rglob("*.py")):
        chunks.append(f"# file: {path.relative_to(workspace_path).as_posix()}\n")
        chunks.append(path.read_text(encoding="utf-8", errors="replace"))
    return "\n".join(chunks)


def _apply_plagiarism_penalty(
    result: EvaluationResult,
    plagiarism: dict[str, Any] | None,
) -> EvaluationResult:
    if plagiarism is None:
        return result
    status = str(plagiarism.get("status") or "clean")
    multiplier = 1.0
    error = result.error
    llm = plagiarism.get("llm")
    if (
        isinstance(llm, dict)
        and llm.get("is_plagiarism")
        and float(llm.get("confidence", 0.0)) >= 0.75
    ):
        status = "plagiarized"
    if status == "plagiarized":
        multiplier = 0.2
        error = "nearest-neighbor code plagiarism detected"
    elif status == "needs_llm_verification":
        multiplier = 0.85
    score = result.score * multiplier
    return replace(result, score=score, passed=result.passed and score >= 0.5, error=error)


def _combine_judge_score(result: EvaluationResult, judge: JudgeResult) -> EvaluationResult:
    score = result.score * 0.85 + judge.overall_score * 0.15
    passed = result.passed and score >= 0.5 and judge.anti_cheat_score >= 0.4
    return replace(result, score=score, passed=passed)
