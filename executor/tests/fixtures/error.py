#!/usr/bin/env python3
"""
Harness that exits with a non-zero code.
Used for testing error handling.
"""

import sys


def main() -> None:
    """Exit with error code 1."""
    # Print some output before failing
    print("This is an error message", file=sys.stderr, flush=True)
    sys.exit(1)


if __name__ == "__main__":
    main()
