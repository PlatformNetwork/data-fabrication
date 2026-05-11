"""Public challenge routes proxied by Platform."""

from __future__ import annotations

from fastapi import APIRouter, BackgroundTasks, Depends, HTTPException, Request

from .evaluator.execution import DataFabricationEvaluator
from .models import (
    DatasetConsensusResponse,
    DatasetResponse,
    LeaderboardEntry,
    StatsResponse,
    StatusResponse,
    SubmissionCreate,
    SubmissionDetail,
    SubmissionResponse,
    WeightResponse,
)
from .repository import DataFabricationRepository
from .sdk.decorators import public_route

router = APIRouter()


def repo_from_request(request: Request) -> DataFabricationRepository:
    return request.app.state.repository


def evaluator_from_request(request: Request) -> DataFabricationEvaluator:
    return request.app.state.evaluator


@public_route(tags=["submissions"], auth_required=True)
@router.post("/submit", response_model=SubmissionResponse)
async def submit(
    payload: SubmissionCreate,
    background_tasks: BackgroundTasks,
    request: Request,
    repository: DataFabricationRepository = Depends(repo_from_request),
) -> SubmissionResponse:
    return await _create_and_schedule(payload, background_tasks, request, repository)


@public_route(tags=["submissions"], auth_required=True)
@router.post("/v1/submissions", response_model=SubmissionResponse)
async def submit_v1(
    payload: SubmissionCreate,
    background_tasks: BackgroundTasks,
    request: Request,
    repository: DataFabricationRepository = Depends(repo_from_request),
) -> SubmissionResponse:
    return await _create_and_schedule(payload, background_tasks, request, repository)


@public_route(tags=["submissions"])
@router.get("/submissions", response_model=list[SubmissionResponse])
async def list_submissions(
    repository: DataFabricationRepository = Depends(repo_from_request),
) -> list[SubmissionResponse]:
    return await repository.list_submissions()


@public_route(tags=["submissions"])
@router.get("/submissions/{submission_id}", response_model=SubmissionDetail)
async def get_submission(
    submission_id: str,
    repository: DataFabricationRepository = Depends(repo_from_request),
) -> SubmissionDetail:
    submission = await repository.get_submission(submission_id)
    if submission is None:
        raise HTTPException(status_code=404, detail="submission not found")
    return submission


@public_route(tags=["leaderboard"])
@router.get("/leaderboard", response_model=list[LeaderboardEntry])
async def leaderboard(
    repository: DataFabricationRepository = Depends(repo_from_request),
) -> list[LeaderboardEntry]:
    return await repository.leaderboard()


@public_route(tags=["weights"])
@router.get("/get_weights", response_model=WeightResponse)
async def public_weights(
    repository: DataFabricationRepository = Depends(repo_from_request),
) -> WeightResponse:
    return WeightResponse(weights=await repository.weights())


@public_route(tags=["status"])
@router.get("/status", response_model=StatusResponse)
async def status(request: Request) -> StatusResponse:
    stats = await request.app.state.repository.stats()
    settings = request.app.state.settings
    return StatusResponse(
        slug=settings.slug,
        status="ok",
        evaluation_enabled=True,
        upload_enabled=settings.public_submissions_enabled,
        total_submissions=stats.total_submissions,
    )


@public_route(tags=["status"])
@router.get("/stats", response_model=StatsResponse)
async def stats(
    repository: DataFabricationRepository = Depends(repo_from_request),
) -> StatsResponse:
    return await repository.stats()


@public_route(tags=["dataset"])
@router.get("/dataset", response_model=DatasetResponse)
async def dataset(request: Request) -> DatasetResponse:
    settings = request.app.state.settings
    return DatasetResponse(
        min_conversations=settings.min_conversations,
        max_conversations=settings.max_conversations,
    )


@public_route(tags=["dataset"])
@router.get("/dataset/history")
async def dataset_history() -> dict[str, list[dict[str, object]]]:
    return {"history": []}


@public_route(tags=["dataset"])
@router.get("/dataset/consensus", response_model=DatasetConsensusResponse)
async def dataset_consensus() -> DatasetConsensusResponse:
    return DatasetConsensusResponse(status="local", validator_count=1, agreement_level=1.0)


@public_route(tags=["agents"])
@router.get("/agent/{hotkey}", response_model=SubmissionDetail)
async def agent(
    hotkey: str,
    repository: DataFabricationRepository = Depends(repo_from_request),
) -> SubmissionDetail:
    submission = await repository.get_agent(hotkey)
    if submission is None:
        raise HTTPException(status_code=404, detail="agent not found")
    return submission


@public_route(tags=["agents"])
@router.get("/agent/{hotkey}/logs")
async def agent_logs(
    hotkey: str,
    repository: DataFabricationRepository = Depends(repo_from_request),
) -> dict[str, str]:
    submission = await repository.get_agent(hotkey)
    if submission is None:
        raise HTTPException(status_code=404, detail="agent not found")
    return {"stdout": submission.stdout, "stderr": submission.stderr}


@public_route(tags=["agents"])
@router.get("/agent/{hotkey}/code")
async def agent_code(
    hotkey: str,
    repository: DataFabricationRepository = Depends(repo_from_request),
) -> dict[str, str]:
    submission = await repository.get_agent(hotkey)
    if submission is None:
        raise HTTPException(status_code=404, detail="agent not found")
    return {"submission_id": submission.id, "code_hash": submission.code_hash}


@public_route(tags=["results"])
@router.get("/results/{submission_id}", response_model=SubmissionDetail)
async def results(
    submission_id: str,
    repository: DataFabricationRepository = Depends(repo_from_request),
) -> SubmissionDetail:
    return await get_submission(submission_id, repository)


async def _create_and_schedule(
    payload: SubmissionCreate,
    background_tasks: BackgroundTasks,
    request: Request,
    repository: DataFabricationRepository,
) -> SubmissionResponse:
    settings = request.app.state.settings
    if not settings.public_submissions_enabled:
        raise HTTPException(status_code=404, detail="submission route disabled")
    try:
        submission_id, dataset_jsonl, code = await repository.create_submission(payload)
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    if dataset_jsonl is not None or code is not None:
        background_tasks.add_task(
            _evaluate_submission,
            repository,
            request.app.state.evaluator,
            submission_id,
            dataset_jsonl,
            code,
        )
    submission = await repository.get_submission(submission_id)
    assert submission is not None
    return SubmissionResponse(
        **submission.model_dump(exclude={"metrics", "violations", "stdout", "stderr"})
    )


async def _evaluate_submission(
    repository: DataFabricationRepository,
    evaluator: DataFabricationEvaluator,
    submission_id: str,
    dataset_jsonl: str | None,
    code: str | None,
) -> None:
    evaluation = await evaluator.evaluate(
        submission_id=submission_id,
        dataset_jsonl=dataset_jsonl,
        harness_code=code,
    )
    await repository.mark_evaluated(submission_id, evaluation)
