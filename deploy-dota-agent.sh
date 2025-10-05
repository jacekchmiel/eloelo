#!/bin/bash
set -e

PROJECT_ROOT=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

cd "$PROJECT_ROOT"/dota-agent || exit 1

TARGET_FILE="$PROJECT_ROOT"/ui/dist/dota-agent.zip

rm -f "$TARGET_FILE"
zip "$TARGET_FILE" * .python-version