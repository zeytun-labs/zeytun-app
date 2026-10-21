#!/usr/bin/env python3
"""Require successful checks and accept only skips explicitly selected by the plan."""
from __future__ import annotations

import json
import os


def validate(needs: dict) -> None:
    if needs["checks"]["result"] != "success":
        raise ValueError("Shared checks did not succeed")
    for job in ("frontend", "native"):
        selected = needs["checks"]["outputs"][job]
        if selected not in ("true", "false"):
            raise ValueError(f"Missing or invalid plan for {job}")
        expected = "success" if selected == "true" else "skipped"
        actual = needs[job]["result"]
        if actual != expected:
            raise ValueError(f"{job}: expected {expected}, got {actual}")


if __name__ == "__main__":
    validate(json.loads(os.environ["CI_NEEDS"]))
    print("All selected checks passed; unselected jobs were intentionally skipped.")
