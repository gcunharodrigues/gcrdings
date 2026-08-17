import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const BASELINE_PATH = path.join(REPO_ROOT, "docs", "BASELINE.md");
const VERIFY_BASELINE_PATH = path.join(REPO_ROOT, "scripts", "verify-baseline.sh");
const APPLIED_VERIFIABLE_RECORD_MIGRATION = path.join(
  REPO_ROOT,
  "frontend",
  "src-tauri",
  "migrations",
  "20260816000000_add_verifiable_record.sql"
);
const GAUNTLET_PATH = path.join(
  REPO_ROOT,
  "adws",
  "feature",
  "gates",
  "gauntlet.sh"
);

function gauntletCommandLines(): string[] {
  const content = fs.readFileSync(GAUNTLET_PATH, "utf8");
  return content
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .filter((line) => !line.startsWith("#"))
    .filter((line) => line !== "set -e");
}

describe("docs/BASELINE.md", () => {
  test("exists", () => {
    expect(fs.existsSync(BASELINE_PATH)).toBe(true);
  });

  test("lists every command line present in the gauntlet, parsed from the gate file itself", () => {
    const baseline = fs.readFileSync(BASELINE_PATH, "utf8");
    const gauntletLines = gauntletCommandLines();

    // The gauntlet must actually have command lines to compare against —
    // otherwise this test would vacuously pass if the gate file went empty.
    expect(gauntletLines.length).toBeGreaterThan(0);

    for (const line of gauntletLines) {
      expect(baseline).toContain(line);
    }
  });

  test("records a packaging command and a release command, each tagged verified or declared-not-run", () => {
    const baseline = fs.readFileSync(BASELINE_PATH, "utf8");

    expect(baseline).toMatch(/tauri:build/);
    expect(baseline.toLowerCase()).toMatch(/release/);

    const taggedPattern = /verified|declared, not run/i;
    expect(taggedPattern.test(baseline)).toBe(true);
  });

  test("pins the Rust, Node, and pnpm toolchains used by CI", () => {
    const rustToolchainPath = path.join(REPO_ROOT, "rust-toolchain.toml");
    const nodeVersionPath = path.join(REPO_ROOT, ".node-version");
    const packageJsonPath = path.join(REPO_ROOT, "frontend", "package.json");
    const ciPath = path.join(REPO_ROOT, ".github", "workflows", "ci.yml");

    expect(fs.existsSync(rustToolchainPath)).toBe(true);
    expect(fs.existsSync(nodeVersionPath)).toBe(true);

    const rustToolchain = fs.readFileSync(rustToolchainPath, "utf8");
    const nodeVersion = fs.readFileSync(nodeVersionPath, "utf8").trim();
    const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));
    const ci = fs.readFileSync(ciPath, "utf8");

    const rustChannel = rustToolchain.match(/channel = "([0-9]+\.[0-9]+\.[0-9]+)"/)?.[1];
    const pnpmVersion = packageJson.packageManager?.match(/^pnpm@(.+)$/)?.[1];

    expect(rustChannel).toMatch(/^[0-9]+\.[0-9]+\.[0-9]+$/);
    expect(nodeVersion).toMatch(/^[0-9]+\.[0-9]+\.[0-9]+$/);
    expect(pnpmVersion).toMatch(/^[0-9]+\.[0-9]+\.[0-9]+$/);
    expect(ci).toMatch(new RegExp(`dtolnay/rust-toolchain@[0-9a-f]{40} # ${rustChannel}`));
    expect(ci).toContain("node-version-file: .node-version");
    expect(ci).toContain(`version: ${pnpmVersion}`);
  });

  test("provides one executable dependency and license verifier", () => {
    expect(fs.existsSync(VERIFY_BASELINE_PATH)).toBe(true);

    const verifier = fs.readFileSync(VERIFY_BASELINE_PATH, "utf8");
    expect(verifier).toContain("bun test frontend/tests/lib/baseline.test.ts");
    expect(verifier).toContain("cargo metadata --locked --format-version 1");
    expect(verifier).toContain("pnpm --dir frontend licenses list --prod --json");
  });

  test("preserves the immutable bytes of the applied verifiable record migration", () => {
    const checksum = createHash("sha384")
      .update(fs.readFileSync(APPLIED_VERIFIABLE_RECORD_MIGRATION))
      .digest("hex");

    expect(checksum).toBe(
      "eb27753aec1a0af35df5b5efd374e41e3eafbe1624f57849bf0868429c97b679efffcc7599a3e1769a9461efe193c03c"
    );
  });
});
