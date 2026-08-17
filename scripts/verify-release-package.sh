#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
allowlist="$repo_root/qualification/v1/package-allowlist.json"

fail() {
    echo "verify-release-package: $1" >&2
    exit 1
}

if [[ "${1:-}" == "--self-test-manifests" ]]; then
    [[ $# -eq 1 ]] || fail "self-test takes no arguments"
    fixture=$(mktemp -d "${TMPDIR:-/tmp}/gcrdings-package-manifest-test.XXXXXX")
    trap 'rm -rf "$fixture"' EXIT INT TERM
    node --input-type=module - "$repo_root" "$allowlist" "$fixture" <<'NODE'
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
const [repoRoot, allowlistPath, fixture] = process.argv.slice(2);
const bytes = readFileSync(allowlistPath);
const allowlist = JSON.parse(bytes);
const hash = (value) => createHash("sha256").update(value).digest("hex");
const candidate = execFileSync("git", ["-C", repoRoot, "rev-parse", "HEAD"], { encoding: "utf8" }).trim();
const appRoot = path.join(fixture, "gcrdings.app");
mkdirSync(appRoot, { recursive: true, mode: 0o755 });
for (const directory of allowlist.application_directories) mkdirSync(path.join(appRoot, directory), { recursive: true, mode: 0o755 });
const entries = allowlist.application_rules.map((rule) => {
  const destination = path.join(appRoot, rule.path);
  mkdirSync(path.dirname(destination), { recursive: true, mode: 0o755 });
  if (rule.hash_policy === "source_exact") copyFileSync(path.join(repoRoot, rule.source), destination);
  else writeFileSync(destination, `fixture:${rule.path}\n`);
  chmodSync(destination, Number.parseInt(rule.mode, 8));
  const sha256 = hash(readFileSync(destination));
  return {
    path: rule.path,
    type: rule.type,
    mode: rule.mode,
    sha256,
  };
});
const directories = [...allowlist.application_directories].sort();
const dmgEntries = allowlist.dmg_entries.map((rule) => ({
  path: rule.path,
  type: rule.type,
  mode: rule.mode,
  ...(rule.type === "file" ? { sha256: "c".repeat(64) } : {}),
  ...(rule.type === "symlink" ? { target: rule.target } : {}),
}));
const manifest = {
  schema_version: 1,
  distribution_mode: "local_adhoc",
  public_notarized: "unsatisfied",
  candidate_commit: candidate,
  allowlist_sha256: hash(bytes),
  toolchain: allowlist.toolchain,
  build_inputs: allowlist.build_inputs,
  artifact_roots: { application: "gcrdings.app", dmg: "gcrdings.dmg" },
  application_entries: entries,
  application_directories: directories,
  application_tree_sha256: hash(JSON.stringify({ entries, directories })),
  dmg_sha256: hash(Buffer.from("fixture-dmg\n")),
  dmg_entries: dmgEntries,
};
writeFileSync(path.join(fixture, "gcrdings.dmg"), "fixture-dmg\n");
writeFileSync(path.join(fixture, "valid.json"), JSON.stringify(manifest));
writeFileSync(path.join(fixture, "forged.json"), JSON.stringify({ ...manifest, unexpected: true }));
const forgedEntries = manifest.application_entries.map((entry, index) => index === 0 ? { ...entry, sha256: "0".repeat(64) } : entry);
writeFileSync(path.join(fixture, "forged-hash.json"), JSON.stringify({
  ...manifest,
  application_entries: forgedEntries,
  application_tree_sha256: hash(JSON.stringify({ entries: forgedEntries, directories })),
}));
writeFileSync(path.join(fixture, "stale.json"), JSON.stringify({ ...manifest, candidate_commit: "0".repeat(40) }));
NODE
    "$0" --compare-manifests "$fixture/valid.json" "$fixture/valid.json" "$fixture/clean-build-results.json" >/dev/null
    node --input-type=module - "$fixture/clean-build-results.json" <<'NODE'
import { readFileSync } from "node:fs";
const result = JSON.parse(readFileSync(process.argv[2], "utf8"));
if (result.receipt_type !== "clean_build_results"
    || result.result !== "independently_valid"
    || result.first.manifest_sha256 !== result.second.manifest_sha256) process.exit(1);
NODE
    if "$0" --compare-manifests "$fixture/valid.json" "$fixture/forged.json" >/dev/null 2>&1; then
        fail "forged_manifest_was_accepted"
    fi
    if "$0" --compare-manifests "$fixture/forged-hash.json" "$fixture/forged-hash.json" >/dev/null 2>&1; then
        fail "forged_artifact_hash_was_accepted"
    fi
    if "$0" --compare-manifests "$fixture/stale.json" "$fixture/stale.json" >/dev/null 2>&1; then
        fail "stale_candidate_was_accepted"
    fi
    echo "verify-release-package: manifest self-test passed"
    exit 0
fi

if [[ "${1:-}" == "--compare-manifests" ]]; then
    [[ $# -ge 3 && $# -le 4 ]] || fail "usage: $0 --compare-manifests <first-manifest> <second-manifest> [results-path]"
    node --input-type=module - "$repo_root" "$allowlist" "$2" "$3" "${4:-}" <<'NODE'
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { lstatSync, readFileSync, readdirSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";

const [repoRoot, allowlistPath, firstPath, secondPath, resultsPath] = process.argv.slice(2);
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const allowlistBytes = readFileSync(allowlistPath);
const allowlist = JSON.parse(allowlistBytes);
const first = JSON.parse(readFileSync(firstPath, "utf8"));
const second = JSON.parse(readFileSync(secondPath, "utf8"));
const reject = (message) => { throw new Error(message); };
const digest = (value) => typeof value === "string" && /^[0-9a-f]{64}$/.test(value);
const exactKeys = (value, expected, label) => {
  if (!value || typeof value !== "object" || Array.isArray(value)) reject(`${label}_not_object`);
  const actual = Object.keys(value).sort();
  if (JSON.stringify(actual) !== JSON.stringify([...expected].sort())) reject(`${label}_schema_invalid`);
};

const octalMode = (stats) => (stats.mode & 0o777).toString(8).padStart(4, "0");
const fileType = (stats) => stats.isFile() ? "file" : stats.isDirectory() ? "directory" : stats.isSymbolicLink() ? "symlink" : "other";

function resolveArtifactRoot(manifestPath, value, kind) {
  if (typeof value !== "string" || value.length === 0 || path.isAbsolute(value)) reject(`manifest_${kind}_root_invalid`);
  const normalized = path.normalize(value);
  if (normalized === ".." || normalized.startsWith(`..${path.sep}`)) reject(`manifest_${kind}_root_escapes`);
  return path.resolve(path.dirname(manifestPath), normalized);
}

function validateOnDiskArtifacts(manifest, manifestPath) {
  exactKeys(manifest.artifact_roots, ["application", "dmg"], "manifest_artifact_roots");
  const appRoot = resolveArtifactRoot(manifestPath, manifest.artifact_roots.application, "application");
  const dmgRoot = resolveArtifactRoot(manifestPath, manifest.artifact_roots.dmg, "dmg");
  const appStats = lstatSync(appRoot);
  const dmgStats = lstatSync(dmgRoot);
  if (!appStats.isDirectory() || appStats.isSymbolicLink()) reject("manifest_application_root_invalid");
  if (!dmgStats.isFile() || dmgStats.isSymbolicLink()) reject("manifest_dmg_root_invalid");
  if (hash(readFileSync(dmgRoot)) !== manifest.dmg_sha256) reject("manifest_dmg_artifact_hash_mismatch");

  const actualEntries = [];
  const actualDirectories = [];
  const visit = (directory, prefix) => {
    for (const name of readdirSync(directory).sort()) {
      const absolute = path.join(directory, name);
      const relative = prefix ? `${prefix}/${name}` : name;
      const stats = lstatSync(absolute);
      const type = fileType(stats);
      if (type === "symlink" || type === "other") reject(`manifest_application_artifact_type_invalid:${relative}`);
      if (type === "directory") {
        actualDirectories.push(relative);
        visit(absolute, relative);
      } else {
        actualEntries.push({ path: relative, type, mode: octalMode(stats), sha256: hash(readFileSync(absolute)) });
      }
    }
  };
  visit(appRoot, "");
  const expectedEntries = new Map(manifest.application_entries.map((entry) => [entry.path, entry]));
  if (actualEntries.length !== expectedEntries.size) reject("manifest_application_artifacts_not_closed");
  for (const actual of actualEntries) {
    const expected = expectedEntries.get(actual.path);
    if (!expected || actual.type !== expected.type || actual.mode !== expected.mode || actual.sha256 !== expected.sha256) {
      reject(`manifest_application_artifact_mismatch:${actual.path}`);
    }
  }
  if (JSON.stringify(actualDirectories.sort()) !== JSON.stringify([...manifest.application_directories].sort())) {
    reject("manifest_application_artifact_directories_not_closed");
  }
}

function validateManifest(manifest, manifestPath) {
  exactKeys(manifest, [
    "schema_version", "distribution_mode", "public_notarized", "candidate_commit",
    "allowlist_sha256", "toolchain", "build_inputs", "artifact_roots", "application_entries",
    "application_directories", "application_tree_sha256", "dmg_sha256", "dmg_entries",
  ], "manifest");
  if (manifest.schema_version !== 1) reject("manifest_schema_invalid");
  if (manifest.distribution_mode !== "local_adhoc") reject("manifest_distribution_invalid");
  if (manifest.public_notarized !== "unsatisfied") reject("manifest_notarization_invalid");
  if (!digest(manifest.allowlist_sha256) || manifest.allowlist_sha256 !== hash(allowlistBytes)) reject("manifest_allowlist_mismatch");
  if (!digest(manifest.application_tree_sha256) || !digest(manifest.dmg_sha256)) reject("manifest_digest_invalid");
  if (JSON.stringify(manifest.toolchain) !== JSON.stringify(allowlist.toolchain)) reject("manifest_toolchain_mismatch");
  if (JSON.stringify(manifest.build_inputs) !== JSON.stringify(allowlist.build_inputs)) reject("manifest_build_inputs_mismatch");
  const head = execFileSync("git", ["-C", repoRoot, "rev-parse", "HEAD"], { encoding: "utf8" }).trim();
  if (manifest.candidate_commit !== head) reject("manifest_candidate_not_closed_head");
  if (!Array.isArray(manifest.application_entries) || !Array.isArray(manifest.application_directories)
      || !Array.isArray(manifest.dmg_entries)) reject("manifest_collections_invalid");
  const rules = new Map(allowlist.application_rules.map((rule) => [rule.path, rule]));
  const seen = new Set();
  for (const entry of manifest.application_entries) {
    const rule = rules.get(entry?.path);
    if (!rule || seen.has(entry.path)) reject("manifest_application_entries_not_closed");
    seen.add(entry.path);
    const keys = ["path", "type", "mode", "sha256"];
    exactKeys(entry, keys, `manifest_entry:${entry.path}`);
    if (entry.type !== rule.type || entry.mode !== rule.mode || !digest(entry.sha256)) reject(`manifest_entry_fact_invalid:${entry.path}`);
    if (rule.hash_policy === "source_exact" && entry.sha256 !== rule.sha256) reject(`manifest_source_hash_invalid:${entry.path}`);
  }
  if (seen.size !== allowlist.application_entries.length) reject("manifest_application_entries_not_closed");
  if (JSON.stringify([...manifest.application_directories].sort()) !== JSON.stringify([...allowlist.application_directories].sort())) reject("manifest_directories_not_closed");
  if (manifest.application_tree_sha256 !== hash(JSON.stringify({ entries: manifest.application_entries, directories: manifest.application_directories }))) reject("manifest_tree_hash_invalid");
  const dmgRules = new Map(allowlist.dmg_entries.map((rule) => [rule.path, rule]));
  const dmgSeen = new Set();
  for (const entry of manifest.dmg_entries) {
    const rule = dmgRules.get(entry?.path);
    if (!rule || dmgSeen.has(entry.path)) reject("manifest_dmg_entries_not_closed");
    dmgSeen.add(entry.path);
    const keys = ["path", "type", "mode"];
    if (rule.type === "file") keys.push("sha256");
    if (rule.type === "symlink") keys.push("target");
    exactKeys(entry, keys, `manifest_dmg_entry:${entry.path}`);
    if (entry.type !== rule.type || entry.mode !== rule.mode || entry.target !== rule.target) reject(`manifest_dmg_fact_invalid:${entry.path}`);
    if (rule.type === "file" && !digest(entry.sha256)) reject(`manifest_dmg_hash_invalid:${entry.path}`);
  }
  if (dmgSeen.size !== allowlist.dmg_entries.length) reject("manifest_dmg_entries_not_closed");
  validateOnDiskArtifacts(manifest, manifestPath);
}

validateManifest(first, firstPath);
validateManifest(second, secondPath);
if (first.candidate_commit !== second.candidate_commit) reject("candidate_commit_changed_between_builds");
if (resultsPath) {
  const result = {
    schema_version: 1,
    receipt_type: "clean_build_results",
    candidate_commit: first.candidate_commit,
    result: "independently_valid",
    first: {
      manifest_sha256: hash(readFileSync(firstPath)),
      application_tree_sha256: first.application_tree_sha256,
      dmg_sha256: first.dmg_sha256,
    },
    second: {
      manifest_sha256: hash(readFileSync(secondPath)),
      application_tree_sha256: second.application_tree_sha256,
      dmg_sha256: second.dmg_sha256,
    },
  };
  const temporary = `${resultsPath}.tmp`;
  writeFileSync(temporary, `${JSON.stringify(result, null, 2)}\n`, { mode: 0o644 });
  renameSync(temporary, resultsPath);
}
console.log("verify-release-package: fixed-input package manifests independently valid");
NODE
    exit 0
fi

[[ $# -ge 2 && $# -le 4 ]] || fail "usage: $0 <app-path> <dmg-path> [--manifest|--write-manifest <path>]"
app_path=$1
dmg_path=$2
manifest_mode=verify
manifest_path="$(dirname "$(dirname "$app_path")")/release-package-manifest.json"
if [[ $# -gt 2 ]]; then
    [[ $# -eq 4 && ( "$3" == "--manifest" || "$3" == "--write-manifest" ) ]] ||
        fail "usage: $0 <app-path> <dmg-path> [--manifest|--write-manifest <path>]"
    [[ "$3" == "--write-manifest" ]] && manifest_mode=write
    manifest_path=$4
fi

[[ -d "$app_path" && ! -L "$app_path" ]] || fail "application_missing_or_aliased"
[[ -f "$dmg_path" && ! -L "$dmg_path" ]] || fail "installer_missing_or_aliased"
[[ -f "$allowlist" ]] || fail "package_allowlist_missing"
/usr/bin/codesign --verify --deep --strict "$app_path" >/dev/null 2>&1 || fail "application_signature_invalid"
/usr/bin/hdiutil verify "$dmg_path" >/dev/null || fail "installer_checksum_invalid"
for executable in gcrdings foundation-helper ffmpeg; do
    /usr/bin/otool -l "$app_path/Contents/MacOS/$executable" 2>/dev/null | /usr/bin/grep 'cmd LC_UUID' >/dev/null ||
        fail "mach_o_lc_uuid_missing"
done

temporary_root=$(mktemp -d "${TMPDIR:-/tmp}/gcrdings-package-verify.XXXXXX")
mount_root="$temporary_root/mount"
mkdir -m 0700 "$mount_root"
mounted=0
cleanup() {
    if [[ $mounted -eq 1 ]]; then
        /usr/bin/hdiutil detach "$mount_root" >/dev/null 2>&1 || true
    fi
    rm -rf "$temporary_root"
}
trap cleanup EXIT INT TERM

/usr/bin/hdiutil attach -readonly -nobrowse -mountpoint "$mount_root" "$dmg_path" >/dev/null
mounted=1

node --input-type=module - \
    "$repo_root" "$allowlist" "$app_path" "$dmg_path" "$mount_root" \
    "$manifest_mode" "$manifest_path" "$temporary_root" <<'NODE'
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  readlinkSync,
  renameSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

const [repoRoot, allowlistPath, appPath, dmgPath, mountRoot, manifestMode, manifestPath, temporaryRoot]
  = process.argv.slice(2);
const allowlistBytes = readFileSync(allowlistPath);
const allowlist = JSON.parse(allowlistBytes);
const sha256 = (value) => createHash("sha256").update(value).digest("hex");
const hashFile = (file) => sha256(readFileSync(file));
const reject = (message) => { throw new Error(message); };
const octalMode = (stats) => (stats.mode & 0o777).toString(8).padStart(4, "0");
const normalizedKey = (value) => value.normalize("NFC").toLocaleLowerCase("en-US");

function command(program, args) {
  return execFileSync(program, args, { encoding: "utf8" }).trim();
}

function assertClosedNames(values, label) {
  const seen = new Set();
  for (const value of values) {
    const key = normalizedKey(value);
    if (seen.has(key)) reject(`${label}_duplicate:${value}`);
    seen.add(key);
  }
}

function fileType(stats) {
  if (stats.isFile()) return "file";
  if (stats.isDirectory()) return "directory";
  if (stats.isSymbolicLink()) return "symlink";
  return "other";
}

const ruleByPath = new Map(allowlist.application_rules.map((rule) => [rule.path, rule]));

function collectApplication(root) {
  const entries = [];
  const directories = [];
  const visit = (directory, prefix) => {
    for (const name of readdirSync(directory).sort()) {
      const absolute = path.join(directory, name);
      const relative = prefix ? `${prefix}/${name}` : name;
      const stats = lstatSync(absolute);
      const type = fileType(stats);
      if (type === "symlink") reject(`application_symlink_forbidden:${relative}`);
      if (type === "directory") {
        directories.push(relative);
        visit(absolute, relative);
        continue;
      }
      if (type !== "file") reject(`application_member_type_forbidden:${relative}`);
      for (const suffix of allowlist.forbidden_suffixes) {
        if (relative.endsWith(suffix)) reject(`forbidden_package_suffix:${relative}`);
      }
      const rule = ruleByPath.get(relative);
      const entry = { path: relative, type, mode: octalMode(stats), sha256: hashFile(absolute) };
      const bytes = readFileSync(absolute);
      for (const marker of allowlist.forbidden_content_markers) {
        if (bytes.includes(Buffer.from(marker))) reject(`forbidden_content_marker:${relative}`);
      }
      entries.push(entry);
    }
  };
  visit(root, "");
  assertClosedNames(entries.map((entry) => entry.path), "application");
  assertClosedNames(directories, "application_directory");
  return { entries, directories };
}

function validateApplication(root) {
  const facts = collectApplication(root);
  const actualPaths = facts.entries.map((entry) => entry.path).sort();
  const expectedPaths = [...allowlist.application_entries].sort();
  if (JSON.stringify(actualPaths) !== JSON.stringify(expectedPaths)) reject("application_membership_mismatch");
  if (JSON.stringify(facts.directories.sort()) !== JSON.stringify([...allowlist.application_directories].sort())) {
    reject("application_directories_mismatch");
  }
  if (ruleByPath.size !== expectedPaths.length) reject("application_rules_not_closed");
  for (const entry of facts.entries) {
    const rule = ruleByPath.get(entry.path);
    if (!rule || entry.type !== rule.type || entry.mode !== rule.mode) reject(`application_rule_mismatch:${entry.path}`);
    if (rule.hash_policy === "source_exact") {
      const source = path.resolve(repoRoot, rule.source);
      if (!source.startsWith(`${path.resolve(repoRoot)}${path.sep}`)) reject(`source_path_outside_repository:${entry.path}`);
      const sourceHash = hashFile(source);
      if (sourceHash !== rule.sha256 || entry.sha256 !== rule.sha256) reject(`source_hash_mismatch:${entry.path}`);
    } else if (rule.hash_policy !== "build_manifest_exact") {
      reject(`unknown_hash_policy:${entry.path}`);
    }
  }
  return facts;
}

function collectDmgRoot() {
  const entries = [];
  for (const name of readdirSync(mountRoot).sort()) {
    const absolute = path.join(mountRoot, name);
    const stats = lstatSync(absolute);
    const type = fileType(stats);
    const entry = { path: name, type, mode: octalMode(stats) };
    if (type === "file") entry.sha256 = hashFile(absolute);
    if (type === "symlink") entry.target = readlinkSync(absolute);
    entries.push(entry);
  }
  assertClosedNames(entries.map((entry) => entry.path), "dmg");
  const rules = new Map(allowlist.dmg_entries.map((rule) => [rule.path, rule]));
  if (entries.length !== rules.size) reject("dmg_membership_mismatch");
  for (const entry of entries) {
    const rule = rules.get(entry.path);
    if (!rule || entry.type !== rule.type || entry.mode !== rule.mode || entry.target !== rule.target) {
      reject(`dmg_rule_mismatch:${entry.path}`);
    }
  }
  return entries;
}

if (allowlist.schema_version !== 1 || allowlist.distribution_mode !== "local_adhoc"
    || allowlist.public_notarized !== "unsatisfied") reject("allowlist_identity_invalid");
const xcode = command("/usr/bin/xcodebuild", ["-version"]).split("\n");
const actualToolchain = {
  rust: command("rustc", ["--version"]).split(/\s+/)[1],
  node: process.version,
  pnpm: command("pnpm", ["--version"]),
  swift: command("swiftc", ["--version"]).split("\n")[0]
    .replace(/^Apple Swift version /, ""),
  xcode: `${xcode[0].replace(/^Xcode /, "")} (${xcode[1].replace(/^Build version /, "")})`,
  macos_sdk: command("/usr/bin/xcrun", ["--sdk", "macosx", "--show-sdk-version"]),
  builder_macos: command("/usr/bin/sw_vers", ["-productVersion"]),
};
if (JSON.stringify(actualToolchain) !== JSON.stringify(allowlist.toolchain)) reject("release_toolchain_mismatch");
assertClosedNames(allowlist.application_entries, "allowlist_application");
assertClosedNames(allowlist.application_directories, "allowlist_directory");
assertClosedNames(allowlist.dmg_entries.map((entry) => entry.path), "allowlist_dmg");

const application = validateApplication(appPath);
const mountedAppPath = path.join(mountRoot, "gcrdings.app");
const mountedApplication = validateApplication(mountedAppPath);
if (JSON.stringify(application) !== JSON.stringify(mountedApplication)) reject("dmg_application_mismatch");
const dmgEntries = collectDmgRoot();

let reference = null;
if (manifestMode === "verify") reference = JSON.parse(readFileSync(manifestPath, "utf8"));
const candidate = execFileSync("git", ["-C", repoRoot, "rev-parse", "HEAD"], { encoding: "utf8" }).trim();
const requestedCandidate = process.env.GCRDINGS_RELEASE_CANDIDATE || reference?.candidate_commit || candidate;
if (!/^[0-9a-f]{40}$/.test(requestedCandidate)) reject("candidate_commit_invalid");
if (requestedCandidate !== candidate) reject("candidate_commit_not_closed_head");
const mainExecutable = path.join(appPath, "Contents/MacOS/gcrdings");
if (!readFileSync(mainExecutable).includes(Buffer.from(`gcrdings-build-commit:${candidate}`))) {
  reject("candidate_marker_mismatch");
}

const manifestDirectory = path.dirname(path.resolve(manifestPath));
const artifactRoot = (artifact, label) => {
  const relative = path.relative(manifestDirectory, path.resolve(artifact));
  if (!relative || path.isAbsolute(relative) || relative === ".." || relative.startsWith(`..${path.sep}`)) {
    reject(`${label}_artifact_root_outside_manifest_tree`);
  }
  return relative.split(path.sep).join("/");
};

const manifest = {
  schema_version: 1,
  distribution_mode: "local_adhoc",
  public_notarized: "unsatisfied",
  candidate_commit: candidate,
  allowlist_sha256: sha256(allowlistBytes),
  toolchain: actualToolchain,
  build_inputs: allowlist.build_inputs,
  artifact_roots: {
    application: artifactRoot(appPath, "application"),
    dmg: artifactRoot(dmgPath, "dmg"),
  },
  application_entries: application.entries,
  application_directories: application.directories.sort(),
  application_tree_sha256: sha256(JSON.stringify(application)),
  dmg_sha256: hashFile(dmgPath),
  dmg_entries: dmgEntries,
};

if (manifestMode === "write") {
  mkdirSync(path.dirname(manifestPath), { recursive: true, mode: 0o755 });
  const temporary = `${manifestPath}.tmp`;
  writeFileSync(temporary, `${JSON.stringify(manifest, null, 2)}\n`, { mode: 0o644 });
  renameSync(temporary, manifestPath);
} else if (JSON.stringify(reference) !== JSON.stringify(manifest)) {
  reject("release_manifest_mismatch");
}
console.log("verify-release-package: package valid");
NODE
