#!/usr/bin/env bash
# Deterministic half of a review loop: the reviewer (agent) judges, this node
# decides the edge. Exit 0 ONLY on an explicit approval — a missing or malformed
# verdict is a reject (safe default), never a silent pass. One script serves
# every verdict gate; it reads WHICH review to judge from its own node id.
set -euo pipefail
case "${ADW_NODE_ID:-verdict}" in
  verdict-qa)       src=qa-walk ;;
  verdict-security) src=review-security ;;
  *)                src=review-code ;;   # 'verdict' — the code review
esac
out="$ADW_RUN_DIR/$src/output.md"
[ -f "$out" ] || { echo "verdict: no reviewer output at $out" >&2; exit 1; }
last=$(grep -E '^VERDICT:' "$out" | tail -1 || true)
case "$last" in
  "VERDICT: approve"*) echo "verdict[$src]: approve"; exit 0 ;;
  *) echo "verdict[$src]: reject (${last:-no VERDICT line})" >&2; exit 1 ;;
esac
