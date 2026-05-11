# AGENTS.md — Data Fabrication

## Project Purpose

Data Fabrication is a Python Platform SDK challenge service. Miners submit ZIP packages containing the full Python harness code; the service executes the harness, evaluates the generated agentic coding dataset, stores results in SQLite, and exposes Platform-compatible weights.

## Architecture

```
data-fabrication/
├── src/data_fabrication/
│   ├── app.py                 # FastAPI app entrypoint
│   ├── config.py              # Runtime settings
│   ├── db.py                  # SQLite wrapper
│   ├── repository.py          # Persistence and leaderboard queries
│   ├── routes.py              # Public Platform proxy routes
│   ├── weights.py             # Weight computation
│   ├── evaluator/             # ZIP artifacts, JSONL parsing, scoring, AST checks, execution
│   └── sdk/                   # Vendored Platform-compatible helpers
├── tests/
├── pyproject.toml
└── Dockerfile
```

## Commands

```bash
python -m pip install -e ".[dev]"
ruff check src tests
ruff format --check src tests
mypy --config-file pyproject.toml src
pytest tests
python -m compileall src
```

## Guidelines

- Keep the runtime Python 3.12+ compatible.
- Prefer Platform-compatible routes: `/health`, `/version`, `/internal/v1/get_weights`.
- Use the vendored Docker executor for untrusted harness execution in production.
- `/submit` must accept ZIP harness packages only, never direct datasets or loose code.
- Direct subprocess execution is for local development/tests only and must remain opt-in.
- Never log shared tokens, miner secrets, or raw bearer headers.
