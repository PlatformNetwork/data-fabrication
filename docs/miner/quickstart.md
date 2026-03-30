# Miner Quickstart

Step-by-step guide to submitting a conversation dataset harness on the Bittensor network.

---

## Prerequisites

| Requirement | Details |
| --- | --- |
| Python | 3.10+ |
| Rust | 1.70+ (for building df-cli from source) |
| Bittensor | Registered hotkey with stake on the subnet |
| df-cli | See [Install](#install-the-cli) below |

---

## Install the CLI

```bash
# Via Platform CLI (recommended)
platform download data-fabrication

# Or build from source (requires cloned repo)
git clone https://github.com/PlatformNetwork/data-fabrication
cd data-fabrication
cargo build --release -p df-cli
```

---

## Overview

```
1. Write harness  →  Python code that generates conversation datasets
2. Submit code     →  df-cli submit (sends code for validation)
3. Wait           →  Validators run AST similarity + LLM plagiarism check
4. Execute        →  Harness runs in sandbox, produces JSONL output
5. Monitor        →  df-cli status / df-cli monitor
```

**Harnesses run in sandboxed environments with:**
- CPU and memory limits
- Network isolation
- File size constraints
- Timeout enforcement

---

## Step 1: Write Your Harness

Your harness must be a Python script that outputs JSONL conversation entries.

### Harness Structure

```
my-harness/
├── generate.py       # Main entry point (required)
└── requirements.txt  # Python dependencies (optional)
```

### Minimal Example

```python
#!/usr/bin/env python3
"""
Conversation Dataset Generator Harness
Generates synthetic conversation data for AI training
"""
import json
import sys

def generate_conversation(prompt: str) -> dict:
    """Generate a single conversation entry."""
    return {
        "prompt": prompt,
        "response": f"Generated response for: {prompt}",
        "metadata": {
            "temperature": 0.7,
            "model": "example-model"
        }
    }

def main():
    conversations = []
    for i in range(10):
        conv = generate_conversation(f"Question {i+1}")
        conversations.append(conv)
    
    # Output as JSONL (one JSON object per line)
    for conv in conversations:
        print(json.dumps(conv))

if __name__ == "__main__":
    main()
```

### Output Format

Each line must be a valid JSON object with this structure:

```json
{"prompt": "Your question here", "response": "Your answer here", "metadata": {...}}
```

---

## Step 2: Submit for Review

```bash
df-cli submit --harness ./my-harness/
```

Validators run:
1. **Format Validation** — Checks JSONL output structure
2. **AST Structural Similarity** — Compares code structure against other submissions
3. **LLM Plagiarism Detection** — Semantic analysis if similarity threshold exceeded

All checks must pass for acceptance.

---

## Step 3: Monitor Evaluation

```bash
# Real-time TUI
df-cli monitor

# Check submission status
df-cli status --hotkey <YOUR_HOTKEY>

# Custom RPC endpoint
df-cli --rpc-url http://localhost:8080 monitor
```

**TUI Controls:**
- `Tab`/`Shift+Tab` — Switch tabs
- `↑`/`↓` — Scroll
- `r` — Refresh
- `q` — Quit

---

## Evaluation Process

```
┌─────────────────┐
│  Submit Code    │
└────────┬────────┘
         ▼
┌─────────────────┐
│ Format Check    │──── FAIL ────► Rejected
└────────┬────────┘
         │ PASS
         ▼
┌─────────────────┐
│ AST Similarity  │──── ≥80% ────► Plagiarism Threshold
└────────┬────────┘
         │ <80%
         ▼
┌─────────────────┐
│ LLM Plagiarism  │──── FAIL ────► Flagged
└────────┬────────┘
         │ PASS
         ▼
┌─────────────────┐
│ Execute Harness │──── Timeout ──► Score Penalty
└────────┬────────┘
         │ Success
         ▼
┌─────────────────┐
│ Quality Score   │
└─────────────────┘
```

### Scoring Metrics

| Metric | Weight |
|--------|--------|
| Format compliance | 20% |
| Dataset quality | 40% |
| Originality score | 30% |
| Execution speed | 10% |

---

## Tips for Passing Plagiarism Detection

### Understand AST Normalization

Variable names are normalized before comparison. This means:

```python
# These are IDENTICAL after normalization
x = 1
y = x + 2

# Same as:
count = 1
total = count + 2
```

Both normalize to `var_0 = 1; var_1 = var_0 + 2`.

### Write Original Logic

- **Avoid copying** common patterns verbatim
- **Add unique logic** specific to your approach
- **Use different control flow** — loops vs comprehensions, if/else vs match
- **Include meaningful comments** to distinguish your work

### Check Before Submitting

```bash
# Test your harness locally first
python3 ./my-harness/generate.py > output.jsonl

# Validate JSONL format
cat output.jsonl | python3 -c "import sys,json;[json.loads(l) for l in sys.stdin]"
```

### Structural Diversity

Avoid these common patterns that trigger similarity flags:
- Identical function signatures
- Same loop structures with renamed variables
- Copied docstrings from other submissions

---

## Example: Advanced Harness

Here's a more sophisticated harness with configurable parameters:

```python
#!/usr/bin/env python3
"""
Advanced Conversation Dataset Generator
Uses templates and randomness for variety
"""
import json
import random
import hashlib
from datetime import datetime

TEMPLATES = [
    "Explain {topic} in simple terms",
    "What are the key differences between {a} and {b}?",
    "How does {concept} relate to {other}?",
]

def generate_prompt(template_idx: int, **kwargs) -> str:
    return TEMPLATES[template_idx].format(**kwargs)

def generate_response(prompt: str, style: str = "informative") -> dict:
    # Your response generation logic here
    seed = hashlib.md5(prompt.encode()).hexdigest()[:8]
    return {
        "response": f"[{style.upper()}] Response generated with seed {seed}",
        "confidence": round(random.uniform(0.7, 0.95), 2),
        "sources": ["internal-knowledge"]
    }

def main():
    topics = ["machine learning", "databases", "networking", "security"]
    count = int(os.environ.get("CONVERSATION_COUNT", 50))
    
    for i in range(count):
        template = random.randint(0, len(TEMPLATES) - 1)
        topic = random.choice(topics)
        prompt = generate_prompt(template, topic=topic, a="X", b="Y", concept="A", other="B")
        response = generate_response(prompt, style=random.choice(["informative", "concise"]))
        
        entry = {
            "id": f"conv-{i:04d}",
            "prompt": prompt,
            "response": response["response"],
            "metadata": {
                "template_id": template,
                "confidence": response["confidence"],
                "timestamp": datetime.utcnow().isoformat()
            }
        }
        print(json.dumps(entry))

if __name__ == "__main__":
    import os
    main()
```

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| `Format validation failed` | Ensure each output line is valid JSON |
| `AST similarity too high` | Restructure your code logic |
| `Execution timeout` | Reduce conversation count or optimize |
| `LLM plagiarism flagged` | Add original logic, avoid copied patterns |

---

## Need Help?

- [Executor Setup Guide](executor-setup.md) — Deploy your own evaluation node
- [Evaluation Pipeline](evaluation-pipeline.md) — Detailed scoring breakdown
- [API Reference](api-reference.md) — RPC endpoints
