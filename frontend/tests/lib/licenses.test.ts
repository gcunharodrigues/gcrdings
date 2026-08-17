import { describe, expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { buildLicenseReport } from "../../../scripts/verify-licenses.mjs";

const REPO_ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const VERIFY_BASELINE_PATH = path.join(REPO_ROOT, "scripts", "verify-baseline.sh");

describe("license and attribution baseline", () => {
  test("LICENSE.md retains the upstream MIT copyright verbatim", () => {
    const content = fs.readFileSync(path.join(REPO_ROOT, "LICENSE.md"), "utf8");
    expect(content).toContain("Copyright (c) 2024 Zackriya Solutions");
  });

  test("THIRD_PARTY_NOTICES.md exists and names the required inventories", () => {
    const noticesPath = path.join(REPO_ROOT, "THIRD_PARTY_NOTICES.md");
    expect(fs.existsSync(noticesPath)).toBe(true);

    const content = fs.readFileSync(noticesPath, "utf8");
    expect(content.toLowerCase()).toContain("httplib");
    expect(content.toLowerCase()).toContain("ffmpeg");
    expect(content.toLowerCase()).toContain("whisper");
    expect(content.toLowerCase()).toMatch(/rust crates?/);
    expect(content.toLowerCase()).toMatch(/npm packages?/);
  });

  test("THIRD_PARTY_NOTICES.md names the locked posthog-rs version", () => {
    const notices = fs.readFileSync(path.join(REPO_ROOT, "THIRD_PARTY_NOTICES.md"), "utf8");
    const cargoLock = fs.readFileSync(path.join(REPO_ROOT, "Cargo.lock"), "utf8");
    const version = cargoLock.match(/\[\[package\]\]\nname = "posthog-rs"\nversion = "([^"]+)"/)?.[1];
    expect(version).toBeTruthy();
    expect(notices).toContain(`**\`posthog-rs\`** (${version})`);
  });

  test("no path in the tree matches a Meetily-Pro namespace", () => {
    const tracked = execFileSync("git", ["ls-files"], {
      cwd: REPO_ROOT,
      encoding: "utf8",
    })
      .split("\n")
      .filter(Boolean);

    const offenders = tracked.filter((p) => /meetily.*pro|pro.*meetily/i.test(p));
    expect(offenders).toEqual([]);
  });

  test("checks resolved Rust and production npm licenses from lockfiles", () => {
    const verifier = fs.readFileSync(VERIFY_BASELINE_PATH, "utf8");

    expect(verifier).toContain("cargo metadata --locked --format-version 1");
    expect(verifier).toContain("pnpm --dir frontend install --frozen-lockfile");
    expect(verifier).toContain("pnpm --dir frontend licenses list --prod --json");
    expect(verifier).toContain("scripts/verify-licenses.mjs");
  });

  test("rejects a resolved dependency without a known license", () => {
    expect(() =>
      buildLicenseReport(
        { packages: [{ name: "bad-crate", version: "1.0.0", source: "registry", license: null }] },
        {},
        "cargo lock",
        "pnpm lock",
      ),
    ).toThrow("Missing or unknown license: bad-crate");
  });
});
