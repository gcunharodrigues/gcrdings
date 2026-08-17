import { describe, expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");

// Paths where a match is expected and accepted: they exist specifically to
// record the upstream project's name (attribution, provenance, pinned release
// input, license text, or a negative package scan marker) rather than to brand
// gcrdings itself. Exact paths only, plus one glob for the ADR series the plan
// explicitly names.
const CONTENT_ALLOWLIST = new Set([
  "LICENSE.md",
  "SOURCE_OFFER.md",
  "THIRD_PARTY_NOTICES.md",
  "PROVENANCE.md",
  "CONTEXT.md",
  "docs/BASELINE.md",
  ".github/workflows/ci.yml",
  ".github/workflows/release-gates.yml",
  "CONTRIBUTING.md",
  "README.md",
  "frontend/tests/lib/branding.test.ts",
  "frontend/tests/lib/licenses.test.ts",
  "frontend/tests/lib/provenance.test.ts",
  "qualification/v1/package-allowlist.json",
  "specs/002-complete-v1/spec.md",
  "specs/002-complete-v1/tasks.md",
  "specs/003-public-github/clarify.md",
  "specs/003-public-github/spec.md",
  "specs/003-public-github/tasks.md",
]);
const CONTENT_ALLOWLIST_PREFIX = "docs/adr/";

// Files that still carry cosmetic or functional "Meetily"/"Zackriya" text
// deliberately deferred out of this ticket's scope (recorded in
// docs/BASELINE.md under "Deferred rebrand surfaces"). An exact list, not a
// directory glob, so a new file added anywhere under these directories is
// still caught by the scan below.
const DEFERRED_EXACT_PATHS = new Set([
  "backend/build-docker.ps1",
  "backend/build-docker.sh",
  "backend/docker-compose.yml",
  "backend/install_dependancies_for_windows.ps1",
  "backend/run-docker.ps1",
  "backend/run-docker.sh",
  "backend/setup-db.ps1",
  "backend/setup-db.sh",
  "backend/start_with_output.ps1",
  "frontend/build-gpu.bat",
  "frontend/build-gpu.ps1",
  "frontend/build.bat",
  "frontend/build.ps1",
  "frontend/build_backup.bat",
  "frontend/dev-gpu.bat",
  "frontend/dev-gpu.ps1",
  "frontend/src-tauri/build/ffmpeg.rs",
  "frontend/src-tauri/src/parakeet_engine/parakeet_engine.rs",
]);

const BINARY_EXTENSIONS = new Set([
  ".png",
  ".gif",
  ".jpg",
  ".jpeg",
  ".ico",
  ".icns",
]);

const FORBIDDEN_TOKENS = [
  "meetily",
  "zackriya",
  "meetily.ai",
  "meeting-minutes",
  "launch20",
  "discord.gg/crrymmqbfh",
  // AC7: PostHog credential prefix, the removed minisign pubkey, and the
  // removed updater manifest URL shape.
  "phc_",
  "dw50cnvzdgvkignvbw1lbnq6ig1pbmlzawdu",
  "releases/latest/download/latest.json",
];

function listTrackedFiles(): string[] {
  const output = execFileSync("git", ["ls-files"], {
    cwd: REPO_ROOT,
    encoding: "utf8",
  });
  return output.split("\n").filter(Boolean);
}

function isAllowlisted(relPath: string): boolean {
  return (
    CONTENT_ALLOWLIST.has(relPath) || relPath.startsWith(CONTENT_ALLOWLIST_PREFIX)
  );
}

function isDeferred(relPath: string): boolean {
  return DEFERRED_EXACT_PATHS.has(relPath);
}

function isBinary(relPath: string): boolean {
  return BINARY_EXTENSIONS.has(path.extname(relPath).toLowerCase());
}

describe("branding: no Meetily/Zackriya surface outside the documented exceptions", () => {
  test("tracked source and docs contain zero forbidden-token matches", () => {
    const offenders: string[] = [];

    for (const relPath of listTrackedFiles()) {
      if (isAllowlisted(relPath) || isDeferred(relPath) || isBinary(relPath)) {
        continue;
      }

      const absPath = path.join(REPO_ROOT, relPath);
      let content: string;
      try {
        content = fs.readFileSync(absPath, "utf8");
      } catch {
        // Deleted-but-still-listed (e.g. mid-rebase) or a directory entry — skip.
        continue;
      }

      const lines = content.split("\n");
      lines.forEach((line, idx) => {
        const lower = line.toLowerCase();
        for (const token of FORBIDDEN_TOKENS) {
          if (lower.includes(token)) {
            offenders.push(`${relPath}:${idx + 1}: matched "${token}"`);
          }
        }
      });
    }

    expect(offenders).toEqual([]);
  });

  test("every entry in the deferred list is still tracked (no stale exclusions)", () => {
    const tracked = new Set(listTrackedFiles());
    const stale = [...DEFERRED_EXACT_PATHS].filter((p) => !tracked.has(p));
    expect(stale).toEqual([]);
  });

  test("tauri.conf.json ships no updater endpoint or signing key", () => {
    const confPath = path.join(
      REPO_ROOT,
      "frontend/src-tauri/tauri.conf.json"
    );
    const conf = JSON.parse(fs.readFileSync(confPath, "utf8"));

    expect(conf.plugins?.updater?.endpoints).toBeUndefined();
    expect(conf.plugins?.updater?.pubkey).toBeUndefined();
    expect(conf.bundle?.createUpdaterArtifacts).toBe(false);

    const permissions = conf.app?.security?.capabilities?.flatMap(
      (capability: { permissions?: string[] }) => capability.permissions ?? [],
    ) ?? [];
    expect(permissions).not.toContain("updater:default");
    expect(permissions).not.toContain("process:default");
    expect(permissions).not.toContain("fs:read-all");
    expect(permissions).not.toContain("fs:write-all");

    const packageJson = fs.readFileSync(
      path.join(REPO_ROOT, "frontend", "package.json"),
      "utf8",
    );
    const cargoToml = fs.readFileSync(
      path.join(REPO_ROOT, "frontend", "src-tauri", "Cargo.toml"),
      "utf8",
    );
    expect(packageJson).not.toContain("@tauri-apps/plugin-updater");
    expect(packageJson).not.toContain("@tauri-apps/plugin-process");
    expect(cargoToml).not.toContain("tauri-plugin-updater");
    expect(cargoToml).not.toContain("tauri-plugin-process");
  });
});
