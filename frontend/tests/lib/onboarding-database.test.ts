import { describe, expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { initializeFirstLaunchDatabase } from "../../src/lib/onboarding-database";

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");

describe("first-launch database initialization", () => {
  test("a failed legacy import never falls back to a fresh database", async () => {
    const commands: string[] = [];
    const invoke = async <T>(command: string): Promise<T> => {
      commands.push(command);
      if (command === "check_default_legacy_database") {
        return "/synthetic/legacy.db" as T;
      }
      if (command === "import_and_initialize_database") {
        throw new Error("synthetic Keychain failure");
      }
      throw new Error(`unexpected command: ${command}`);
    };

    await expect(initializeFirstLaunchDatabase(invoke)).rejects.toThrow(
      "synthetic Keychain failure",
    );
    expect(commands).toEqual([
      "check_default_legacy_database",
      "import_and_initialize_database",
    ]);
  });

  test("a fresh database is created only after confirmed legacy absence", async () => {
    const commands: string[] = [];
    const invoke = async <T>(command: string): Promise<T> => {
      commands.push(command);
      return (command === "check_default_legacy_database" ? null : undefined) as T;
    };

    await expect(initializeFirstLaunchDatabase(invoke)).resolves.toBe("fresh");
    expect(commands).toEqual([
      "check_default_legacy_database",
      "initialize_fresh_database",
    ]);
  });

  test("the renderer delegates first-launch decisions to the guarded flow", () => {
    const context = fs.readFileSync(
      path.join(ROOT, "frontend/src/contexts/OnboardingContext.tsx"),
      "utf8",
    );

    expect(context).toContain("initializeFirstLaunchDatabase(invoke)");
    expect(context).not.toContain("invoke('initialize_fresh_database')");
  });

  test("the native fresh command rejects repeat state before defaults or readiness", () => {
    const commands = fs.readFileSync(
      path.join(ROOT, "frontend/src-tauri/src/database/commands.rs"),
      "utf8",
    );
    const start = commands.indexOf("pub async fn initialize_fresh_database");
    const end = commands.indexOf("/// Get the database directory path", start);
    const fresh = commands.slice(start, end);

    expect(fresh).toContain("try_state::<AppState>()");
    expect(fresh).toContain("create_fresh");
    expect(fresh).toContain("if !app.manage");
    expect(fresh.indexOf("create_fresh")).toBeLessThan(
      fresh.indexOf("save_model_config"),
    );
    expect(fresh.indexOf("if !app.manage")).toBeLessThan(
      fresh.indexOf('app.emit("database-initialized"'),
    );
  });
});
