#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
candidate=$(git -C "$repo_root" rev-parse HEAD)
target_triple=$(rustc -vV | awk '/^host:/ {print $2}')
expected_ffmpeg_sha256=77d2c853f431318d55ec02676d9b2f185ebfdddb9f7677a251fbe453affe025a
expected_ort_sha256=e5c83560aa9e88afa39d9dca9fb5f5a767e28adb5458d1c36fe0357131b6af8b
expected_normalized_ort_sha256=a2ab3572c0dbf30ec3c0346e8e46a6df7ccf79d0295b0d7810264b0958718a97
expected_fluidaudio_sha256=e14173844a6c296995c9e8ca0fea574168ef2204d0c682a828f46f002892ed5d
ffmpeg_source=${GCRDINGS_FFMPEG_SOURCE:-}
ort_root=${ORT_LIB_LOCATION:-}
fluidaudio_source=${GCRDINGS_FLUIDAUDIO_SOURCE:-}
output_root=${GCRDINGS_RELEASE_OUTPUT_ROOT:-"$repo_root/target"}
work_root="$output_root/.local-adhoc-work-$$"
preserved_root="$work_root/preserved"
swift_command=$(command -v swift)

fail() {
    echo "build-local-adhoc: $1" >&2
    exit 1
}

publish_bundle() {
    local stage=$1
    local destination=$2
    local previous=$3
    if [[ -e "$destination" || -L "$destination" ]]; then
        mv "$destination" "$previous"
    fi
    if ! mv "$stage" "$destination"; then
        [[ ! -e "$previous" && ! -L "$previous" ]] || mv "$previous" "$destination"
        return 1
    fi
}

restore_published_bundle() {
    local destination=$1
    local previous=$2
    if [[ -d "$destination" && ! -L "$destination" ]]; then
        rm -rf "$destination"
    else
        rm -f "$destination"
    fi
    [[ ! -e "$previous" && ! -L "$previous" ]] || mv "$previous" "$destination"
}

