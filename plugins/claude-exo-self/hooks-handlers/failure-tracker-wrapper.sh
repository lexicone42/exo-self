#!/usr/bin/env bash
source "$(dirname "$0")/env.sh"
exec uv run python "$(dirname "$0")/failure-tracker.py"
