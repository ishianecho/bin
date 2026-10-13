#!/usr/bin/env bash

set -euo pipefail

usage() {
  echo "usage: completion.sh <zsh|bash> <ghf|proxy|xrayctl>" >&2
}

shell="${1:-}"
tool="${2:-}"

case "$shell" in
  zsh | bash) ;;
  *)
    usage
    exit 2
    ;;
esac

case "$tool" in
  ghf | proxy | xrayctl) ;;
  *)
    usage
    exit 2
    ;;
esac

tool_path="$(dirname "$(realpath "$0")")/$tool"
[ -x "$tool_path" ] || tool_path="$(command -v "$tool" || true)"
[ -n "$tool_path" ] || {
  echo "completion.sh: $tool was not found" >&2
  exit 1
}

exec "$tool_path" completion "$shell"
