import { describe, expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const PROVENANCE_PATH = path.join(REPO_ROOT, "PROVENANCE.md");
const GITMODULES_PATH = path.join(REPO_ROOT, ".gitmodules");
const CANONICAL_UPSTREAM = "https://github.com/Zackriya-Solutions/meetily.git";

function readProvenance(): string {
  return fs.readFileSync(PROVENANCE_PATH, "utf8");
}

describe("PROVENANCE.md", () => {
  test("exists", () => {
    expect(fs.existsSync(PROVENANCE_PATH)).toBe(true);
  });

  test("records an upstream repository URL", () => {
    const content = readProvenance();
    expect(content).toContain(CANONICAL_UPSTREAM);
  });

  test("records a revision as a 40-hex SHA, or an explicit unverified marker with a stated verification method", () => {
    const content = readProvenance();
    const hasFullSha = /\b[0-9a-f]{40}\b/i.test(content);
    const hasUnverifiedMarker =
      /`unverified`/.test(content) && /[Vv]erification method/.test(content);

    expect(hasFullSha || hasUnverifiedMarker).toBe(true);
  });

  test("records the local import commit", () => {
    const content = readProvenance();
    expect(content).toMatch(/8d2dad8/);
  });

  test("records a disposition for the whisper.cpp submodule reference", () => {
    const content = readProvenance();
    expect(content.toLowerCase()).toMatch(/whisper\.cpp/);
    expect(content).toMatch(/[Dd]isposition/);
  });

  test("if .gitmodules still exists, every submodule entry names a pinned SHA rather than only a branch", () => {
    if (!fs.existsSync(GITMODULES_PATH)) {
      // Removed per PROVENANCE.md's documented disposition — nothing to check.
      return;
    }

    const content = fs.readFileSync(GITMODULES_PATH, "utf8");
    const submoduleBlocks = content
      .split(/\[submodule/)
      .slice(1)
      .map((block) => "[submodule" + block);

    for (const block of submoduleBlocks) {
      // A pin is expressed as an explicit "sha" field the maintainer records
      // here for a follow-on `git submodule update --init` to that commit —
      // `branch = ...` alone (a moving target) does not count as pinned.
      expect(block).toMatch(/\bsha\s*=\s*[0-9a-f]{40}\b/i);
    }
  });

  test("uses no moving Git branch dependency", () => {
    const manifests = [
      path.join(REPO_ROOT, "Cargo.toml"),
      path.join(REPO_ROOT, "frontend", "src-tauri", "Cargo.toml"),
      path.join(REPO_ROOT, "foundation-helper", "Package.swift"),
    ];

    const moving = manifests.flatMap((manifest) =>
      fs.existsSync(manifest) ? fs
        .readFileSync(manifest, "utf8")
        .split("\n")
        .filter((line) => /git\s*=/.test(line) && /branch\s*=/.test(line))
        .map((line) => `${path.relative(REPO_ROOT, manifest)}: ${line.trim()}`) : [],
    );

    expect(moving).toEqual([]);
  });

  test("uses no moving Hugging Face model reference", () => {
    const files = execFileSync(
      "git",
      ["grep", "-l", "-e", "huggingface\\.co", "--", "frontend/src-tauri/src"],
      { cwd: REPO_ROOT, encoding: "utf8" },
    )
      .trim()
      .split("\n")
    .filter(Boolean);
    const moving = files.flatMap((file) =>
      fs
        .readFileSync(path.join(REPO_ROOT, file), "utf8")
        .split("\n")
        .filter((line) => line.includes("/resolve/main/"))
        .map((line) => `${path.relative(REPO_ROOT, file)}: ${line.trim()}`),
    );

    expect(moving).toEqual([]);
  });

  test("does not use Meetily infrastructure for the default Parakeet model", () => {
    const source = fs.readFileSync(
      path.join(
        REPO_ROOT,
        "frontend",
        "src-tauri",
        "src",
        "parakeet_engine",
        "parakeet_engine.rs",
      ),
      "utf8",
    );

    expect(source).not.toContain("meetily.towardsgeneralintelligence.com");
  });
});
