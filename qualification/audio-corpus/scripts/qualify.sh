#!/bin/sh
set -eu
umask 077

usage() {
  echo "Usage: $0 --observations FILE --output FILE --corpus-dir DIRECTORY [--baseline RECEIPT]"
}

observations=""
output=""
corpus_dir=""
baseline=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --observations|--output|--corpus-dir|--baseline)
      [ "$#" -ge 2 ] || { usage >&2; exit 2; }
      case "$1" in
        --observations) observations=$2 ;;
        --output) output=$2 ;;
        --corpus-dir) corpus_dir=$2 ;;
        --baseline) baseline=$2 ;;
      esac
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
done

[ -f "$observations" ] || { echo "Observation file is required" >&2; exit 2; }
[ -n "$output" ] || { usage >&2; exit 2; }
[ -n "$corpus_dir" ] || { usage >&2; exit 2; }
[ -z "$baseline" ] || [ -f "$baseline" ] || { echo "Baseline receipt is not readable" >&2; exit 2; }
command -v bun >/dev/null 2>&1 || { echo "Qualification requires Bun" >&2; exit 1; }

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
manifest="$script_dir/../manifest.json"
generated="$corpus_dir/generated-manifest.json"

"$script_dir/generate.sh" --output "$corpus_dir"
if [ -n "$baseline" ]; then
  bun "$script_dir/measure.ts" \
    --manifest "$manifest" \
    --generated "$generated" \
    --observations "$observations" \
    --baseline "$baseline" \
    --output "$output"
else
  bun "$script_dir/measure.ts" \
    --manifest "$manifest" \
    --generated "$generated" \
    --observations "$observations" \
    --output "$output"
fi
bun "$script_dir/../../validate-receipt.ts" "$output"
bun -e '
  const receipt = await Bun.file(process.argv[1]).json();
  if (receipt.outcome === "failed") process.exit(1);
' "$output"
