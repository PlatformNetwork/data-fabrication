# Miner Guide

## Purpose

Data Fabrication rewards miners for producing high-quality agentic coding conversation datasets.
Your submission is a harness package that generates JSONL conversations, and your score reflects the
quality, diversity, verifiability, and originality of the generated data.

## Miner Flow

1. Build a complete harness that generates agentic coding conversations.
2. Package the harness as a ZIP archive.
3. Submit the archive with your miner hotkey.
4. Track evaluation status, report details, and sample output.
5. Improve the harness based on failures or weak metrics.
6. Submit a new version when it materially improves dataset quality.

## Dataset Contract

Generated conversations should use the agentic coding conversation schema. Each example should
include:

| Field | Purpose |
| --- | --- |
| `task` | Coding task or user request being solved. |
| `tools` | Available tool definitions, when the conversation uses tools. |
| `messages` | Ordered conversation messages. |
| `final` | Final answer or result. |

Expected roles include `system`, `user`, `assistant`, `tool`, and `function`.

## Building A Strong Harness

A strong harness should:

- generate realistic coding tasks;
- include coherent multi-turn trajectories;
- use tool calls only when they make sense;
- produce final answers that match the task;
- vary repositories, bugs, prompts, and solution styles;
- avoid repeated templates and trivial paraphrases;
- include enough reasoning to make the answer auditable;
- stay deterministic enough for reproducible review;
- avoid secrets, network-only dependencies, and unsafe filesystem behavior.

## Submitting A Harness

Submit a complete ZIP package:

```http
POST /submit
Content-Type: application/json
```

```json
{
  "hotkey": "5Abc...",
  "filename": "submission.zip",
  "package_base64": "<base64-encoded-zip>",
  "signature": "optional-hotkey-signature"
}
```

The versioned submission route accepts the same payload:

```http
POST /v1/submissions
```

Submission rules:

- The archive must contain the full harness, not direct dataset output.
- Direct loose files are rejected.
- Unsafe archive paths, symlinks, invalid layouts, and oversized archives are rejected.
- `hotkey` or `miner_hotkey` identifies the miner receiving score credit.
- `filename` should end with `.zip`.

## Tracking Results

List recent submissions:

```http
GET /submissions
```

Read one submission:

```http
GET /submissions/{submission_id}
```

Read the detailed evaluation report:

```http
GET /v1/submissions/{submission_id}/report
```

Read sample generated JSONL:

```http
GET /v1/submissions/{submission_id}/samples
```

Read your latest agent result:

```http
GET /agent/{hotkey}
```

## Leaderboard And Dataset Info

Read current rankings:

```http
GET /leaderboard
```

Read dataset constraints:

```http
GET /dataset
```

Read challenge status and stats:

```http
GET /status
GET /stats
```

## Scoring Model

The final score is normalized to `[0, 1]` and combines:

- dataset quality;
- agentic behavior;
- useful tool and function calls;
- reasoning quality;
- coding relevance;
- verifiability;
- diversity;
- originality.

Similarity and plagiarism checks can reduce or reject a score when submissions appear cloned or only
minimally changed.

## Miner Checklist

Before submitting:

- Generate valid JSONL locally.
- Confirm examples include `task`, `tools`, `messages`, and `final`.
- Remove generated datasets from the archive.
- Remove secrets, local caches, and unnecessary dependencies.
- Keep the archive below the published size limits.
- Confirm the harness can run without private services.
- Make outputs diverse rather than template-heavy.