if [[ "${1:-}" == "--self-test-publication" ]]; then
    [[ $# -eq 1 ]] || fail "publication self-test takes no arguments"
    fixture=$(mktemp -d "${TMPDIR:-/tmp}/gcrdings-publication-test.XXXXXX")
    trap 'rm -rf "$fixture"' EXIT INT TERM
    mkdir -p "$fixture/destination" "$fixture/stage"
    printf 'previous\n' >"$fixture/destination/artifact"
    printf 'candidate\n' >"$fixture/stage/artifact"
    publish_bundle "$fixture/stage" "$fixture/destination" "$fixture/previous"
    restore_published_bundle "$fixture/destination" "$fixture/previous"
    [[ "$(<"$fixture/destination/artifact")" == "previous" ]] || fail "publication_rollback_mismatch"
    [[ ! -e "$fixture/previous" ]] || fail "publication_rollback_left_previous"
    echo "build-local-adhoc: publication rollback self-test passed"
    exit 0
fi

[[ $# -eq 0 ]] || fail "this command takes no arguments; configure pinned local inputs through the documented environment"
[[ "$(uname -m)" == "arm64" && "$(uname -s)" == "Darwin" ]] || fail "unsupported_target"
[[ -n "$ffmpeg_source" && -f "$ffmpeg_source" && ! -L "$ffmpeg_source" ]] || fail "GCRDINGS_FFMPEG_SOURCE must name the pinned local regular file"
[[ -n "$ort_root" && -f "$ort_root/lib/libonnxruntime.a" && ! -L "$ort_root/lib/libonnxruntime.a" ]] || fail "ORT_LIB_LOCATION must name the pinned local ONNX Runtime root"
[[ -n "$fluidaudio_source" && -f "$fluidaudio_source" && ! -L "$fluidaudio_source" ]] || fail "GCRDINGS_FLUIDAUDIO_SOURCE must name the pinned local regular archive"
[[ "$(shasum -a 256 "$ffmpeg_source" | awk '{print $1}')" == "$expected_ffmpeg_sha256" ]] || fail "ffmpeg_input_hash_mismatch"
[[ "$(shasum -a 256 "$ort_root/lib/libonnxruntime.a" | awk '{print $1}')" == "$expected_ort_sha256" ]] || fail "onnxruntime_input_hash_mismatch"
[[ "$(shasum -a 256 "$fluidaudio_source" | awk '{print $1}')" == "$expected_fluidaudio_sha256" ]] || fail "fluidaudio_input_hash_mismatch"
[[ "$(rustc --version | awk '{print $2}')" == "1.97.1" ]] || fail "rust_toolchain_mismatch"
[[ "$(node --version)" == "v22.23.1" ]] || fail "node_toolchain_mismatch"
[[ "$(pnpm --version)" == "10.34.5" ]] || fail "pnpm_toolchain_mismatch"
[[ "$(swiftc --version 2>/dev/null | sed -n '1p')" == "Apple Swift version 6.3.3 (swiftlang-6.3.3.1.3 clang-2100.1.1.101)" ]] || fail "swift_toolchain_mismatch"
[[ "$(swiftc --version 2>&1 >/dev/null)" == "swift-driver version: 1.148.6 " ]] || fail "swift_driver_mismatch"
[[ "$(xcodebuild -version)" == $'Xcode 26.6\nBuild version 17F113' ]] || fail "xcode_toolchain_mismatch"
[[ "$(xcrun --sdk macosx --show-sdk-version)" == "26.5" ]] || fail "macos_sdk_mismatch"
[[ "$(sw_vers -productVersion)" == "26.5.2" ]] || fail "macos_builder_mismatch"
[[ -x "$repo_root/frontend/node_modules/.bin/tauri" ]] || fail "frozen frontend dependencies are not installed"
git -C "$repo_root" diff --quiet --ignore-submodules -- || fail "tracked_worktree_not_clean"
git -C "$repo_root" diff --cached --quiet --ignore-submodules -- || fail "tracked_index_not_clean"

mkdir -p "$work_root/inputs"
trap 'rm -rf "$work_root"' EXIT INT TERM
cp "$ffmpeg_source" "$work_root/inputs/ffmpeg"
chmod 0755 "$work_root/inputs/ffmpeg"
[[ "$(shasum -a 256 "$work_root/inputs/ffmpeg" | awk '{print $1}')" == "$expected_ffmpeg_sha256" ]] || fail "staged_ffmpeg_hash_mismatch"
ffmpeg_source="$work_root/inputs/ffmpeg"

mkdir -p "$preserved_root"
preserve_paths=(
    "frontend/.next"
    "frontend/out"
    "frontend/next-env.d.ts"
    "frontend/tsconfig.tsbuildinfo"
    "frontend/src-tauri/binaries/ffmpeg-$target_triple"
    "frontend/src-tauri/binaries/foundation-helper-$target_triple"
)
preserved=()
publication_destination=
publication_previous=
publication_swapped=0
publication_committed=0
for relative in "${preserve_paths[@]}"; do
    source_path="$repo_root/$relative"
    if [[ -e "$source_path" || -L "$source_path" ]]; then
        backup_path="$preserved_root/$relative"
        mkdir -p "$(dirname "$backup_path")"
        mv "$source_path" "$backup_path"
        preserved+=("$relative")
    fi
done

cleanup() {
    if [[ "$publication_swapped" == "1" && "$publication_committed" == "0" ]]; then
        restore_published_bundle "$publication_destination" "$publication_previous"
        publication_swapped=0
    fi
    for relative in "${preserve_paths[@]}"; do
        generated="$repo_root/$relative"
        if [[ -d "$generated" && ! -L "$generated" ]]; then
            rm -rf "$generated"
        else
            rm -f "$generated"
        fi
    done
    for relative in "${preserved[@]}"; do
        backup_path="$preserved_root/$relative"
        destination="$repo_root/$relative"
        mkdir -p "$(dirname "$destination")"
        mv "$backup_path" "$destination"
    done
    rm -rf "$work_root"
}
trap cleanup EXIT INT TERM

ort_build_root="$work_root/onnxruntime"
mkdir -p "$ort_build_root/lib"
cp "$ort_root/lib/libonnxruntime.a" "$ort_build_root/lib/libonnxruntime.a"
LC_ALL=C /usr/bin/perl -0pi -e 's#/Users/runner#/workspace/ci#g' "$ort_build_root/lib/libonnxruntime.a"
[[ "$(shasum -a 256 "$ort_build_root/lib/libonnxruntime.a" | awk '{print $1}')" == "$expected_normalized_ort_sha256" ]] || fail "onnxruntime_normalized_hash_mismatch"
if LC_ALL=C grep -a -q '/Users/' "$ort_build_root/lib/libonnxruntime.a"; then
    fail "onnxruntime_private_builder_path"
fi

mkdir -p "$work_root/bin"
cat >"$work_root/bin/swift" <<EOF
#!/usr/bin/env bash
set -euo pipefail
if [[ "\${1:-}" == "build" ]]; then
    swift_build_root="$work_root"
    previous=
    has_oso=0
    for argument in "\$@"; do
        if [[ "\$previous" == "--scratch-path" || "\$previous" == "--build-path" ]]; then
            swift_build_root="\$argument"
        fi
        if [[ "\$argument" == "-oso_prefix" ]]; then
            has_oso=1
        fi
        previous="\$argument"
    done
    if [[ \$has_oso -eq 1 ]]; then
        exec "$swift_command" "\$@" \\
            --jobs 1 \\
            -Xswiftc -debug-prefix-map -Xswiftc "$repo_root=/workspace/gcrdings" \\
            -Xswiftc -file-prefix-map -Xswiftc "$repo_root=/workspace/gcrdings" \\
            -Xswiftc -debug-prefix-map -Xswiftc "$work_root=/workspace/release-work" \\
            -Xswiftc -file-prefix-map -Xswiftc "$work_root=/workspace/release-work" \\
            -Xswiftc -debug-prefix-map -Xswiftc "\$swift_build_root=/workspace/swift-build" \\
            -Xswiftc -file-prefix-map -Xswiftc "\$swift_build_root=/workspace/swift-build" \\
            -Xswiftc -file-compilation-dir -Xswiftc /workspace/gcrdings \\
            -Xswiftc -num-threads -Xswiftc 1
    fi
    exec "$swift_command" "\$@" \\
        --jobs 1 \\
        -Xswiftc -debug-prefix-map -Xswiftc "$repo_root=/workspace/gcrdings" \\
        -Xswiftc -file-prefix-map -Xswiftc "$repo_root=/workspace/gcrdings" \\
        -Xswiftc -debug-prefix-map -Xswiftc "$work_root=/workspace/release-work" \\
        -Xswiftc -file-prefix-map -Xswiftc "$work_root=/workspace/release-work" \\
        -Xswiftc -debug-prefix-map -Xswiftc "\$swift_build_root=/workspace/swift-build" \\
        -Xswiftc -file-prefix-map -Xswiftc "\$swift_build_root=/workspace/swift-build" \\
        -Xswiftc -file-compilation-dir -Xswiftc /workspace/gcrdings \\
        -Xswiftc -num-threads -Xswiftc 1 \\
        -Xlinker -oso_prefix -Xlinker "\$swift_build_root"
fi
exec "$swift_command" "\$@"
EOF
chmod 0755 "$work_root/bin/swift"

source_date_epoch=$(git -C "$repo_root" show -s --format=%ct "$candidate")
common_rustflags="--remap-path-prefix=$HOME/.cargo=/workspace/cargo --remap-path-prefix=$repo_root=/workspace/gcrdings --remap-path-prefix=$work_root=/workspace/release-work -C link-arg=-mmacosx-version-min=14.2"
common_cflags="-ffile-prefix-map=$HOME/.cargo=/workspace/cargo -fdebug-prefix-map=$HOME/.cargo=/workspace/cargo -ffile-prefix-map=$repo_root=/workspace/gcrdings -fdebug-prefix-map=$repo_root=/workspace/gcrdings -ffile-prefix-map=$work_root=/workspace/release-work -fdebug-prefix-map=$work_root=/workspace/release-work"

build_once() {
    label=$1
    build_target="$work_root/build-target"
    helper_target="$work_root/swift-build"
    manifest="$work_root/$label-manifest.json"
    build_rustflags="$common_rustflags --remap-path-prefix=$build_target=/workspace/cargo-target"
    build_cflags="$common_cflags -ffile-prefix-map=$build_target=/workspace/cargo-target -fdebug-prefix-map=$build_target=/workspace/cargo-target"

    rm -rf "$repo_root/frontend/.next" "$repo_root/frontend/out"
    rm -f "$repo_root/frontend/tsconfig.tsbuildinfo"
    (
        export PATH="$work_root/bin:$PATH"
        export TZ=UTC LC_ALL=C LANG=C
        export SOURCE_DATE_EPOCH="$source_date_epoch"
        export ZERO_AR_DATE=1
        export SWIFT_DETERMINISTIC_HASHING=1
        export CARGO_NET_OFFLINE=true
        export CARGO_TARGET_DIR="$build_target"
        export CARGO_INCREMENTAL=0
        export RUSTFLAGS="$build_rustflags"
        export CFLAGS="$build_cflags"
        export CXXFLAGS="$build_cflags"
        export GCRDINGS_BUILD_COMMIT="$candidate"
        export GCRDINGS_RELEASE_CANDIDATE="$candidate"
        export GCRDINGS_LOCAL_ADHOC=1
        export GCRDINGS_FFMPEG_SOURCE="$ffmpeg_source"
        export GCRDINGS_FLUIDAUDIO_STATIC_LIBRARY="$fluidaudio_source"
        export ORT_LIB_LOCATION="$ort_build_root"
        export FOUNDATION_HELPER_BUILD_DIR="$helper_target"
        export TAURI_GPU_FEATURE=metal

        cargo metadata --locked --offline --format-version 1 >/dev/null
        "$repo_root/scripts/prepare-foundation-helper.sh"
        pnpm --dir "$repo_root/frontend" exec tauri build --bundles app,dmg -- --locked
        app="$build_target/release/bundle/macos/gcrdings.app"
        dmg="$build_target/release/bundle/dmg/gcrdings_0.4.0_aarch64.dmg"
        artifact_root="$work_root/$label-artifacts"
        mkdir -p "$artifact_root/macos" "$artifact_root/dmg"
        /usr/bin/ditto "$app" "$artifact_root/macos/gcrdings.app"
        cp "$dmg" "$artifact_root/dmg/gcrdings_0.4.0_aarch64.dmg"
        "$repo_root/scripts/verify-release-package.sh" \
            "$artifact_root/macos/gcrdings.app" \
            "$artifact_root/dmg/gcrdings_0.4.0_aarch64.dmg" \
            --write-manifest "$manifest"
    )
}

build_once first
rm -rf "$work_root/build-target" "$work_root/swift-build"
build_once second
"$repo_root/scripts/verify-release-package.sh" --compare-manifests \
    "$work_root/first-manifest.json" \
    "$work_root/second-manifest.json" \
    "$work_root/clean-build-results.json"

publication_stage="$work_root/publication"
mkdir -p "$publication_stage/macos" "$publication_stage/dmg"
/usr/bin/ditto "$work_root/second-artifacts/macos/gcrdings.app" "$publication_stage/macos/gcrdings.app"
cp "$work_root/second-artifacts/dmg/gcrdings_0.4.0_aarch64.dmg" "$publication_stage/dmg/"
cp "$work_root/clean-build-results.json" "$publication_stage/"

destination="$output_root/release/bundle"
mkdir -p "$(dirname "$destination")"
publication_destination=$destination
publication_previous="$work_root/previous-bundle"
publish_bundle "$publication_stage" "$publication_destination" "$publication_previous"
publication_swapped=1

GCRDINGS_RELEASE_CANDIDATE="$candidate" \
    "$repo_root/scripts/verify-release-package.sh" \
    "$destination/macos/gcrdings.app" \
    "$destination/dmg/gcrdings_0.4.0_aarch64.dmg" \
    --write-manifest "$destination/release-package-manifest.json"
GCRDINGS_RELEASE_CANDIDATE="$candidate" \
    "$repo_root/scripts/verify-release-package.sh" \
    "$destination/macos/gcrdings.app" \
    "$destination/dmg/gcrdings_0.4.0_aarch64.dmg"
publication_committed=1
echo "build-local-adhoc: fixed-input local_adhoc package ready"
