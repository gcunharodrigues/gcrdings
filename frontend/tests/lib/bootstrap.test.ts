import { describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const SCRIPT_PATH = path.join(REPO_ROOT, "scripts", "bootstrap-dev.sh");
const HELPER_SCRIPT_PATH = path.join(
  REPO_ROOT,
  "scripts",
  "prepare-foundation-helper.sh"
);

describe("scripts/bootstrap-dev.sh", () => {
  test("exists", () => {
    expect(fs.existsSync(SCRIPT_PATH)).toBe(true);
  });

  test("has the executable bit set", () => {
    const mode = fs.statSync(SCRIPT_PATH).mode;
    expect(mode & 0o111).not.toBe(0);
  });

  test("provides an executable Foundation Models helper preparation script", () => {
    expect(fs.existsSync(HELPER_SCRIPT_PATH)).toBe(true);
    expect(fs.statSync(HELPER_SCRIPT_PATH).mode & 0o111).not.toBe(0);
  });

  test("is idempotent: a second consecutive invocation exits 0", () => {
    const env = {
      ...process.env,
      DEVELOPER_DIR:
        process.env.DEVELOPER_DIR ??
        "/Applications/Xcode.app/Contents/Developer",
    };
    spawnSync(SCRIPT_PATH, [], { cwd: REPO_ROOT, encoding: "utf8", env });
    const second = spawnSync(SCRIPT_PATH, [], {
      cwd: REPO_ROOT,
      encoding: "utf8",
      env,
    });

    expect(second.status).toBe(0);
  }, 15_000);
});
