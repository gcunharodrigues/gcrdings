import { describe, expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");

describe("Tauri content security policy", () => {
  test("allows Next.js hydration only in development", () => {
    const config = JSON.parse(
      fs.readFileSync(path.join(REPO_ROOT, "frontend/src-tauri/tauri.conf.json"), "utf8"),
    );
    const production = config.app.security.csp;
    const development = config.app.security.devCsp;

    expect(production["script-src"] ?? production["default-src"]).not.toContain("'unsafe-eval'");
    expect(production["script-src"] ?? production["default-src"]).not.toContain("'unsafe-inline'");
    expect(development["script-src"]).toContain("'unsafe-eval'");
    expect(development["script-src"]).toContain("'unsafe-inline'");
  });
});
