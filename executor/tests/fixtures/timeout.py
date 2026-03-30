#!/usr/bin/env python3
"""
Harness that runs forever.
Used for testing timeout enforcement.
"""

import time


def main() -> None:
    """Run in an infinite loop to trigger timeout."""
    # Print something so we know it started
    print('{"status": "started"}', flush=True)

    # Infinite loop - will be killed by timeout
    while True:
        time.sleep(1)


if __name__ == "__main__":
    main()
