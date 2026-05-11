"""Tool-assisted LLM judge for harness and dataset quality."""

from __future__ import annotations

import asyncio
import json
import re
import shlex
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

import httpx

from data_fabrication.config import DataFabricationSettings

from .scoring import EvaluationResult
from .static_review import StaticReviewReport


@dataclass(frozen=True)
class ToolEvent:
    name: str
    arguments: dict[str, Any]
    result: Any


@dataclass(frozen=True)
class JudgeResult:
    overall_score: float
    quality_score: float
    completeness_score: float
    anti_cheat_score: float
    reasoning: str
    tool_events: list[ToolEvent]
    llm_used: bool = False

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


class JudgeToolSandbox:
    """Restricted file and process tools exposed to the evaluation judge."""

    def __init__(self, root: Path, *, timeout_seconds: float = 5.0) -> None:
        self.root = root.resolve()
        self.timeout_seconds = timeout_seconds

    def list_dir(self, path: str = ".") -> list[str]:
        base = self._resolve(path)
        if not base.is_dir():
            return []
        return sorted(child.relative_to(self.root).as_posix() for child in base.iterdir())[:200]

    def read_file(self, path: str, *, max_bytes: int = 8_000) -> str:
        target = self._resolve(path)
        if not target.is_file():
            return ""
        return target.read_bytes()[:max_bytes].decode("utf-8", errors="replace")

    def grep(self, pattern: str, path: str = ".") -> list[dict[str, Any]]:
        base = self._resolve(path)
        regex = re.compile(pattern)
        files = [base] if base.is_file() else sorted(base.rglob("*"))
        matches: list[dict[str, Any]] = []
        for file_path in files:
            if not file_path.is_file() or file_path.suffix not in {".py", ".md", ".yaml", ".yml"}:
                continue
            for line_num, line in enumerate(
                file_path.read_text(encoding="utf-8", errors="replace").splitlines(),
                start=1,
            ):
                if regex.search(line):
                    matches.append(
                        {
                            "path": file_path.relative_to(self.root).as_posix(),
                            "line": line_num,
                            "text": line[:240],
                        }
                    )
                if len(matches) >= 100:
                    return matches
        return matches

    async def bash(self, command: str) -> dict[str, Any]:
        argv = shlex.split(command)
        if not self._allowed_command(argv):
            return {"returncode": 126, "stdout": "", "stderr": "command is not allowed"}
        proc = await asyncio.create_subprocess_exec(
            *argv,
            cwd=self.root,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        try:
            stdout, stderr = await asyncio.wait_for(proc.communicate(), self.timeout_seconds)
        except TimeoutError:
            proc.kill()
            stdout, stderr = await proc.communicate()
            return {
                "returncode": 124,
                "stdout": stdout.decode("utf-8", errors="replace")[-4000:],
                "stderr": stderr.decode("utf-8", errors="replace")[-4000:],
            }
        return {
            "returncode": proc.returncode or 0,
            "stdout": stdout.decode("utf-8", errors="replace")[-4000:],
            "stderr": stderr.decode("utf-8", errors="replace")[-4000:],
        }

    def _resolve(self, path: str) -> Path:
        target = (self.root / path).resolve()
        if target != self.root and self.root not in target.parents:
            raise ValueError("path escapes judge sandbox")
        return target

    def _allowed_command(self, argv: list[str]) -> bool:
        if not argv:
            return False
        if argv[:2] == ["python", "-m"] and len(argv) >= 3:
            return argv[2] in {"py_compile", "compileall"}
        if argv[0] == "python" and len(argv) == 2 and argv[1].endswith(".py"):
            return True
        return argv[0] in {"wc", "head"} and len(argv) <= 4


class LlmDatasetJudge:
    """Evaluate generated agentic coding datasets with local tools and optional LLM review."""

    def __init__(self, settings: DataFabricationSettings) -> None:
        self.settings = settings

    async def evaluate(
        self,
        *,
        workspace_path: Path,
        entrypoint: str,
        dataset_result: EvaluationResult,
        static_report: StaticReviewReport,
        sample_jsonl: str,
    ) -> JudgeResult:
        sandbox = JudgeToolSandbox(workspace_path, timeout_seconds=10.0)
        tool_events = await self._collect_tool_evidence(sandbox, entrypoint)
        heuristic = self._heuristic_result(dataset_result, static_report, tool_events)
        if not self.settings.llm_judge_enabled:
            return heuristic
        try:
            return await self._llm_refine(heuristic, tool_events, sample_jsonl)
        except Exception as exc:
            return JudgeResult(
                overall_score=heuristic.overall_score,
                quality_score=heuristic.quality_score,
                completeness_score=heuristic.completeness_score,
                anti_cheat_score=heuristic.anti_cheat_score,
                reasoning=f"{heuristic.reasoning} LLM judge unavailable: {exc}",
                tool_events=heuristic.tool_events,
                llm_used=False,
            )

    async def _collect_tool_evidence(
        self, sandbox: JudgeToolSandbox, entrypoint: str
    ) -> list[ToolEvent]:
        events = [
            ToolEvent("list_dir", {"path": "."}, sandbox.list_dir(".")),
            ToolEvent(
                "grep",
                {"pattern": "open\\(|subprocess|socket|requests|httpx|eval\\(|exec\\("},
                sandbox.grep("open\\(|subprocess|socket|requests|httpx|eval\\(|exec\\("),
            ),
        ]
        for manifest in ("data_fabrication.yaml", "data_fabrication.yml", "manifest.yaml"):
            content = sandbox.read_file(manifest)
            if content:
                events.append(ToolEvent("read_file", {"path": manifest}, content))
                break
        compile_result = await sandbox.bash(f"python -m py_compile {shlex.quote(entrypoint)}")
        events.append(
            ToolEvent("bash", {"command": f"python -m py_compile {entrypoint}"}, compile_result)
        )
        return events

    def _heuristic_result(
        self,
        dataset_result: EvaluationResult,
        static_report: StaticReviewReport,
        tool_events: list[ToolEvent],
    ) -> JudgeResult:
        metrics = dataset_result.metrics
        quality = _clamp(
            (
                metrics.quality_score
                + metrics.agentic_score
                + metrics.reasoning_score
                + metrics.function_call_score
                + metrics.coding_relevance_score
            )
            / 5
        )
        completeness = _clamp(
            (metrics.format_score + metrics.verifiability_score + float(dataset_result.passed)) / 3
        )
        compile_event = next((event for event in tool_events if event.name == "bash"), None)
        compile_ok = bool(
            isinstance(compile_event, ToolEvent)
            and isinstance(compile_event.result, dict)
            and compile_event.result.get("returncode") == 0
        )
        anti_cheat = _clamp(
            1.0
            - static_report.critical_count * 0.5
            - static_report.warning_count * 0.05
            - static_report.suspicious_count * 0.2
            + (0.05 if compile_ok else -0.1)
        )
        overall = _clamp(quality * 0.55 + completeness * 0.25 + anti_cheat * 0.20)
        return JudgeResult(
            overall_score=overall,
            quality_score=quality,
            completeness_score=completeness,
            anti_cheat_score=anti_cheat,
            reasoning=(
                "Tool-assisted heuristic judge scored dataset quality, schema completeness, "
                "and anti-cheat signals."
            ),
            tool_events=tool_events,
        )

    async def _llm_refine(
        self,
        heuristic: JudgeResult,
        tool_events: list[ToolEvent],
        sample_jsonl: str,
    ) -> JudgeResult:
        payload = {
            "model": self.settings.llm_model,
            "stream": False,
            "messages": [
                {
                    "role": "user",
                    "content": _judge_prompt(heuristic, tool_events, sample_jsonl),
                }
            ],
        }
        async with httpx.AsyncClient(timeout=self.settings.llm_timeout_seconds) as client:
            response = await client.post(self.settings.llm_endpoint, json=payload)
            response.raise_for_status()
        data = _parse_json_object(response.text)
        return JudgeResult(
            overall_score=_clamp(float(data.get("overall_score", heuristic.overall_score))),
            quality_score=_clamp(float(data.get("quality_score", heuristic.quality_score))),
            completeness_score=_clamp(
                float(data.get("completeness_score", heuristic.completeness_score))
            ),
            anti_cheat_score=_clamp(
                float(data.get("anti_cheat_score", heuristic.anti_cheat_score))
            ),
            reasoning=str(data.get("reasoning") or heuristic.reasoning),
            tool_events=tool_events,
            llm_used=True,
        )


def _judge_prompt(heuristic: JudgeResult, tool_events: list[ToolEvent], sample_jsonl: str) -> str:
    return f"""You are judging a miner ZIP harness that generates agentic coding RLHF datasets.
Use the tool evidence and dataset sample to grade quality, completeness, and anti-cheat behavior.
Return one JSON object with numeric scores in [0,1]: overall_score, quality_score,
completeness_score, anti_cheat_score, and a concise reasoning string.

Heuristic:
{json.dumps(heuristic.to_dict(), default=str)[:6000]}

Tool evidence:
{json.dumps([asdict(event) for event in tool_events], default=str)[:8000]}

Dataset sample:
{sample_jsonl[:8000]}
"""


def _parse_json_object(body: str) -> dict[str, Any]:
    start = body.find("{")
    end = body.rfind("}")
    if start < 0 or end < start:
        raise ValueError("LLM judge did not return JSON")
    data = json.loads(body[start : end + 1])
    if not isinstance(data, dict):
        raise ValueError("LLM judge returned non-object JSON")
    return data


def _clamp(value: float) -> float:
    if value != value:
        return 0.0
    return max(0.0, min(1.0, value))
