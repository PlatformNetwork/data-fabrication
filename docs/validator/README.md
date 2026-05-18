# Validator Guide

## Purpose

Data Fabrication lets validators operate a dataset-generation challenge for Platform. Validators
accept miner harness packages, run safety and quality evaluation, persist results, and expose the
best completed score per hotkey as Platform weights.

## Responsibilities

Validators are responsible for:

- accepting only valid ZIP harness packages;
- keeping execution limits strict;
- protecting shared Platform tokens;
- monitoring failed, rejected, and plagiarized submissions;
- preserving evaluation records for audit;
- exposing leaderboard, dataset metadata, and score-derived weights.

## Evaluation Lifecycle

1. A miner submits a harness package and hotkey.
2. The archive is decoded, validated, and safely extracted.
3. Static review checks for unsafe code and invalid structure.
4. The harness generates JSONL conversations.
5. The output is parsed against the dataset schema.
6. Quality, agentic behavior, diversity, and verifiability are scored.
7. Similarity checks compare harness structure against previous submissions.
8. The result, metrics, logs, and violations are stored.
9. Platform reads the best completed score per hotkey.

## Runtime Configuration

Project-specific settings use the `DATA_FABRICATION_` environment prefix. Platform orchestrators can
also provide compatible `CHALLENGE_` settings.

| Setting | Purpose |
| --- | --- |
| `DATA_FABRICATION_DATABASE_URL` | Persistent storage location. |
| `DATA_FABRICATION_SHARED_TOKEN` | Token for protected Platform weight calls. |
| `DATA_FABRICATION_SHARED_TOKEN_FILE` | File containing the shared token. |
| `DATA_FABRICATION_ARTIFACT_ROOT` | Directory for uploaded harness artifacts. |
| `DATA_FABRICATION_MAX_SUBMISSION_SIZE_BYTES` | Encoded request size limit. |
| `DATA_FABRICATION_MAX_ZIP_FILES` | Maximum files in an archive. |
| `DATA_FABRICATION_MAX_ZIP_UNCOMPRESSED_BYTES` | Maximum uncompressed archive size. |
| `DATA_FABRICATION_EVALUATION_TIMEOUT_SECONDS` | Harness execution timeout. |
| `DATA_FABRICATION_MIN_CONVERSATIONS` | Minimum accepted conversations. |
| `DATA_FABRICATION_MAX_CONVERSATIONS` | Maximum scored conversations. |
| `DATA_FABRICATION_PUBLIC_SUBMISSIONS_ENABLED` | Enables or disables public submissions. |
| `DATA_FABRICATION_DIRECT_SUBPROCESS_ENABLED` | Local-only execution mode for development. |
| `DATA_FABRICATION_LLM_PLAGIARISM_ENABLED` | Enables optional semantic plagiarism review. |
| `DATA_FABRICATION_LLM_JUDGE_ENABLED` | Enables optional LLM quality review. |

Use secret files in production so shared tokens are not exposed in shell history or process lists.

## Public Miner Surface

Miners and dashboards use:

```http
POST /submit
POST /v1/submissions
GET /submissions
GET /submissions/{submission_id}
GET /v1/submissions/{submission_id}/report
GET /v1/submissions/{submission_id}/samples
GET /leaderboard
GET /status
GET /stats
GET /dataset
GET /dataset/consensus
GET /agent/{hotkey}
GET /results/{submission_id}
```

## Platform Contract

Health check:

```http
GET /health
```

Version and capability check:

```http
GET /version
```

Weight request:

```http
GET /internal/v1/get_weights
Authorization: Bearer <shared-token>
X-Platform-Challenge-Slug: data-fabrication
```

Example response:

```json
{
  "challenge_slug": "data-fabrication",
  "epoch": 1760000000,
  "weights": {
    "5Abc...": 0.82
  }
}
```

## Scoring And Weights

Only completed, positive scores are exported as raw weights. Platform normalizes those values before
submitting final subnet weights.

Validators should inspect:

- total submissions;
- completed submissions;
- active miners;
- best score;
- violations;
- static review results;
- optional judge or plagiarism review output;
- stdout and stderr for failed harnesses.

## Operator Checklist

Before accepting traffic:

1. Configure persistent storage and artifact root.
2. Configure shared Platform token delivery.
3. Set archive and output size limits.
4. Decide whether optional LLM review is enabled.
5. Keep direct subprocess execution disabled in production.
6. Submit a known-good harness.
7. Confirm status, stats, report, samples, leaderboard, and weights.

During operation:

- monitor repeated archive validation failures;
- review high-similarity submissions;
- tune conversation count and output limits;
- back up persistent evaluation records;
- rotate shared tokens if exposed;
- keep execution isolated for untrusted miner code.

## Security Checklist

- Reject direct datasets and loose code.
- Reject unsafe archive paths and symlinks.
- Keep token values out of logs.
- Run untrusted harnesses with strict time, memory, and process limits.
- Cap stored logs and generated output size.
- Treat optional LLM reviews as advisory unless configured as required.
