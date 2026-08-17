#!/usr/bin/env bash
# Land the approved work: one atomic commit + ticket closed. This node only
# runs after the human review gate approved — the commit IS the approval.
set -e
TID=$(python3 -c "import json,os; print(json.load(open(os.path.join(os.environ['ADW_RUN_DIR'],'claim','output.md')))['id'])")
git add -A
BASELINE="$ADW_RUN_DIR/land-baseline"
if [ -s "$BASELINE" ]; then
  # Files already dirty before this run started must not be swept into the
  # ticket's atomic commit. Unstage every baseline path; a path that is both
  # baseline-dirty and build-changed is conservatively left for the human.
  # -0/-- : paths are NUL-separated and never shell-split, and '--' stops a
  #         file named like an option (e.g. '-x') being read as one.
  xargs -0 -r git restore --staged -- < "$BASELINE"
  echo "land: held back pre-existing paths: $(tr '\0' ' ' < "$BASELINE")" >&2
fi
git commit -m "adw(chore): land ticket $TID"
python3 ~/.agents/adw/board.py complete "$TID"
echo "landed $TID"

# Optional per-repo post-land hook (INSTANCE-owned — the template never ships
# one, so `instantiate.py --update` never overwrites it). Runs after the commit
# + ticket-close with $1=<ticket-id>, cwd=repo root. Guarded + non-fatal: a hook
# failure must never fail an already-committed land.
[ -x adws/post-land.sh ] && adws/post-land.sh "$TID" || true
