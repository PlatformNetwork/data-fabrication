"""FastAPI application entrypoint for Data Fabrication."""

from __future__ import annotations

from pathlib import Path
from typing import Annotated

from fastapi import BackgroundTasks, Depends, FastAPI, Header, HTTPException, Request

from .config import DataFabricationSettings, settings
from .db import Database
from .evaluator.execution import DataFabricationEvaluator
from .models import SubmissionCreate, SubmissionResponse
from .repository import DataFabricationRepository
from .routes import _evaluate_submission, router
from .sdk import create_challenge_app
from .sdk.auth import build_internal_auth_dependency
from .weights import get_weights


def create_app(app_settings: DataFabricationSettings = settings) -> FastAPI:
    """Create the Data Fabrication FastAPI app."""

    database = Database(app_settings.resolved_database_path)
    repository = DataFabricationRepository(
        database,
        max_submission_size_bytes=app_settings.max_submission_size_bytes,
        artifact_root=Path(app_settings.artifact_root),
        max_zip_files=app_settings.max_zip_files,
        max_zip_uncompressed_bytes=app_settings.max_zip_uncompressed_bytes,
    )
    evaluator = DataFabricationEvaluator(app_settings)

    async def get_weights_fn() -> dict[str, float]:
        return await get_weights(repository)

    app = create_challenge_app(
        settings=app_settings,
        database=database,
        public_router=router,
        get_weights_fn=get_weights_fn,
    )
    app.state.settings = app_settings
    app.state.database = database
    app.state.repository = repository
    app.state.evaluator = evaluator

    @app.post(
        "/internal/v1/bridge/submissions",
        response_model=SubmissionResponse,
        dependencies=[Depends(build_internal_auth_dependency(app_settings))],
    )
    async def bridge_submission(
        request: Request,
        background_tasks: BackgroundTasks,
        x_platform_verified_hotkey: Annotated[str, Header(min_length=1, max_length=128)],
        x_submission_filename: Annotated[str | None, Header()] = None,
    ) -> SubmissionResponse:
        body = await request.body()
        if len(body) > app_settings.max_submission_size_bytes:
            raise HTTPException(status_code=413, detail="submission too large")
        payload = SubmissionCreate(
            hotkey=x_platform_verified_hotkey,
            filename=x_submission_filename,
        )
        try:
            submission_id = await repository.create_submission(
                payload,
                verified_hotkey=x_platform_verified_hotkey,
                raw_package=body,
                filename=x_submission_filename or "submission.zip",
            )
        except ValueError as exc:
            raise HTTPException(status_code=400, detail=str(exc)) from exc
        background_tasks.add_task(
            _evaluate_submission,
            repository,
            evaluator,
            submission_id,
        )
        submission = await repository.get_submission(submission_id)
        assert submission is not None
        return SubmissionResponse(
            **submission.model_dump(exclude={"metrics", "violations", "stdout", "stderr"})
        )

    return app


app = create_app()
