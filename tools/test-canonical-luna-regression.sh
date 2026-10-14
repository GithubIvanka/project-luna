#!/usr/bin/env bash
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# The default entrypoint owns exactly two managed source-image slots.
if [ "${LUNA_CANONICAL_MANAGED_RUN:-0}" != 1 ] && [ -z "${LUNA_CANONICAL_REGRESSION_ROOT:-}" ]; then
    exec python3 "$REPO_ROOT/tools/canonical-luna-workbench.py" run build
fi
RUN_ROOT="${LUNA_CANONICAL_REGRESSION_ROOT:?explicit regression evidence root required}"
export LUNA_CANONICAL_INPUT_ROOT="${LUNA_CANONICAL_INPUT_ROOT:-$REPO_ROOT/dist/canonical-inputs}"
if [ -e "$RUN_ROOT" ]; then
    [ "${LUNA_CANONICAL_MANAGED_RUN:-0}" = 1 ] && [ -f "$RUN_ROOT/resource-patch.json" ] && [ "$(find "$RUN_ROOT" -mindepth 1 -maxdepth 1 | wc -l)" -eq 1 ] || {
        echo "regression root already exists: $RUN_ROOT" >&2; exit 1;
    }
fi
mkdir -p "$RUN_ROOT/logs"
printf 'Project Luna generated canonical evidence v1\n' > "$RUN_ROOT/.luna-canonical-evidence-v1"
run() {
    local name="$1"
    shift
    echo "==> $name"
    if "$@" >"$RUN_ROOT/logs/$name.log" 2>&1; then
        printf '%s PASS\n' "$name" | tee -a "$RUN_ROOT/results.txt"
    else
        printf '%s FAIL\n' "$name" | tee -a "$RUN_ROOT/results.txt"
        tail -60 "$RUN_ROOT/logs/$name.log" >&2
        return 1
    fi
}
remove_disposable_disks() {
    [ "${LUNA_CANONICAL_KEEP_TEST_DISKS:-0}" = 1 ] && return 0
    rm -f "$1"/*.qcow2 "$1"/*.img "$1/image"/*.img
    if [ -d "$1/image" ]; then
        python3 "$REPO_ROOT/tools/canonical-luna-artifacts.py" cleanup-build "$1/image"
    fi
}
run inputs bash "$REPO_ROOT/tools/prepare-canonical-luna-inputs.sh" --verify
run input-negative-checks python3 "$REPO_ROOT/tools/test-canonical-luna-inputs.py"
run artifact-checks python3 "$REPO_ROOT/tools/test-canonical-luna-artifacts.py"
run state-runtime cargo test --manifest-path "$REPO_ROOT/Cargo.toml" -p luna-state -p luna-system-manager -p luna-system-runtime
run app-runtime cargo test --manifest-path "$REPO_ROOT/Cargo.toml" --locked --offline --target x86_64-unknown-linux-musl -p luna-app-runtime -p luna-system-runtime -p luna-user-session -p luna-cli -p luna-app-manager
run init cargo test --manifest-path "$REPO_ROOT/components/core/luna-init/Cargo.toml"
export LUNA_CANONICAL_OUTPUT_ROOT="${LUNA_CANONICAL_OUTPUT_ROOT:-$RUN_ROOT/image}"
export LUNA_CANONICAL_IMAGE="${LUNA_CANONICAL_IMAGE:-$LUNA_CANONICAL_OUTPUT_ROOT/luna-0.1.0.img}"
if [ "${LUNA_CANONICAL_SKIP_BUILD:-0}" = 1 ]; then
    run build sha256sum -c "$LUNA_CANONICAL_IMAGE.sha256"
else
    run build bash "$REPO_ROOT/tools/build-canonical-luna-disk-image.sh"
fi
run structure bash "$REPO_ROOT/tools/test-canonical-luna-disk-image.sh"
export LUNA_CANONICAL_TEST_ROOT="$RUN_ROOT/production"
run production bash "$REPO_ROOT/tools/test-canonical-luna-production-ovmf.sh"
remove_disposable_disks "$LUNA_CANONICAL_TEST_ROOT"
export LUNA_CANONICAL_TEST_ROOT="$RUN_ROOT/display"
run display bash "$REPO_ROOT/tools/test-canonical-luna-boot-display-ovmf.sh"
export LUNA_CANONICAL_TEST_ROOT="$RUN_ROOT/alpha-design"
run alpha-design python3 "$REPO_ROOT/tools/test-canonical-luna-alpha-design.py"
export LUNA_CANONICAL_TEST_ROOT="$RUN_ROOT/bundle-launch"
run bundle-launch python3 "$REPO_ROOT/tools/test-canonical-luna-bundle-launch.py"
for iteration in 1 2 3; do
    export LUNA_CANONICAL_TEST_ROOT="$RUN_ROOT/login-$iteration"
    export LUNA_CANONICAL_MONITOR_PATH="/tmp/luna-regression-$$.sock"
    run "login-$iteration" bash "$REPO_ROOT/tools/test-canonical-luna-login-ovmf.sh"
    remove_disposable_disks "$LUNA_CANONICAL_TEST_ROOT"
done
for action in logout reboot shutdown; do
    export LUNA_CANONICAL_TEST_ROOT="$RUN_ROOT/$action"
    run "$action" bash "$REPO_ROOT/tools/test-canonical-luna-lifecycle-ovmf.sh" "$action"
    remove_disposable_disks "$LUNA_CANONICAL_TEST_ROOT"
done
sha256sum -c "$LUNA_CANONICAL_IMAGE.sha256" > "$RUN_ROOT/logs/source-image-checksum.log"
printf 'Canonical Luna regression: PASS\n' | tee -a "$RUN_ROOT/results.txt"
echo "Evidence: $RUN_ROOT"
