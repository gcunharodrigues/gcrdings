#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
audit_policy="$repo_root/qualification/v1/cargo-audit-reviewed-warnings.json"
audit_report=$(mktemp "${TMPDIR:-/tmp}/gcrdings-cargo-audit.XXXXXX")
active_packages=$(mktemp "${TMPDIR:-/tmp}/gcrdings-cargo-tree.XXXXXX")
clippy_probe=$(mktemp -d "${TMPDIR:-/tmp}/gcrdings-clippy-probe.XXXXXX")

cleanup() {
    rm -f "$audit_report" "$active_packages"
    rm -f "$clippy_probe/Cargo.toml" "$clippy_probe/Cargo.lock" "$clippy_probe/output" "$clippy_probe/src/lib.rs"
    rm -rf "$clippy_probe/target"
    rmdir "$clippy_probe/src" "$clippy_probe" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

cd "$repo_root"

collect_cargo_audit() {
    audit_status=0
    cargo audit --target-os macos --target-arch aarch64 --json >"$audit_report" || audit_status=$?
    cargo tree --workspace --target aarch64-apple-darwin --locked --prefix none --format '{p}' >"$active_packages"
}

validate_cargo_audit() {
    local probe_mode=${1:-normal}
    node --input-type=module - "$audit_report" "$active_packages" "$audit_status" "$audit_policy" \
        "$(/bin/date -u +%F)" "$probe_mode" <<'NODE'
import { readFileSync } from "node:fs";

const [reportPath, activePath, rawStatus, policyPath, today, probeMode] = process.argv.slice(2);
const reject = (message) => { throw new Error(message); };
const exactKeys = (value, expected, label) => {
  if (!value || typeof value !== "object" || Array.isArray(value)) reject(`${label}_not_object`);
  if (JSON.stringify(Object.keys(value).sort()) !== JSON.stringify([...expected].sort())) {
    reject(`${label}_schema_invalid`);
  }
};
const report = JSON.parse(readFileSync(reportPath, "utf8"));
const status = Number(rawStatus);
const policy = JSON.parse(readFileSync(policyPath, "utf8"));
exactKeys(policy, ["schema_version", "owner", "reviewed_on", "expires_on", "reviews"], "policy");
if (policy.schema_version !== 1 || policy.owner !== "human:guilherme"
    || !/^\d{4}-\d{2}-\d{2}$/.test(policy.reviewed_on)
    || !/^\d{4}-\d{2}-\d{2}$/.test(policy.expires_on)
    || policy.reviewed_on > today || policy.expires_on < today || !Array.isArray(policy.reviews)) {
  reject("policy_identity_or_expiry_invalid");
}
if (probeMode === "empty_reviews") policy.reviews = [];

const active = new Set(
  readFileSync(activePath, "utf8")
    .split("\n")
    .map((line) => line.match(/^(\S+) v([^\s]+)/))
    .filter(Boolean)
    .map((match) => `${match[1]}@${match[2]}`),
);
const vulnerabilities = report.vulnerabilities?.list ?? [];
const reachableVulnerabilities = vulnerabilities.filter(({ package: dependency }) =>
  active.has(`${dependency.name}@${dependency.version}`),
);
if (reachableVulnerabilities.length > 0) reject("reachable_vulnerability");

const warningKey = (kind, advisoryId, packageName, version) =>
  `${kind}:${advisoryId ?? "none"}:${packageName}@${version}`;
const reachableWarnings = [];
for (const [kind, findings] of Object.entries(report.warnings ?? {})) {
  if (!Array.isArray(findings)) reject("warning_report_invalid");
  for (const finding of findings) {
    const dependency = finding.package;
    if (active.has(`${dependency.name}@${dependency.version}`)) {
      reachableWarnings.push(warningKey(kind, finding.advisory?.id ?? null, dependency.name, dependency.version));
    }
  }
}
const reviewedWarnings = policy.reviews.map((review, index) => {
  exactKeys(review, ["kind", "advisory_id", "package", "version", "rationale"], `review:${index}`);
  if (!review.kind || !review.package || !review.version || typeof review.rationale !== "string"
      || review.rationale.length < 40 || (review.advisory_id !== null && typeof review.advisory_id !== "string")) {
    reject(`review_invalid:${index}`);
  }
  return warningKey(review.kind, review.advisory_id, review.package, review.version);
});
const uniqueReviews = new Set(reviewedWarnings);
if (uniqueReviews.size !== reviewedWarnings.length) reject("duplicate_review");
if (JSON.stringify([...reachableWarnings].sort()) !== JSON.stringify([...uniqueReviews].sort())) {
  reject("reachable_warning_review_mismatch");
}
if (status !== 0 && vulnerabilities.length === 0 && reachableWarnings.length === 0) {
  reject(`cargo_audit_failed_without_findings:${status}`);
}
console.log(`cargo audit: zero reachable vulnerabilities; ${reachableWarnings.length} exact reachable warnings reviewed until ${policy.expires_on}`);
NODE
}

if [[ "${1:-}" == "--self-test-cargo-audit-policy" ]]; then
    [[ $# -eq 1 ]] || { echo "verify-release-gates: audit policy self-test takes no arguments" >&2; exit 1; }
    collect_cargo_audit
    validate_cargo_audit
    if validate_cargo_audit empty_reviews >/dev/null 2>&1; then
        echo "verify-release-gates: empty audit policy accepted reachable warnings" >&2
        exit 1
    fi
    echo "verify-release-gates: cargo audit policy blocks unreviewed reachable warnings"
    exit 0
fi

if [[ "${1:-}" == "--self-test-clippy-baseline" ]]; then
    mkdir "$clippy_probe/src"
    printf '%s\n' '[package]' 'name = "clippy-structural-probe"' 'version = "0.0.0"' 'edition = "2021"' > "$clippy_probe/Cargo.toml"
    printf '%s\n' \
        'pub fn new_structural_debt(a: u8, b: u8, c: u8, d: u8, e: u8, f: u8, g: u8, h: u8) -> u8 {' \
        '    a + b + c + d + e + f + g + h' \
        '}' > "$clippy_probe/src/lib.rs"
    if cargo clippy --manifest-path "$clippy_probe/Cargo.toml" --offline -- -D warnings >"$clippy_probe/output" 2>&1; then
        echo "verify-release-gates: site-scoped policy accepted new structural debt" >&2
        exit 1
    fi
    grep -q "too many arguments" "$clippy_probe/output" || {
        echo "verify-release-gates: structural probe failed for an unexpected reason" >&2
        exit 1
    }
    echo "verify-release-gates: site-scoped Clippy policy rejects new structural occurrences"
    exit 0
fi

# Reviewed legacy structural debt is allowed only at the documented source sites.
# All new structural, correctness, and ordinary warning occurrences remain denied.
if [[ "${1:-}" == "--self-test-clippy-policy" ]]; then
    mkdir "$clippy_probe/src"
    printf '%s\n' '[package]' 'name = "clippy-policy-probe"' 'version = "0.0.0"' 'edition = "2021"' > "$clippy_probe/Cargo.toml"
    printf '%s\n' 'pub fn rejected_warning() {' '    let deliberately_unused = 1;' '}' > "$clippy_probe/src/lib.rs"
    if cargo clippy --manifest-path "$clippy_probe/Cargo.toml" --offline -- \
        -D warnings >"$clippy_probe/output" 2>&1; then
        echo "verify-release-gates: Clippy warning policy accepted an ordinary warning" >&2
        exit 1
    fi
    grep -q "unused variable" "$clippy_probe/output" || {
        echo "verify-release-gates: Clippy warning probe failed for an unexpected reason" >&2
        exit 1
    }
    rm -f "$clippy_probe/output" "$clippy_probe/Cargo.lock"
    echo "verify-release-gates: Clippy warning policy rejects ordinary warnings"
    exit 0
fi

cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
swift test --package-path foundation-helper
cargo test --workspace --locked
pnpm --dir frontend lint
pnpm --dir frontend exec tsc --noEmit
pnpm --dir frontend build
bun test --timeout=15000 --max-concurrency=4 frontend/tests/lib

collect_cargo_audit
validate_cargo_audit

pnpm --dir frontend audit --audit-level high

if [[ -n "${GCRDINGS_RELEASE_APP:-}" || -n "${GCRDINGS_RELEASE_DMG:-}" ]]; then
    [[ -n "${GCRDINGS_RELEASE_APP:-}" && -n "${GCRDINGS_RELEASE_DMG:-}" ]] || {
        echo "verify-release-gates: both release artifact paths are required" >&2
        exit 1
    }
    scripts/verify-release-package.sh "$GCRDINGS_RELEASE_APP" "$GCRDINGS_RELEASE_DMG"
elif [[ "${GCRDINGS_REQUIRE_RELEASE_PACKAGE:-0}" == "1" ]]; then
    echo "verify-release-gates: exact release artifacts are required" >&2
    exit 1
fi

echo "verify-release-gates: strict gates passed"
