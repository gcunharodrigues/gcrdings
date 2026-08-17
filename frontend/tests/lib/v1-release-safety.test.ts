import { describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import allowlist from "../../../qualification/v1/package-allowlist.json";

const REPO_ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");

function repositoryFile(relativePath: string) {
  return path.join(REPO_ROOT, relativePath);
}

function source(relativePath: string) {
  const filePath = repositoryFile(relativePath);
  return existsSync(filePath) ? readFileSync(filePath, "utf8") : "";
}

describe("Wave 7 release safety prerequisites", () => {
  test("T032 requires hash-verified bundled FFmpeg and forbids every production fallback", () => {
    const violations: string[] = [];
    const buildSource = source("frontend/src-tauri/build/ffmpeg.rs");
    const runtimeSource = source("frontend/src-tauri/src/audio/ffmpeg.rs");

    if (/reqwest|https?:\/\/|download_and_extract_ffmpeg|download_ffmpeg_package/.test(buildSource)) {
      violations.push("build can download FFmpeg instead of failing closed on the pinned local binary");
    }
    if (/ffmpeg_sidecar|\bwhich\s*\(|std::env::var\("HOME"\)|current_dir\(|\.bashrc|\.bash_profile|\.zshrc|handle_ffmpeg_installation/.test(runtimeSource)) {
      violations.push("runtime can use network, PATH, home, CWD, sidecar, or shell-profile fallback");
    }
    if (!/Sha256|sha2|ffmpeg_digest_matches|EXPECTED_FFMPEG_SHA256/.test(runtimeSource)) {
      violations.push("runtime does not hash-verify the bundled FFmpeg before returning it");
    }
    if (!/current_exe\(\)/.test(runtimeSource)) {
      violations.push("runtime has no packaged-executable-relative FFmpeg boundary");
    }
    if (/once_cell::sync::Lazy|static FFMPEG_PATH/.test(runtimeSource)) {
      violations.push("production FFmpeg validation is cached across spawn boundaries");
    }
    if (!/verify_bundled_ffmpeg_for_spawn/.test(runtimeSource)) {
      violations.push("production FFmpeg is not reverified at the spawn boundary");
    }

    expect(violations).toEqual([]);
  });

  test("T033 requires independently verified fixed-input packages and all distribution notices", () => {
    const violations: string[] = [];
    const buildScript = source("scripts/build-local-adhoc.sh");
    const helperScript = source("scripts/prepare-foundation-helper.sh");
    const verifier = source("scripts/verify-release-package.sh");
    const rustBuild = source("frontend/src-tauri/build.rs");
    const tauriConfig = source("frontend/src-tauri/tauri.conf.json");
    const fluidAudioBuild = source("frontend/src-tauri/vendor/fluidaudio-local/build.rs");
    const fluidAudioContract = source("frontend/src-tauri/vendor/fluidaudio-local/build_support.rs");
    const buildGuide = source("docs/BUILDING.md");
    const collaborationDocs = source("specs/002-complete-v1/spec.md") + source("specs/002-complete-v1/tasks.md");

    expect(allowlist).toMatchObject({
      schema_version: 1,
      distribution_mode: "local_adhoc",
      public_notarized: "unsatisfied",
    });
    expect(new Set(allowlist.application_entries).size).toBe(allowlist.application_entries.length);

    if (!buildScript) violations.push("scripts/build-local-adhoc.sh is missing");
    if (!/--remap-path-prefix/.test(buildScript)) violations.push("Rust builder paths are not remapped");
    if (!/-debug-prefix-map/.test(buildScript + helperScript)) violations.push("Swift builder paths are not remapped");
    if (!verifier) violations.push("scripts/verify-release-package.sh is missing");
    if (!/package-allowlist\.json/.test(verifier)) violations.push("package verifier does not consume the closed allowlist");
    if (!/forbidden_content_markers/.test(verifier)) violations.push("package verifier does not scan forbidden builder/private content");
    if (/no_uuid/.test(buildScript + helperScript + rustBuild)) violations.push("packaged Mach-O executables suppress LC_UUID");
    if (!/LC_UUID/.test(verifier)) violations.push("package verifier does not require launchable Mach-O UUIDs");
    if (/otool[^\n]*\|[^\n]*grep -q[^\n]*LC_UUID/.test(verifier)) {
      violations.push("package verifier can misclassify a large Mach-O when grep -q closes a pipe early");
    }
    if (!/GCRDINGS_FLUIDAUDIO_SOURCE|fluidaudio_input_hash_mismatch/.test(buildScript)) {
      violations.push("builder does not require the canonical FluidAudio archive");
    }
    const stagedFfmpeg = buildScript.indexOf('ffmpeg_source="$work_root/inputs/ffmpeg"');
    const checkoutPreservation = buildScript.indexOf("preserve_paths=(");
    if (stagedFfmpeg < 0 || checkoutPreservation < 0 || stagedFfmpeg > checkoutPreservation) {
      violations.push("builder does not stage FFmpeg before preserving checkout inputs");
    }
    if (!/GCRDINGS_LOCAL_ADHOC/.test(fluidAudioBuild) || !/validate_canonical_archive/.test(fluidAudioBuild)) {
      violations.push("local ad-hoc FluidAudio build does not fail closed on the canonical archive");
    }
    if (!/EXPECTED_FLUIDAUDIO_SHA256/.test(fluidAudioContract) || !/WrongArchitecture|MissingSymbol/.test(fluidAudioContract)) {
      violations.push("canonical FluidAudio archive is not hash, architecture, and symbol bound");
    }
    if (!/stage_canonical_archive/.test(fluidAudioBuild)) {
      violations.push("canonical FluidAudio is not copied to exclusive private staging before validation");
    }
    if (!/artifact_roots/.test(verifier)) {
      violations.push("package manifests do not carry concrete artifact roots for on-disk rehashing");
    }
    if (/normalized_sha256|normalizedMachOHash|normalized_binary_changed/.test(verifier + JSON.stringify(allowlist))) {
      violations.push("package verification still requires normalized cross-build executable equality");
    }
    if (/normalized unsigned SHA-256 must match/.test(JSON.stringify(allowlist))) {
      violations.push("package allowlist still claims cross-build executable equality");
    }
    if (!/build_once first[\s\S]*build_once second/.test(buildScript)) {
      violations.push("builder does not independently verify two clean local builds");
    }
    if (!/clean-build-results\.json/.test(buildScript) || !/clean_build_results/.test(verifier)) {
      violations.push("builder does not retain the two verified clean-build results");
    }
    if (!/swiftlang-6\.3\.3\.1\.3|17F113|macos_sdk/.test(buildScript + JSON.stringify(allowlist))) {
      violations.push("release toolchain is not pinned to Swift, Xcode, SDK, and builder macOS");
    }
    if (!/GCRDINGS_FFMPEG_SOURCE[\s\S]*ORT_LIB_LOCATION[\s\S]*GCRDINGS_FLUIDAUDIO_SOURCE[\s\S]*build-local-adhoc\.sh/.test(buildGuide)) {
      violations.push("build guide does not route all pinned inputs through the qualified builder");
    }
    if (!/authenticated private transfer/.test(buildGuide) || !/output is not a qualified release package/.test(buildGuide)) {
      violations.push("build guide does not separate authenticated release inputs from development builds");
    }
    if (/\/Users\/(?:operator|maintainer)\//.test(collaborationDocs)) {
      violations.push("collaboration docs expose a maintainer-local path");
    }

    for (const document of allowlist.required_documents) {
      if (!existsSync(repositoryFile(document))) violations.push(`${document} is missing`);
      if (!tauriConfig.includes(document)) violations.push(`${document} is not declared as a bundle resource`);
      if (!allowlist.application_entries.includes(`Contents/Resources/${document}`)) {
        violations.push(`${document} is absent from exact package membership`);
      }
    }
    const manifestProbe = spawnSync(repositoryFile("scripts/verify-release-package.sh"), ["--self-test-manifests"], {
      cwd: REPO_ROOT,
      encoding: "utf8",
    });
    if (manifestProbe.status !== 0) violations.push(`closed package manifest self-test failed: ${manifestProbe.stderr || manifestProbe.stdout}`);
    const publishProbe = spawnSync(repositoryFile("scripts/build-local-adhoc.sh"), ["--self-test-publication"], {
      cwd: REPO_ROOT,
      encoding: "utf8",
    });
    if (publishProbe.status !== 0) violations.push(`publication rollback self-test failed: ${publishProbe.stderr || publishProbe.stdout}`);

    expect(violations).toEqual([]);
  });

  test("T034 requires identical strict local, CI, audit, package, and release gates", () => {
    const violations: string[] = [];
    const gateScript = source("scripts/verify-release-gates.sh");
    const ci = source(".github/workflows/ci.yml");
    const releaseWorkflow = source(".github/workflows/release-gates.yml");
    const release = source("scripts/release.sh");
    const packageJson = JSON.parse(source("frontend/package.json"));
    const requiredCommands = [
      "cargo fmt --all --check",
      "cargo clippy --workspace --all-targets --locked -- -D warnings",
      "swift test --package-path foundation-helper",
      "cargo test --workspace --locked",
      "pnpm --dir frontend lint",
      "pnpm --dir frontend exec tsc --noEmit",
      "pnpm --dir frontend build",
      "bun test --timeout=15000 --max-concurrency=4 frontend/tests/lib",
      "cargo audit",
      "pnpm --dir frontend audit --audit-level high",
      "scripts/verify-release-package.sh",
    ];

    if (!gateScript) violations.push("scripts/verify-release-gates.sh is missing");
    for (const command of requiredCommands) {
      if (!gateScript.includes(command)) violations.push(`strict gate is missing: ${command}`);
    }
    if (ci.includes("./scripts/verify-release-gates.sh")) violations.push("fast CI must not run the long strict gate");
    if (!releaseWorkflow.includes("./scripts/verify-release-gates.sh")) violations.push("manual release workflow does not call the strict gate script");
    if (!release.includes("./scripts/verify-release-gates.sh")) violations.push("release does not call the strict gate script");
    if (!release.includes("./scripts/verify-release-package.sh")) violations.push("release does not verify package contents");
    if (/uses:\s*actions\/(?:checkout|setup-node)@v\d+/.test(ci)) violations.push("CI actions are not pinned to immutable commits");
    if (!/cargo install cargo-audit --version [0-9]/.test(releaseWorkflow)) violations.push("manual release workflow cargo-audit version is not exact");
    if (!/--self-test-clippy-baseline/.test(gateScript)) violations.push("Clippy structural exceptions are not exact-count enforced");
    const qualificationTest = source("frontend/tests/lib/v1-qualification.test.ts");
    if (!/reports only the exact T028 rows/.test(qualificationTest) || !/T028 execution remains blocked/.test(qualificationTest)) {
      violations.push("T028 blocked evidence is not separated into green exact assertions");
    }
    if (packageJson.scripts?.lint === "next lint" || !packageJson.scripts?.lint) {
      violations.push("frontend lint is interactive or undefined");
    }
    if (/cargo audit[^\n]*--ignore|pnpm[^\n]*audit[^\n]*(?:--ignore|--audit-level (?:moderate|low))/.test(gateScript + ci + release)) {
      violations.push("security audit is weakened by an ignore or lower severity threshold");
    }
    if (!/cargo-audit-reviewed-warnings\.json/.test(gateScript)) {
      violations.push("cargo audit warnings have no exact reviewed policy");
    }
    if (/-A (?:warnings|clippy::all|clippy::correctness|clippy::suspicious)/.test(gateScript)) {
      violations.push("Clippy policy suppresses a blanket or correctness warning class");
    }
    if (/-A clippy::(?:too_many_arguments|module_inception|type_complexity)/.test(gateScript)) {
      violations.push("Clippy structural exceptions remain global instead of site-scoped");
    }
    const clippyProbe = spawnSync(repositoryFile("scripts/verify-release-gates.sh"), ["--self-test-clippy-policy"], {
      cwd: REPO_ROOT,
      encoding: "utf8",
    });
    if (clippyProbe.status !== 0 || !clippyProbe.stdout.includes("rejects ordinary warnings")) {
      violations.push(`Clippy warning policy self-test failed: ${clippyProbe.stderr || clippyProbe.stdout}`);
    }
    const auditProbe = spawnSync(repositoryFile("scripts/verify-release-gates.sh"), ["--self-test-cargo-audit-policy"], {
      cwd: REPO_ROOT,
      encoding: "utf8",
    });
    if (auditProbe.status !== 0 || !auditProbe.stdout.includes("blocks unreviewed reachable warnings")) {
      violations.push(`cargo audit warning policy self-test failed: ${auditProbe.stderr || auditProbe.stdout}`);
    }

    expect(violations).toEqual([]);
  }, 30_000);

  test("release dry-run binds the exact local package paths", () => {
    const fixture = mkdtempSync(path.join(tmpdir(), "gcrdings-release-dry-run-"));
    try {
      const scripts = path.join(fixture, "scripts");
      const bin = path.join(fixture, "bin");
      mkdirSync(scripts, { recursive: true });
      mkdirSync(bin, { recursive: true });
      const executable = (filePath: string, content: string) => {
        writeFileSync(filePath, content);
        chmodSync(filePath, 0o755);
      };
      executable(path.join(scripts, "release.sh"), source("scripts/release.sh"));
      for (const name of ["bootstrap-dev.sh", "prepare-foundation-helper.sh", "verify-release-package.sh"]) {
        executable(path.join(scripts, name), "#!/bin/sh\nexit 0\n");
      }
      for (const name of ["cargo", "pnpm"]) executable(path.join(bin, name), "#!/bin/sh\nexit 0\n");
      executable(path.join(scripts, "verify-release-gates.sh"), `#!/bin/sh
test "$GCRDINGS_REQUIRE_RELEASE_PACKAGE" = 1
test "$GCRDINGS_RELEASE_APP" = "$PWD/target/release/bundle/macos/gcrdings.app"
test "$GCRDINGS_RELEASE_DMG" = "$PWD/target/release/bundle/dmg/gcrdings_0.4.0_aarch64.dmg"
echo exact-release-artifacts-bound
`);
      const result = spawnSync(path.join(scripts, "release.sh"), ["--dry-run"], {
        cwd: fixture,
        encoding: "utf8",
        env: { ...process.env, PATH: `${bin}:${process.env.PATH}` },
      });
      expect(result.status, result.stderr || result.stdout).toBe(0);
      expect(result.stdout).toContain("exact-release-artifacts-bound");
    } finally {
      rmSync(fixture, { recursive: true, force: true });
    }
  });

  test("governing plan matches the pinned Rust toolchain", () => {
    const toolchain = source("rust-toolchain.toml").match(/channel\s*=\s*"([^"]+)"/)?.[1];
    expect(toolchain).toBeTruthy();
    expect(source("specs/002-complete-v1/plan.md")).toContain(`Rust ${toolchain} edition 2021`);
  });

  test("T035 requires deterministic local_adhoc install and data-snapshot rollback", () => {
    const violations: string[] = [];
    const harness = source("scripts/qualify-local-adhoc-install.sh");
    const schema = source("qualification/v1/rollback-manifest.schema.json");
    const readme = source("qualification/v1/README.md");
    const requiredHarnessTerms = [
      "local_adhoc",
      "snapshot",
      "sha256",
      "integrity_check",
      "staged",
      "rollback",
      "install-clean",
      "install-existing",
      "rollback-interrupted",
      "hdiutil detach",
      "lsof",
      "package_manifest_sha",
      "backup_databases",
      "wal_checkpoint(FULL)",
      "sync_tree",
      "operation.lock",
      "keychain-changed",
      "original-default",
      "original-list",
      "helper_process_count",
      "journal_count",
      "recovered_candidate",
      "process_group",
      "process-identity",
      "reject_symlink_ancestors",
      "wal_checkpoint_busy",
    ];

    if (!harness) violations.push("scripts/qualify-local-adhoc-install.sh is missing");
    for (const term of requiredHarnessTerms) {
      if (!harness.includes(term)) violations.push(`rollback harness is missing contract term: ${term}`);
    }
    if (!schema) violations.push("qualification/v1/rollback-manifest.schema.json is missing");
    if (schema && !/snapshot_sha256|database_integrity|candidate_commit|restored_sha256/.test(schema)) {
      violations.push("rollback manifest does not bind snapshot, candidate, integrity, and restored hashes");
    }
    if (!readme.includes("scripts/qualify-local-adhoc-install.sh")) {
      violations.push("qualification README does not document the rollback harness");
    }
    if (/\bsudo\b/.test(harness)) violations.push("rollback harness requires administrator access");
    if (!/security default-keychain/.test(harness) || !/security list-keychains/.test(harness)) {
      violations.push("rollback harness does not preserve the user Keychain configuration");
    }
    if (/shasum -a 256 \"\$original_default\" \"\$original_list\" \| shasum/.test(harness)) {
      violations.push("Keychain configuration digest includes temporary evidence filenames");
    }
    if (!/additionalProperties"\s*:\s*false/.test(schema)) {
      violations.push("rollback evidence schema is not closed");
    }
    if (!/"package"\s*:/.test(schema) || !/application_manifest_sha256|dmg_sha256/.test(schema)) {
      violations.push("rollback evidence does not bind the exact package manifest, application, and DMG");
    }
    if (/"(?:mounted_images|candidate_processes|listeners|temporary_keychains|temporary_artifacts)"\s*:\s*0/.test(harness)) {
      violations.push("rollback cleanup evidence contains assumed zero counts");
    }
    const recovery = harness.slice(
      harness.indexOf("recover_pending_transaction()"),
      harness.indexOf("run_fixture_case()"),
    );
    const detachIndex = recovery.indexOf("detach_transaction_mount");
    const removeIndex = recovery.indexOf('remove_entry "$pending_root"');
    if (detachIndex < 0 || removeIndex < 0 || detachIndex > removeIndex) {
      violations.push("pending recovery does not detach its transaction mount before journal removal");
    }
    const fixtureRun = spawnSync(repositoryFile("scripts/qualify-local-adhoc-install.sh"), ["--self-test"], {
      cwd: REPO_ROOT,
      encoding: "utf8",
    });
    const fixtureCases = [
      "install-clean",
      "install-existing",
      "failed-launch",
      "migration-failure",
      "insufficient-disk",
      "interrupted-rollback",
      "already-installed",
      "alias-rejection",
      "package-binding",
      "sqlite-quiescence",
      "sqlite-writer-busy",
      "two-process-interruption",
      "sigkill-recovery",
      "recovery-candidate-binding",
      "symlink-ancestor",
      "measured-cleanup",
    ];
    if (fixtureRun.status !== 0) violations.push(`rollback fixture suite failed: ${fixtureRun.stderr || fixtureRun.stdout}`);
    for (const fixtureCase of fixtureCases) {
      if (!fixtureRun.stdout.includes(`self-test: ${fixtureCase} passed`)) {
        violations.push(`rollback fixture did not prove ${fixtureCase}`);
      }
    }

    expect(violations).toEqual([]);
  }, 60_000);
});
