#!/usr/bin/env python3
"""
Simple harness that generates valid JSON-L output.
Used for testing successful harness execution.
"""

import json
import sys


def main() -> None:
    """Generate 3 valid conversation entries in JSON-L format."""
    conversations = [
        {
            "messages": [
                {"role": "user", "content": "Hello, how are you?"},
                {
                    "role": "assistant",
                    "content": "I'm doing well, thank you for asking!",
                },
            ]
        },
        {
            "messages": [
                {"role": "user", "content": "What is Python?"},
                {
                    "role": "assistant",
                    "content": "Python is a high-level programming language.",
                },
            ]
        },
        {
            "messages": [
                {"role": "user", "content": "Goodbye!"},
                {"role": "assistant", "content": "See you later! Have a great day!"},
            ]
        },
    ]

    for conv in conversations:
        print(json.dumps(conv), flush=True)

    sys.exit(0)


if __name__ == "__main__":
    main()
