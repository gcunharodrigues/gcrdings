#!/usr/bin/env bash
set -euo pipefail

mode=${1:-}
shift || true
self_test_mode=0

fail() {
    echo "qualify-local-adhoc-install: $1" >&2
    exit 1
}

reject_symlink_ancestors() {
    local target=$1
    local current=/
    local component
    [[ "$target" == /* ]] || fail "path_must_be_absolute"
    IFS='/' read -r -a components <<<"${target#/}"
    for component in "${components[@]}"; do
        [[ -n "$component" ]] || continue
        current="${current%/}/$component"
        [[ ! -L "$current" ]] || fail "symlink_ancestor_rejected"
    done
}

sync_path() {
    local target=$1
    [[ -e "$target" && ! -L "$target" ]] || fail "sync_target_invalid"
    [[ "$self_test_mode" == "0" ]] || return 0
    /bin/sync -f "$target"
    /bin/sync -f "$(dirname "$target")"
}

require_regular_tree() {
    local tree=$1
    local alias_entry
    [[ ! -L "$tree" ]] || fail "alias_entry_rejected"
    if [[ -e "$tree" ]]; then
        alias_entry=$(/usr/bin/find -P "$tree" -type l -print -quit)
        [[ -z "$alias_entry" ]] || fail "alias_entry_rejected"
    fi
}

tree_sha256() {
    local tree=$1
    local root
    if [[ ! -e "$tree" ]]; then
        printf '%s' absent | shasum -a 256 | awk '{print $1}'
        return
    fi
    require_regular_tree "$tree"
    (
        cd "$(dirname "$tree")"
        root=$(basename "$tree")
        /usr/bin/find -s "$root" -print | while IFS= read -r entry; do
            if [[ -d "$entry" ]]; then
                printf 'd\t%s\t%s\n' "$(stat -f '%Lp' "$entry")" "$entry"
            elif [[ -f "$entry" ]]; then
                printf 'f\t%s\t%s\t%s\t%s\n' \
                    "$(stat -f '%Lp' "$entry")" \
                    "$(stat -f '%z' "$entry")" \
                    "$(shasum -a 256 "$entry" | awk '{print $1}')" \
                    "$entry"
            else
                fail "unsupported_snapshot_entry"
            fi
        done
    ) | shasum -a 256 | awk '{print $1}'
}

tree_content_sha256() {
    local tree=$1
    if [[ ! -e "$tree" ]]; then
        printf '%s' absent | shasum -a 256 | awk '{print $1}'
        return
    fi
    require_regular_tree "$tree"
    (
        cd "$tree"
        /usr/bin/find -s . -print | while IFS= read -r entry; do
            if [[ -d "$entry" ]]; then
                printf 'd\t%s\t%s\n' "$(stat -f '%Lp' "$entry")" "$entry"
            elif [[ -f "$entry" ]]; then
                printf 'f\t%s\t%s\t%s\t%s\n' \
                    "$(stat -f '%Lp' "$entry")" \
                    "$(stat -f '%z' "$entry")" \
                    "$(shasum -a 256 "$entry" | awk '{print $1}')" \
                    "$entry"
            else
                fail "unsupported_snapshot_entry"
            fi
        done
    ) | shasum -a 256 | awk '{print $1}'
}

integrity_check_databases() {
    local tree=$1
    local database
    [[ -d "$tree" ]] || return 0
    require_regular_tree "$tree"
    /usr/bin/find -s "$tree" -type f \( -name '*.sqlite' -o -name '*.db' \) -print |
        while IFS= read -r database; do
            [[ "$(sqlite3 "$database" 'PRAGMA quick_check;' 2>/dev/null)" == "ok" ]] ||
                fail "database_integrity_failed"
        done
}

copy_tree() {
    local source=$1
    local destination=$2
    require_regular_tree "$source"
    mkdir -p "$(dirname "$destination")"
    /usr/bin/ditto --noqtn "$source" "$destination"
}

sync_tree() {
    local tree=$1
    [[ "$self_test_mode" == "0" ]] || return 0
    if [[ -d "$tree" ]]; then
        /usr/bin/find -P "$tree" -type f -exec /bin/sync -f {} \;
    fi
    /bin/sync -f "$tree"
    /bin/sync -f "$(dirname "$tree")"
}

quiesce_databases() {
    local tree=$1
    local database output checkpoint busy log_frames checkpointed
    [[ -d "$tree" ]] || return 0
    /usr/bin/find -P "$tree" -type f \( -name '*.sqlite' -o -name '*.db' \) -print |
        while IFS= read -r database; do
            output=$(printf '.timeout 0\nPRAGMA wal_checkpoint(FULL);\nPRAGMA quick_check;\n' |
                sqlite3 "$database" 2>/dev/null) ||
                fail "database_quiescence_failed"
            checkpoint=$(printf '%s\n' "$output" | head -n 1)
            IFS='|' read -r busy log_frames checkpointed <<<"$checkpoint"
            [[ "$busy" == "0" && "$log_frames" =~ ^-?[0-9]+$ && "$checkpointed" == "$log_frames" ]] ||
                fail "wal_checkpoint_busy"
            [[ "$(printf '%s\n' "$output" | tail -n 1)" == "ok" ]] ||
                fail "database_quiescence_failed"
        done
}

remove_sqlite_sidecars() {
    local tree=$1
    [[ -d "$tree" ]] || return 0
    /usr/bin/find -P "$tree" -type f \( -name '*-wal' -o -name '*-shm' \) -delete
}

backup_databases() {
    local source_tree=$1
    local snapshot_tree=$2
    local database relative destination staged
    [[ -d "$source_tree" ]] || return 0
    /usr/bin/find -P "$source_tree" -type f \( -name '*.sqlite' -o -name '*.db' \) -print |
        while IFS= read -r database; do
            relative=${database#"$source_tree"/}
            destination="$snapshot_tree/$relative"
            staged="${destination}.gcrdings-backup"
            [[ "$staged" != *'"'* && "$staged" != *$'\n'* ]] || fail "database_backup_path_invalid"
            sqlite3 "$database" ".backup \"$staged\"" || fail "database_backup_failed"
            [[ "$(sqlite3 "$staged" 'PRAGMA quick_check;' 2>/dev/null)" == "ok" ]] ||
                fail "database_backup_integrity_failed"
            mv "$staged" "$destination"
            [[ "$self_test_mode" != "0" ]] || /bin/sync -f "$destination"
        done
}

remove_entry() {
    local entry=$1
    [[ -n "$entry" && "$entry" != / && "$entry" != "$HOME" ]] || fail "unsafe_cleanup_target"
    if [[ -d "$entry" && ! -L "$entry" ]]; then
        rm -rf "$entry"
    else
        rm -f "$entry"
    fi
}

restore_entry() {
    local snapshot=$1
    local destination=$2
    local existed=$3
    local transaction_id=$4
    local stage=$5
    local displaced=$6
    [[ "$transaction_id" =~ ^transaction\.[A-Za-z0-9]+$ ]] || fail "transaction_id_invalid"
    [[ "$stage" == "${destination}.gcrdings-restore-$transaction_id" &&
       "$displaced" == "${destination}.gcrdings-displaced-$transaction_id" ]] ||
        fail "transaction_restore_paths_invalid"

    if [[ -e "$displaced" || -L "$displaced" ]]; then
        if [[ -e "$destination" || -L "$destination" ]]; then
            if [[ "$existed" == "1" ]] &&
                [[ "$(tree_content_sha256 "$destination")" == "$(tree_content_sha256 "$snapshot")" ]]; then
                remove_entry "$displaced"
            else
                remove_entry "$destination"
                mv "$displaced" "$destination"
                sync_path "$destination"
            fi
        else
            mv "$displaced" "$destination"
            sync_path "$destination"
        fi
    fi
    remove_entry "$stage"
    if [[ "$existed" == "1" ]]; then
        copy_tree "$snapshot" "$stage"
        remove_sqlite_sidecars "$stage"
        [[ "$(tree_content_sha256 "$stage")" == "$(tree_content_sha256 "$snapshot")" ]] ||
            fail "staged_restore_mismatch"
        sync_tree "$stage"
    fi
    if [[ -e "$destination" || -L "$destination" ]]; then
        mv "$destination" "$displaced"
    fi
    if [[ "$existed" == "1" ]]; then
        mv "$stage" "$destination"
        sync_path "$destination"
        [[ "$(tree_content_sha256 "$destination")" == "$(tree_content_sha256 "$snapshot")" ]] || {
            remove_entry "$destination"
            [[ ! -e "$displaced" && ! -L "$displaced" ]] || mv "$displaced" "$destination"
            fail "post_swap_restore_mismatch"
        }
        sync_tree "$destination"
    fi
    remove_entry "$displaced"
}

snapshot_state() {
    local work_root=$1
    local app_target=$2
    local data_target=$3
    local app_existed=0
    local data_existed=0
    [[ -f "$work_root/transaction-id" && ! -L "$work_root/transaction-id" ]] ||
        fail "transaction_id_missing"
    if [[ -e "$app_target" || -L "$app_target" ]]; then
        require_regular_tree "$app_target"
        copy_tree "$app_target" "$work_root/snapshot/app"
        app_existed=1
    fi
    if [[ -e "$data_target" || -L "$data_target" ]]; then
        require_regular_tree "$data_target"
        quiesce_databases "$data_target"
        integrity_check_databases "$data_target"
        copy_tree "$data_target" "$work_root/snapshot/data"
        backup_databases "$data_target" "$work_root/snapshot/data"
        remove_sqlite_sidecars "$work_root/snapshot/data"
        integrity_check_databases "$work_root/snapshot/data"
        remove_sqlite_sidecars "$work_root/snapshot/data"
        data_existed=1
    fi
    printf '%s\n' "$app_existed" >"$work_root/app-existed"
    printf '%s\n' "$data_existed" >"$work_root/data-existed"
    tree_content_sha256 "$work_root/snapshot/app" >"$work_root/app-before.sha256"
    tree_content_sha256 "$work_root/snapshot/data" >"$work_root/data-before.sha256"
    chmod -R a-w "$work_root/snapshot" 2>/dev/null || true
    printf 'complete\n' >"$work_root/snapshot-complete"
    sync_tree "$work_root"
}

restore_state() {
    local work_root=$1
    local app_target=$2
    local data_target=$3
    local transaction_id app_stage app_displaced data_stage data_displaced
    transaction_id=$(<"$work_root/transaction-id")
    app_stage=$(jq -r '.paths.application_stage' "$work_root/transaction.json")
    app_displaced=$(jq -r '.paths.application_displaced' "$work_root/transaction.json")
    data_stage=$(jq -r '.paths.data_stage' "$work_root/transaction.json")
    data_displaced=$(jq -r '.paths.data_displaced' "$work_root/transaction.json")
    chmod -R u+w "$work_root/snapshot" 2>/dev/null || true
    restore_entry "$work_root/snapshot/app" "$app_target" "$(<"$work_root/app-existed")" \
        "$transaction_id" "$app_stage" "$app_displaced"
    restore_entry "$work_root/snapshot/data" "$data_target" "$(<"$work_root/data-existed")" \
        "$transaction_id" "$data_stage" "$data_displaced"
    integrity_check_databases "$data_target"
    remove_sqlite_sidecars "$data_target"
    [[ "$(tree_content_sha256 "$app_target")" == "$(<"$work_root/app-before.sha256")" ]] ||
        fail "application_restore_mismatch"
    [[ "$(tree_content_sha256 "$data_target")" == "$(<"$work_root/data-before.sha256")" ]] ||
        fail "data_restore_mismatch"
}

write_transaction_journal() {
    local root=$1
    local transaction_id=$2
    local candidate_commit=$3
    local app_target=$4
    local data_target=$5
    local keychain_path=$6
    local journal_stage="$root/transaction.json.tmp"
    jq -n \
        --arg transaction_id "$transaction_id" \
        --arg candidate_commit "$candidate_commit" \
        --arg app_stage "${app_target}.gcrdings-restore-$transaction_id" \
        --arg app_displaced "${app_target}.gcrdings-displaced-$transaction_id" \
        --arg data_stage "${data_target}.gcrdings-restore-$transaction_id" \
        --arg data_displaced "${data_target}.gcrdings-displaced-$transaction_id" \
        --arg keychain "$keychain_path" \
        '{schema_version: 1, transaction_id: $transaction_id, candidate_commit: $candidate_commit,
          paths: {application_stage: $app_stage, application_displaced: $app_displaced,
            data_stage: $data_stage, data_displaced: $data_displaced, keychain: $keychain,
            keychain_original_default: "original-default", keychain_original_list: "original-list"}}' \
        >"$journal_stage"
    chmod 600 "$journal_stage"
    sync_path "$journal_stage"
    mv "$journal_stage" "$root/transaction.json"
    sync_path "$root/transaction.json"
}

acquire_operation_lock() {
    local root=$1
    local lock="$root/operation.lock"
    local owner
    if mkdir "$lock" 2>/dev/null; then
        printf '%s\n' "$$" >"$lock/pid"
        [[ "$self_test_mode" != "0" ]] || /bin/sync -f "$lock/pid"
        return 0
    fi
    [[ -d "$lock" && ! -L "$lock" && -f "$lock/pid" && ! -L "$lock/pid" ]] || return 1
    owner=$(<"$lock/pid")
    [[ "$owner" =~ ^[0-9]+$ ]] || return 1
    kill -0 "$owner" 2>/dev/null && return 1
    remove_entry "$lock"
    mkdir "$lock"
    printf '%s\n' "$$" >"$lock/pid"
    [[ "$self_test_mode" != "0" ]] || /bin/sync -f "$lock/pid"
}

release_operation_lock() {
    local root=$1
    local lock="$root/operation.lock"
    [[ ! -d "$lock" || -L "$lock" ]] || remove_entry "$lock"
}

restore_keychain_transaction() {
    local root=$1
    local default_value keychain_entry keychain_path
    local search_list=()
    [[ -f "$root/keychain-changed" && ! -L "$root/keychain-changed" ]] || return 0
    [[ -f "$root/original-default" && -f "$root/original-list" && -f "$root/keychain-path" ]] ||
        fail "keychain_recovery_state_invalid"
    default_value=$(sed -e 's/^[[:space:]]*"//' -e 's/"[[:space:]]*$//' "$root/original-default")
    while IFS= read -r keychain_entry; do
        keychain_entry=$(printf '%s' "$keychain_entry" | sed -e 's/^[[:space:]]*"//' -e 's/"[[:space:]]*$//')
        [[ -z "$keychain_entry" ]] || search_list+=("$keychain_entry")
    done <"$root/original-list"
    security default-keychain -d user -s "$default_value" >/dev/null
    security list-keychains -d user -s "${search_list[@]}" >/dev/null
    keychain_path=$(<"$root/keychain-path")
    [[ "$keychain_path" == "$root/qualification.keychain-db" ]] || fail "keychain_recovery_state_invalid"
    security delete-keychain "$keychain_path" >/dev/null 2>&1 || [[ ! -e "$keychain_path" ]] ||
        fail "temporary_keychain_cleanup_failed"
    rm -f "$root/keychain-changed"
}

recorded_process_identity_matches() {
    local root=$1
    local process_group=$2
    local identity_file="$root/process-identity"
    local expected_executable expected_started_at observed_executable observed_group observed_started_at
    [[ -f "$identity_file" && ! -L "$identity_file" ]] || return 1
    jq -e --argjson process_group "$process_group" '
        type == "object" and
        (keys == ["executable", "process_group", "schema_version", "started_at"]) and
        .schema_version == 1 and .process_group == $process_group and
        (.executable | type == "string" and startswith("/")) and
        (.started_at | type == "string" and length > 0)
    ' "$identity_file" >/dev/null || return 1
    expected_executable=$(jq -r '.executable' "$identity_file")
    expected_started_at=$(jq -r '.started_at' "$identity_file")
    observed_group=$(ps -o pgid= -p "$process_group" 2>/dev/null | tr -d ' ' || true)
    observed_started_at=$(ps -o lstart= -p "$process_group" 2>/dev/null | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' || true)
    observed_executable=$(lsof -a -p "$process_group" -d txt -Fn 2>/dev/null | sed -n 's/^n//p' | head -n 1 || true)
    [[ "$observed_group" == "$process_group" &&
       "$observed_started_at" == "$expected_started_at" &&
       "$observed_executable" == "$expected_executable" ]]
}

terminate_recorded_process_group() {
    local root=$1
    local group_file="$root/process-group"
    local process_group own_group
    [[ -f "$group_file" && ! -L "$group_file" ]] || return 0
    process_group=$(<"$group_file")
    own_group=$(ps -o pgid= -p $$ | tr -d ' ')
    [[ "$process_group" =~ ^[0-9]+$ && "$process_group" != "$own_group" ]] ||
        fail "process_group_invalid"
    if kill -0 -- "-$process_group" 2>/dev/null; then
        recorded_process_identity_matches "$root" "$process_group" || fail "process_identity_mismatch"
        kill -TERM -- "-$process_group" 2>/dev/null || true
        for _ in {1..50}; do
            kill -0 -- "-$process_group" 2>/dev/null || break
            sleep 0.1
        done
        if kill -0 -- "-$process_group" 2>/dev/null; then
            recorded_process_identity_matches "$root" "$process_group" || fail "process_identity_mismatch"
        fi
        kill -0 -- "-$process_group" 2>/dev/null && kill -KILL -- "-$process_group" 2>/dev/null || true
    fi
    kill -0 -- "-$process_group" 2>/dev/null && fail "candidate_process_group_cleanup_failed"
    rm -f "$group_file" "$root/process-identity"
    [[ "$self_test_mode" != "0" ]] || /bin/sync -f "$root"
}

record_process_identity() {
    local root=$1
    local process_group=$2
    local expected_executable=$3
    local observed_executable observed_group observed_started_at stage
    for _ in {1..50}; do
        observed_group=$(ps -o pgid= -p "$process_group" 2>/dev/null | tr -d ' ' || true)
        observed_started_at=$(ps -o lstart= -p "$process_group" 2>/dev/null | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' || true)
        observed_executable=$(lsof -a -p "$process_group" -d txt -Fn 2>/dev/null | sed -n 's/^n//p' | head -n 1 || true)
        [[ "$observed_group" == "$process_group" && -n "$observed_started_at" &&
           "$observed_executable" == "$expected_executable" ]] && break
        sleep 0.02
    done
    [[ "$observed_group" == "$process_group" && -n "$observed_started_at" &&
       "$observed_executable" == "$expected_executable" ]] || fail "process_identity_unavailable"
    stage="$root/.process-identity.$$"
    jq -n --argjson process_group "$process_group" --arg executable "$observed_executable" \
        --arg started_at "$observed_started_at" \
        '{schema_version: 1, process_group: $process_group, executable: $executable, started_at: $started_at}' \
        >"$stage"
    chmod 600 "$stage"
    sync_path "$stage"
    mv "$stage" "$root/process-identity"
    sync_path "$root/process-identity"
}

detach_transaction_mount() {
    local root=$1
    local transaction_mount="$root/mount"
    [[ -e "$transaction_mount" ]] || return 0
    [[ -d "$transaction_mount" && ! -L "$transaction_mount" ]] || fail "pending_mount_invalid"
    if hdiutil info 2>/dev/null | grep -F -q "$transaction_mount"; then
        hdiutil detach "$transaction_mount" >/dev/null 2>&1 ||
            hdiutil detach -force "$transaction_mount" >/dev/null 2>&1 ||
            fail "pending_mount_detach_failed"
    fi
}

recover_pending_transaction() {
    local pending=$1
    local journal_root=$2
    local app_target=$3
    local data_target=$4
    local pending_name pending_root
    [[ -f "$pending" && ! -L "$pending" ]] || return 0
    jq -e 'type == "object" and (keys == ["candidate_commit", "schema_version", "transaction_id"]) and .schema_version == 1' "$pending" >/dev/null ||
        fail "invalid_pending_rollback"
    pending_name=$(jq -r '.transaction_id' "$pending")
    recovered_candidate=$(jq -r '.candidate_commit' "$pending")
    [[ "$pending_name" =~ ^transaction\.[A-Za-z0-9]+$ && "$recovered_candidate" =~ ^[0-9a-f]{40}$ ]] ||
        fail "invalid_pending_rollback"
    pending_root="$journal_root/$pending_name"
    [[ -d "$pending_root" && ! -L "$pending_root" ]] || fail "missing_pending_snapshot"
    [[ "$(<"$pending_root/transaction-id")" == "$pending_name" ]] || fail "pending_transaction_mismatch"
    [[ -f "$pending_root/transaction.json" && ! -L "$pending_root/transaction.json" ]] ||
        fail "pending_transaction_missing"
    jq -e --arg transaction_id "$pending_name" --arg candidate_commit "$recovered_candidate" '
        type == "object" and
        (keys == ["candidate_commit", "paths", "schema_version", "transaction_id"]) and
        .schema_version == 1 and .transaction_id == $transaction_id and
        .candidate_commit == $candidate_commit and
        (.paths | type == "object" and
          (keys == ["application_displaced", "application_stage", "data_displaced", "data_stage",
                    "keychain", "keychain_original_default", "keychain_original_list"]))
    ' "$pending_root/transaction.json" >/dev/null || fail "pending_candidate_mismatch"
    terminate_recorded_process_group "$pending_root"
    detach_transaction_mount "$pending_root"
    if [[ -f "$pending_root/snapshot-complete" && ! -L "$pending_root/snapshot-complete" ]]; then
        restore_state "$pending_root" "$app_target" "$data_target"
    fi
    restore_keychain_transaction "$pending_root"
    remove_entry "$pending_root"
    rm -f "$pending"
    [[ "$self_test_mode" != "0" ]] || /bin/sync -f "$journal_root"
    recovery_occurred=1
}

run_fixture_case() {
    local case_id=$1
    local fixture=$2
    local current_app="$fixture/current/gcrdings.app"
    local current_data="$fixture/current/data"
    local candidate_app="$fixture/candidate/gcrdings.app"
    local work="$fixture/work-$case_id"
    local staged
    local failure_status
    local available_kib
    local required_kib
    mkdir -p "$candidate_app/Contents/MacOS" "$work"
    printf 'transaction.fixture\n' >"$work/transaction-id"
    write_transaction_journal "$work" transaction.fixture "$(printf '0%.0s' {1..40})" \
        "$current_app" "$current_data" "$work/qualification.keychain-db"
    printf 'candidate-app\n' >"$candidate_app/Contents/MacOS/gcrdings"
    if [[ "$case_id" != "install-clean" ]]; then
        mkdir -p "$current_app/Contents/MacOS" "$current_data"
        if [[ "$case_id" == "already-installed" ]]; then
            printf 'candidate-app\n' >"$current_app/Contents/MacOS/gcrdings"
        else
            printf 'old-app\n' >"$current_app/Contents/MacOS/gcrdings"
        fi
        sqlite3 "$current_data/meeting_minutes.sqlite" 'CREATE TABLE state(value TEXT); INSERT INTO state VALUES("preserve");'
    fi
    snapshot_state "$work" "$current_app" "$current_data"

    if [[ "$case_id" == "insufficient-disk" ]]; then
        available_kib=1
        required_kib=2
        [[ "$available_kib" -lt "$required_kib" ]] || fail "insufficient_disk_fixture_invalid"
        restore_state "$work" "$current_app" "$current_data"
        echo "self-test: insufficient-disk passed"
        return
    fi

    staged="$fixture/current/.gcrdings-install-$case_id"
    copy_tree "$candidate_app" "$staged"
    remove_entry "$current_app"
    mv "$staged" "$current_app"
    if [[ -e "$current_data/meeting_minutes.sqlite" ]]; then
        sqlite3 "$current_data/meeting_minutes.sqlite" 'UPDATE state SET value="candidate";'
    else
        mkdir -p "$current_data"
        sqlite3 "$current_data/meeting_minutes.sqlite" 'CREATE TABLE state(value TEXT); INSERT INTO state VALUES("candidate");'
    fi

    if [[ "$case_id" == "failed-launch" ]]; then
        failure_status=0
        (exit 23) || failure_status=$?
        [[ "$failure_status" == "23" ]] || fail "failed_launch_fixture_invalid"
    fi
    if [[ "$case_id" == "migration-failure" ]]; then
        if sqlite3 "$current_data/meeting_minutes.sqlite" 'THIS IS NOT SQL' >/dev/null 2>&1; then
            fail "migration_failure_fixture_invalid"
        fi
    fi

    if [[ "$case_id" == "interrupted-rollback" ]]; then
        printf '%s\n' "$work" >"$fixture/pending"
        restore_state "$(<"$fixture/pending")" "$current_app" "$current_data"
        rm -f "$fixture/pending"
    else
        restore_state "$work" "$current_app" "$current_data"
    fi
    echo "self-test: $case_id passed"
}

self_test() {
    local fixture
    local case_id
    local case_root
    local alias_root
    self_test_mode=1
    fixture=$(mktemp -d "${TMPDIR:-/tmp}/gcrdings-install-self-test.XXXXXX")
    trap "chmod -R u+w '$fixture' 2>/dev/null || true; remove_entry '$fixture'" EXIT INT TERM
    for case_id in install-clean install-existing failed-launch migration-failure insufficient-disk interrupted-rollback already-installed; do
        case_root="$fixture/$case_id"
        mkdir -p "$case_root"
        run_fixture_case "$case_id" "$case_root"
    done
    alias_root="$fixture/alias-rejection"
    mkdir -p "$alias_root/outside" "$alias_root/tree"
    ln -s "$alias_root/outside" "$alias_root/tree/escape"
    if (require_regular_tree "$alias_root/tree") >/dev/null 2>&1; then
        fail "alias_self_test_did_not_fail"
    fi
    echo "self-test: alias-rejection passed"

    case_root="$fixture/package-binding"
    mkdir -p "$case_root"
    printf 'package\n' >"$case_root/app"
    package_hash=$(shasum -a 256 "$case_root/app" | awk '{print $1}')
    printf 'tampered\n' >"$case_root/app"
    [[ "$(shasum -a 256 "$case_root/app" | awk '{print $1}')" != "$package_hash" ]] ||
        fail "package_binding_fixture_invalid"
    echo "self-test: package-binding passed"

    case_root="$fixture/sqlite-quiescence"
    mkdir -p "$case_root"
    sqlite3 "$case_root/state.sqlite" 'PRAGMA journal_mode=WAL; CREATE TABLE state(value TEXT); INSERT INTO state VALUES("preserve");' >/dev/null
    quiesce_databases "$case_root"
    integrity_check_databases "$case_root"
    echo "self-test: sqlite-quiescence passed"

    case_root="$fixture/snapshot-sidecar-cleanup"
    snapshot_app="$case_root/current/gcrdings.app"
    snapshot_data="$case_root/current/data"
    snapshot_work="$case_root/work"
    mkdir -p "$snapshot_app/Contents/MacOS" "$snapshot_data" "$snapshot_work"
    printf 'app\n' >"$snapshot_app/Contents/MacOS/gcrdings"
    printf 'transaction.snapshot\n' >"$snapshot_work/transaction-id"
    write_transaction_journal "$snapshot_work" transaction.snapshot "$(printf '4%.0s' {1..40})" \
        "$snapshot_app" "$snapshot_data" "$snapshot_work/qualification.keychain-db"
    sqlite3 "$snapshot_data/state.sqlite" \
        'PRAGMA journal_mode=WAL; CREATE TABLE state(value TEXT); INSERT INTO state VALUES("preserve");' >/dev/null
    snapshot_state "$snapshot_work" "$snapshot_app" "$snapshot_data"
    [[ -z "$(find "$snapshot_work/snapshot" -type f \( -name '*-wal' -o -name '*-shm' \) -print -quit)" ]] ||
        fail "snapshot_sqlite_sidecar_retained"
    restore_state "$snapshot_work" "$snapshot_app" "$snapshot_data"
    echo "self-test: snapshot-sidecar-cleanup passed"

    case_root="$fixture/sqlite-writer-busy"
    mkdir -p "$case_root"
    sqlite3 "$case_root/state.sqlite" 'PRAGMA journal_mode=WAL; CREATE TABLE state(value TEXT); INSERT INTO state VALUES("preserve");' >/dev/null
    mkfifo "$case_root/writer-input"
    sqlite3 "$case_root/state.sqlite" <"$case_root/writer-input" >"$case_root/writer-output" 2>&1 &
    sqlite_writer_pid=$!
    exec 9>"$case_root/writer-input"
    printf '.timeout 0\nPRAGMA journal_mode=WAL;\nBEGIN IMMEDIATE;\nINSERT INTO state VALUES("held");\n.shell /usr/bin/touch %s\n' \
        "$case_root/writer-ready" >&9
    for _ in {1..100}; do
        [[ -f "$case_root/writer-ready" ]] && break
        sleep 0.01
    done
    [[ -f "$case_root/writer-ready" ]] || fail "sqlite_writer_fixture_not_ready"
    if (quiesce_databases "$case_root") >/dev/null 2>&1; then
        fail "wal_checkpoint_busy_was_accepted"
    fi
    exec 9>&-
    wait "$sqlite_writer_pid"
    rm -f "$case_root/writer-input" "$case_root/writer-output" "$case_root/writer-ready"
    quiesce_databases "$case_root"
    echo "self-test: sqlite-writer-busy passed"

    case_root="$fixture/two-process-interruption"
    mkdir -p "$case_root"
    (
        mkdir "$case_root/operation.lock"
        printf '%s\n' "$$" >"$case_root/operation.lock/pid"
        : >"$case_root/holder-ready"
        while [[ ! -f "$case_root/release-holder" ]]; do sleep 0.01; done
        remove_entry "$case_root/operation.lock"
    ) &
    holder_pid=$!
    while [[ ! -f "$case_root/holder-ready" ]]; do sleep 0.01; done
    if (acquire_operation_lock "$case_root") 2>/dev/null; then
        fail "concurrent_operation_fixture_invalid"
    fi
    : >"$case_root/release-holder"
    wait "$holder_pid"
    acquire_operation_lock "$case_root"
    release_operation_lock "$case_root"
    echo "self-test: two-process-interruption passed"

    case_root="$fixture/sigkill-recovery"
    crash_app="$case_root/current/gcrdings.app"
    crash_data="$case_root/current/data"
    crash_journal="$case_root/journal"
    crash_transaction="$crash_journal/transaction.crash"
    crash_pending="$crash_journal/rollback.pending"
    crash_candidate=$(printf '1%.0s' {1..40})
    mkdir -p "$crash_app/Contents/MacOS" "$crash_data" "$crash_transaction"
    printf 'old-app\n' >"$crash_app/Contents/MacOS/gcrdings"
    sqlite3 "$crash_data/meeting_minutes.sqlite" 'CREATE TABLE state(value TEXT); INSERT INTO state VALUES("preserve");'
    printf 'transaction.crash\n' >"$crash_transaction/transaction-id"
    write_transaction_journal "$crash_transaction" transaction.crash "$crash_candidate" \
        "$crash_app" "$crash_data" "$crash_transaction/qualification.keychain-db"
    snapshot_state "$crash_transaction" "$crash_app" "$crash_data"
    crash_app_before=$(<"$crash_transaction/app-before.sha256")
    crash_data_before=$(<"$crash_transaction/data-before.sha256")
    jq -n --arg candidate_commit "$crash_candidate" \
        '{schema_version: 1, transaction_id: "transaction.crash", candidate_commit: $candidate_commit}' \
        >"$crash_pending"
    sync_path "$crash_pending"
    (
        /usr/bin/perl -MPOSIX -e 'POSIX::setsid() >= 0 or die "setsid"; exec @ARGV or die "exec"' \
            /bin/sleep 60 &
        crash_group=$!
        printf '%s\n' "$crash_group" >"$crash_transaction/process-group"
        sync_path "$crash_transaction/process-group"
        record_process_identity "$crash_transaction" "$crash_group" /bin/sleep
        printf 'candidate-app\n' >"$crash_app/Contents/MacOS/gcrdings"
        sqlite3 "$crash_data/meeting_minutes.sqlite" 'UPDATE state SET value="candidate";'
        crash_app_stage=$(jq -r '.paths.application_stage' "$crash_transaction/transaction.json")
        crash_app_displaced=$(jq -r '.paths.application_displaced' "$crash_transaction/transaction.json")
        copy_tree "$crash_app" "$crash_app_stage"
        mv "$crash_app" "$crash_app_displaced"
        sync_path "$crash_app_displaced"
        : >"$case_root/worker-ready"
        sync_path "$case_root/worker-ready"
        /bin/sh -c 'kill -STOP "$PPID"'
    ) &
    crash_worker=$!
    for _ in {1..100}; do
        [[ -f "$case_root/worker-ready" ]] && break
        sleep 0.01
    done
    [[ -f "$case_root/worker-ready" ]] || fail "sigkill_worker_not_ready"
    kill -KILL "$crash_worker"
    wait "$crash_worker" 2>/dev/null || true
    recovery_occurred=0
    recovered_candidate=
    recover_pending_transaction "$crash_pending" "$crash_journal" "$crash_app" "$crash_data"
    [[ "$recovery_occurred" == "1" && "$recovered_candidate" == "$crash_candidate" ]] ||
        fail "sigkill_recovery_candidate_mismatch"
    [[ "$(tree_content_sha256 "$crash_app")" == "$crash_app_before" ]] ||
        fail "sigkill_application_restore_mismatch"
    [[ "$(tree_content_sha256 "$crash_data")" == "$crash_data_before" ]] ||
        fail "sigkill_data_restore_mismatch"
    [[ ! -e "$crash_pending" && ! -e "$crash_transaction" ]] || fail "sigkill_journal_not_reconciled"
    echo "self-test: sigkill-recovery passed"

    case_root="$fixture/reused-process-group"
    mkdir -p "$case_root"
    /usr/bin/perl -MPOSIX -e 'POSIX::setsid() >= 0 or die "setsid"; exec @ARGV or die "exec"' \
        /bin/sleep 60 &
    reused_group=$!
    for _ in {1..50}; do
        [[ "$(ps -o pgid= -p "$reused_group" | tr -d ' ')" == "$reused_group" ]] && break
        sleep 0.02
    done
    [[ "$(ps -o pgid= -p "$reused_group" | tr -d ' ')" == "$reused_group" ]] ||
        fail "reused_process_group_fixture_not_ready"
    printf '%s\n' "$reused_group" >"$case_root/process-group"
    jq -n --argjson process_group "$reused_group" \
        --arg executable "/not/the/recorded/candidate" --arg started_at "stale" \
        '{schema_version: 1, process_group: $process_group, executable: $executable, started_at: $started_at}' \
        >"$case_root/process-identity"
    if (terminate_recorded_process_group "$case_root") >"$case_root/termination.log" 2>&1; then accepted_reused_group=1; else accepted_reused_group=0; fi
    if kill -0 "$reused_group" 2>/dev/null; then reused_group_alive=1; else reused_group_alive=0; fi
    kill -TERM -- "-$reused_group"
    wait "$reused_group" 2>/dev/null || true
    [[ "$accepted_reused_group" == "0" ]] || fail "reused_process_group_was_accepted"
    [[ "$reused_group_alive" == "1" ]] || fail "reused_process_group_was_signaled"
    grep -Fxq "qualify-local-adhoc-install: process_identity_mismatch" "$case_root/termination.log" ||
        fail "reused_process_group_did_not_reach_identity_comparison"
    echo "self-test: reused-process-group passed"

    case_root="$fixture/recovery-candidate-binding"
    binding_app="$case_root/current/gcrdings.app"
    binding_data="$case_root/current/data"
    binding_journal="$case_root/journal"
    binding_transaction="$binding_journal/transaction.binding"
    binding_pending="$binding_journal/rollback.pending"
    mkdir -p "$binding_app/Contents/MacOS" "$binding_data" "$binding_transaction"
    printf 'old-app\n' >"$binding_app/Contents/MacOS/gcrdings"
    printf 'transaction.binding\n' >"$binding_transaction/transaction-id"
    write_transaction_journal "$binding_transaction" transaction.binding "$(printf '2%.0s' {1..40})" \
        "$binding_app" "$binding_data" "$binding_transaction/qualification.keychain-db"
    snapshot_state "$binding_transaction" "$binding_app" "$binding_data"
    jq -n --arg candidate_commit "$(printf '3%.0s' {1..40})" \
        '{schema_version: 1, transaction_id: "transaction.binding", candidate_commit: $candidate_commit}' \
        >"$binding_pending"
    if (recover_pending_transaction "$binding_pending" "$binding_journal" "$binding_app" "$binding_data") \
        >/dev/null 2>&1; then
        fail "recovery_candidate_mismatch_was_accepted"
    fi
    echo "self-test: recovery-candidate-binding passed"

    case_root="$fixture/symlink-ancestor"
    mkdir -p "$case_root/real"
    ln -s "$case_root/real" "$case_root/alias"
    if (reject_symlink_ancestors "$case_root/alias/child") >/dev/null 2>&1; then
        fail "symlink_ancestor_was_accepted"
    fi
    ln -s "$case_root/missing" "$case_root/broken"
    if (reject_symlink_ancestors "$case_root/broken/child") >/dev/null 2>&1; then
        fail "broken_symlink_ancestor_was_accepted"
    fi
    echo "self-test: symlink-ancestor passed"

    case_root="$fixture/measured-cleanup"
    mkdir -p "$case_root/.gcrdings-stage"
    remove_entry "$case_root/.gcrdings-stage"
    cleanup_count=$(/usr/bin/find "$case_root" -mindepth 1 -maxdepth 1 -print | wc -l | tr -d ' ')
    [[ "$cleanup_count" == "0" ]] || fail "measured_cleanup_fixture_invalid"
    echo "self-test: measured-cleanup passed"
    echo "qualify-local-adhoc-install: self-test passed"
}

[[ "$mode" != "--self-test" ]] || {
    [[ $# -eq 0 ]] || fail "self_test_takes_no_arguments"
    self_test
    exit 0
}

case "$mode" in
    install-clean|install-existing|rollback|rollback-interrupted) ;;
    *) fail "expected install-clean, install-existing, rollback, rollback-interrupted, or --self-test" ;;
esac

dmg=
application=
package_manifest=
candidate=
manifest=
while [[ $# -gt 0 ]]; do
    case "$1" in
        --dmg) dmg=${2:-}; shift 2 ;;
        --application) application=${2:-}; shift 2 ;;
        --package-manifest) package_manifest=${2:-}; shift 2 ;;
        --candidate) candidate=${2:-}; shift 2 ;;
        --manifest) manifest=${2:-}; shift 2 ;;
        *) fail "unknown_argument" ;;
    esac
done

[[ -f "$dmg" && ! -L "$dmg" ]] || fail "installer_missing_or_aliased"
[[ -d "$application" && ! -L "$application" ]] || fail "application_missing_or_aliased"
[[ -f "$package_manifest" && ! -L "$package_manifest" ]] || fail "package_manifest_missing_or_aliased"
[[ "$candidate" =~ ^[0-9a-f]{40}$ ]] || fail "invalid_candidate"
[[ -n "$manifest" && ! -L "$manifest" ]] || fail "invalid_manifest_destination"

app_target="$HOME/Applications/gcrdings.app"
data_target="$HOME/Library/Application Support/com.gcrdings.app"
journal_root="$HOME/Library/Application Support/com.gcrdings.install-qualification"
reject_symlink_ancestors "$HOME/Applications"
reject_symlink_ancestors "$app_target"
reject_symlink_ancestors "$HOME/Library/Application Support"
reject_symlink_ancestors "$data_target"
reject_symlink_ancestors "$journal_root"
mkdir -p "$journal_root"
chmod 700 "$journal_root"
acquire_operation_lock "$journal_root" || fail "qualification_already_running"
operation_locked=1
trap 'release_operation_lock "$journal_root"' EXIT INT TERM

pending="$journal_root/rollback.pending"
recovery_occurred=0
recovered_candidate=
recover_pending_transaction "$pending" "$journal_root" "$app_target" "$data_target"

if [[ "$mode" == "rollback-interrupted" ]]; then
    [[ "$recovery_occurred" == "1" && "$recovered_candidate" == "$candidate" ]] ||
        fail "rollback_interrupted_requires_recovered_candidate"
fi

[[ "$mode" != "install-clean" || (! -e "$app_target" && ! -e "$data_target") ]] ||
    fail "clean_install_requires_absent_state"
[[ "$mode" != "install-existing" || -e "$data_target" ]] ||
    fail "existing_install_requires_data"

GCRDINGS_RELEASE_CANDIDATE="$candidate" \
    "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/verify-release-package.sh" \
    "$application" "$dmg" --manifest "$package_manifest"

if pgrep -f -x "$app_target/Contents/MacOS/gcrdings" >/dev/null 2>&1; then
    pkill -TERM -f -x "$app_target/Contents/MacOS/gcrdings" || fail "existing_application_quiescence_failed"
    for _ in {1..50}; do
        pgrep -f -x "$app_target/Contents/MacOS/gcrdings" >/dev/null 2>&1 || break
        sleep 0.1
    done
    pgrep -f -x "$app_target/Contents/MacOS/gcrdings" >/dev/null 2>&1 &&
        fail "existing_application_quiescence_failed"
fi
quiesce_databases "$data_target"

baseline_mounted_count=$(hdiutil info | grep -F -c "$journal_root/transaction." || true)
baseline_candidate_process_count=$({ pgrep -f -x "$app_target/Contents/MacOS/gcrdings" 2>/dev/null || true; } | wc -l | tr -d ' ')
baseline_helper_process_count=$({ pgrep -f "$app_target/Contents/MacOS/(foundation-helper|ffmpeg)" 2>/dev/null || true; } | wc -l | tr -d ' ')
baseline_listener_count=$(
    { lsof -nP -iTCP -sTCP:LISTEN 2>/dev/null || true; } |
        grep -E -c 'gcrdings|foundation-helper' || true
)
baseline_temporary_keychain_count=$((
    $(security list-keychains -d user 2>/dev/null | grep -F -c "$journal_root" || true) +
    $(/usr/bin/find "$journal_root" -name 'qualification.keychain-db' -print 2>/dev/null | wc -l | tr -d ' ')
))
baseline_temporary_artifact_count=$(/usr/bin/find "$HOME/Applications" -maxdepth 1 \( -name '.gcrdings-install-*' -o -name '*.gcrdings-restore-*' -o -name '*.gcrdings-displaced-*' \) -print 2>/dev/null | wc -l | tr -d ' ')
baseline_journal_count=$(/usr/bin/find "$journal_root" -mindepth 1 -maxdepth 1 ! -name 'operation.lock' -print | wc -l | tr -d ' ')

available_kib=$(df -Pk "$journal_root" | awk 'NR==2 {print $4}')
required_kib=$(( $(stat -f '%z' "$dmg") / 1024 + 1048576 ))
[[ "$available_kib" -ge "$required_kib" ]] || fail "insufficient_disk"

work_root=$(mktemp -d "$journal_root/transaction.XXXXXX")
chmod 700 "$work_root"
work_name=$(basename "$work_root")
mount_root="$work_root/mount"
keychain="$work_root/qualification.keychain-db"
printf '%s\n' "$work_name" >"$work_root/transaction-id"
chmod 600 "$work_root/transaction-id"
write_transaction_journal "$work_root" "$work_name" "$candidate" "$app_target" "$data_target" "$keychain"
pending_stage="$journal_root/.rollback.pending.$$"
jq -n --arg transaction_id "$work_name" --arg candidate_commit "$candidate" \
    '{schema_version: 1, transaction_id: $transaction_id, candidate_commit: $candidate_commit}' >"$pending_stage"
chmod 600 "$pending_stage"
sync_path "$pending_stage"
mv "$pending_stage" "$pending"
sync_path "$pending"

keychain_password=$(/usr/bin/uuidgen | tr -d '-')
original_default="$work_root/original-default"
original_list="$work_root/original-list"
app_pid=
app_process_group=
mounted=0
keychain_changed=0
rollback_complete=0
transaction_cleaned=0
manifest_stage=

cleanup_actual() {
    local cleanup_ready=1
    if [[ -f "$work_root/process-group" && ! -L "$work_root/process-group" ]]; then
        if terminate_recorded_process_group "$work_root"; then
            [[ -z "$app_pid" ]] || wait "$app_pid" 2>/dev/null || true
            app_pid=
            app_process_group=
        else
            cleanup_ready=0
        fi
    fi
    if [[ "$mounted" == "1" ]]; then
        if hdiutil detach "$mount_root" >/dev/null 2>&1 || hdiutil detach -force "$mount_root" >/dev/null 2>&1; then
            mounted=0
        else
            cleanup_ready=0
        fi
    fi
    if [[ "$keychain_changed" == "1" ]]; then
        if restore_keychain_transaction "$work_root"; then
            keychain_changed=0
        else
            cleanup_ready=0
            echo "qualify-local-adhoc-install: keychain_restore_pending" >&2
        fi
    elif ! security delete-keychain "$keychain" >/dev/null 2>&1 && [[ -e "$keychain" ]]; then
        cleanup_ready=0
    fi
    if [[ "$rollback_complete" == "0" ]]; then
        if [[ -f "$work_root/snapshot-complete" && ! -L "$work_root/snapshot-complete" ]]; then
            if (restore_state "$work_root" "$app_target" "$data_target"); then
                rollback_complete=1
            else
                echo "qualify-local-adhoc-install: rollback_pending" >&2
            fi
        else
            rollback_complete=1
        fi
    fi
    if [[ "$transaction_cleaned" == "0" ]]; then
        if [[ "$rollback_complete" == "1" && "$cleanup_ready" == "1" ]]; then
            [[ -z "$manifest_stage" ]] || rm -f "$manifest_stage"
            remove_entry "$work_root"
            rm -f "$pending"
            transaction_cleaned=1
        else
            echo "qualify-local-adhoc-install: cleanup_pending" >&2
        fi
    fi
    if [[ "$operation_locked" == "1" ]]; then
        release_operation_lock "$journal_root"
        operation_locked=0
    fi
}
trap cleanup_actual EXIT INT TERM

snapshot_state "$work_root" "$app_target" "$data_target"
security default-keychain -d user >"$original_default"
security list-keychains -d user >"$original_list"
printf '%s\n' "$keychain" >"$work_root/keychain-path"
sync_path "$original_default"
sync_path "$original_list"
sync_path "$work_root/keychain-path"
security create-keychain -p "$keychain_password" "$keychain" >/dev/null
security unlock-keychain -p "$keychain_password" "$keychain" >/dev/null
keychain_changed=1
printf 'changed\n' >"$work_root/keychain-changed"
sync_path "$work_root/keychain-changed"
security list-keychains -d user -s "$keychain" >/dev/null
security default-keychain -d user -s "$keychain" >/dev/null

mkdir -p "$mount_root"
hdiutil attach -readonly -nobrowse -mountpoint "$mount_root" "$dmg" >/dev/null
mounted=1
candidate_app="$mount_root/gcrdings.app"
candidate_executable="$candidate_app/Contents/MacOS/gcrdings"
[[ -x "$candidate_executable" && ! -L "$candidate_app" ]] || fail "candidate_application_missing"
codesign --verify --deep --strict "$candidate_app" >/dev/null 2>&1 || fail "candidate_signature_invalid"
grep -a -q "gcrdings-build-commit:$candidate" "$candidate_executable" || fail "candidate_marker_mismatch"
require_regular_tree "$candidate_app"
[[ "$(tree_sha256 "$candidate_app")" == "$(tree_sha256 "$application")" ]] ||
    fail "mounted_application_hash_mismatch"

stage="$HOME/Applications/.gcrdings-install-$$"
mkdir -p "$HOME/Applications"
copy_tree "$candidate_app" "$stage"
restore_entry "$stage" "$app_target" 1 "$work_name" \
    "$(jq -r '.paths.application_stage' "$work_root/transaction.json")" \
    "$(jq -r '.paths.application_displaced' "$work_root/transaction.json")"
remove_entry "$stage"

/usr/bin/perl -MPOSIX -e 'POSIX::setsid() >= 0 or die "setsid"; exec @ARGV or die "exec"' \
    "$app_target/Contents/MacOS/gcrdings" >"$work_root/application.log" 2>&1 &
app_pid=$!
app_process_group=$app_pid
printf '%s\n' "$app_process_group" >"$work_root/process-group"
sync_path "$work_root/process-group"
record_process_identity "$work_root" "$app_process_group" "$app_target/Contents/MacOS/gcrdings"
sleep 2
kill -0 "$app_pid" 2>/dev/null || fail "candidate_launch_failed"
launched_group_members=$(pgrep -g "$app_process_group" 2>/dev/null | wc -l | tr -d ' ')
launched_group_listeners=$(
    { lsof -nP -a -g "$app_process_group" -iTCP -sTCP:LISTEN 2>/dev/null || true; } |
        tail -n +2 | wc -l | tr -d ' '
)
[[ "$launched_group_members" -ge 1 ]] || fail "candidate_process_group_missing"
stopped_pid=$app_pid
terminate_recorded_process_group "$work_root"
wait "$app_pid" || true
app_pid=
app_process_group=
if kill -0 "$stopped_pid" 2>/dev/null || lsof -nP -a -p "$stopped_pid" -iTCP -sTCP:LISTEN 2>/dev/null | grep -q .; then
    fail "candidate_process_cleanup_failed"
fi

restore_state "$work_root" "$app_target" "$data_target"
rollback_complete=1

snapshot_app_sha=$(<"$work_root/app-before.sha256")
snapshot_data_sha=$(<"$work_root/data-before.sha256")
restored_app_sha=$(tree_content_sha256 "$app_target")
restored_data_sha=$(tree_content_sha256 "$data_target")
keychain_configuration_sha=$(/bin/cat "$original_default" "$original_list" | shasum -a 256 | awk '{print $1}')
package_manifest_sha=$(shasum -a 256 "$package_manifest" | awk '{print $1}')
package_application_sha=$(tree_sha256 "$application")
package_application_manifest_sha=$(jq -r '.application_tree_sha256' "$package_manifest")
package_dmg_sha=$(shasum -a 256 "$dmg" | awk '{print $1}')
[[ "$package_application_manifest_sha" =~ ^[0-9a-f]{64}$ ]] || fail "package_manifest_application_hash_invalid"

hdiutil detach "$mount_root" >/dev/null
mounted=0
restore_keychain_transaction "$work_root" || fail "keychain_restore_failed"
keychain_changed=0
security default-keychain -d user >"$work_root/restored-default"
security list-keychains -d user >"$work_root/restored-list"
restored_keychain_configuration_sha=$(/bin/cat "$work_root/restored-default" "$work_root/restored-list" | shasum -a 256 | awk '{print $1}')
[[ "$restored_keychain_configuration_sha" == "$keychain_configuration_sha" ]] ||
    fail "keychain_configuration_restore_mismatch"

remove_entry "$work_root"
rm -f "$pending"
transaction_cleaned=1

mounted_count=$(hdiutil info | grep -F -c "$journal_root/transaction." || true)
candidate_process_count=$({ pgrep -f -x "$app_target/Contents/MacOS/gcrdings" 2>/dev/null || true; } | wc -l | tr -d ' ')
helper_process_count=$({ pgrep -f "$app_target/Contents/MacOS/(foundation-helper|ffmpeg)" 2>/dev/null || true; } | wc -l | tr -d ' ')
listener_count=$(lsof -nP -iTCP -sTCP:LISTEN 2>/dev/null | grep -E -c 'gcrdings|foundation-helper' || true)
temporary_keychain_count=$((
    $(security list-keychains -d user 2>/dev/null | grep -F -c "$journal_root" || true) +
    $(/usr/bin/find "$journal_root" -name 'qualification.keychain-db' -print 2>/dev/null | wc -l | tr -d ' ')
))
temporary_artifact_count=$(/usr/bin/find "$HOME/Applications" -maxdepth 1 \( -name '.gcrdings-install-*' -o -name '*.gcrdings-restore-*' -o -name '*.gcrdings-displaced-*' \) -print 2>/dev/null | wc -l | tr -d ' ')
journal_count=$(/usr/bin/find "$journal_root" -mindepth 1 -maxdepth 1 ! -name 'operation.lock' -print | wc -l | tr -d ' ')
[[ "$mounted_count" == "$baseline_mounted_count" &&
   "$candidate_process_count" == "$baseline_candidate_process_count" &&
   "$helper_process_count" == "$baseline_helper_process_count" &&
   "$listener_count" == "$baseline_listener_count" &&
   "$temporary_keychain_count" == "$baseline_temporary_keychain_count" &&
   "$temporary_artifact_count" == "$baseline_temporary_artifact_count" &&
   "$journal_count" == "$baseline_journal_count" ]] || fail "measured_cleanup_incomplete"

case "$mode" in
    install-clean) result_code=clean_install_launched ;;
    install-existing) result_code=existing_data_preserved ;;
    rollback) result_code=snapshot_restored_exactly ;;
    rollback-interrupted) result_code=rollback_retry_completed ;;
esac

manifest_parent=$(dirname "$manifest")
reject_symlink_ancestors "$manifest_parent"
mkdir -p "$manifest_parent"
[[ ! -L "$manifest_parent" ]] || fail "invalid_manifest_destination"
manifest_stage=$(mktemp "$manifest_parent/.gcrdings-rollback-manifest.XXXXXX")
chmod 600 "$manifest_stage"
if [[ "$recovery_occurred" == "1" ]]; then
    recovery_candidate_json="\"$recovered_candidate\""
    recovery_occurred_json=true
else
    recovery_candidate_json=null
    recovery_occurred_json=false
fi
cat >"$manifest_stage" <<JSON
{
  "schema_version": 1,
  "distribution_mode": "local_adhoc",
  "candidate_commit": "$candidate",
  "package": {
    "manifest_sha256": "$package_manifest_sha",
    "application_sha256": "$package_application_sha",
    "application_manifest_sha256": "$package_application_manifest_sha",
    "dmg_sha256": "$package_dmg_sha"
  },
  "case": "$mode",
  "status": "passed",
  "code": "$result_code",
  "recovery": {
    "occurred": $recovery_occurred_json,
    "candidate_commit": $recovery_candidate_json
  },
  "snapshot": {
    "application_sha256": "$snapshot_app_sha",
    "data_sha256": "$snapshot_data_sha",
    "database_integrity": "ok",
    "keychain_configuration_sha256": "$keychain_configuration_sha"
  },
  "restored": {
    "application_sha256": "$restored_app_sha",
    "data_sha256": "$restored_data_sha",
    "database_integrity": "ok",
    "keychain_configuration_sha256": "$restored_keychain_configuration_sha"
  },
  "cleanup": {
    "mounted_images": { "before": $baseline_mounted_count, "after": $mounted_count },
    "candidate_processes": { "before": $baseline_candidate_process_count, "after": $candidate_process_count },
    "helper_processes": { "before": $baseline_helper_process_count, "after": $helper_process_count },
    "listeners": { "before": $baseline_listener_count, "after": $listener_count },
    "temporary_keychains": { "before": $baseline_temporary_keychain_count, "after": $temporary_keychain_count },
    "temporary_artifacts": { "before": $baseline_temporary_artifact_count, "after": $temporary_artifact_count },
    "pending_journals": { "before": $baseline_journal_count, "after": $journal_count },
    "process_group_members_observed": $launched_group_members,
    "process_group_listeners_observed": $launched_group_listeners
  }
}
JSON
mv -f "$manifest_stage" "$manifest"
manifest_stage=

echo "qualify-local-adhoc-install: $result_code"
