#!/usr/bin/env bash
set -Eeuo pipefail

readonly SERVICE_NAME="wifimic-server"
readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly DEFAULT_BINARY_PATH="${HOME}/.local/bin/wifimic_server"
readonly DEFAULT_UNIT_PATH="${XDG_CONFIG_HOME:-${HOME}/.config}/systemd/user/wifimic-server.service"
readonly UPDATE_TIMEOUT_SECONDS="${WIFIMIC_UPDATE_TIMEOUT_SECONDS:-300}"
readonly HEALTH_TIMEOUT_SECONDS="${WIFIMIC_HEALTH_TIMEOUT_SECONDS:-45}"
readonly SMOKE_HOST="${WIFIMIC_SMOKE_HOST:-127.0.0.1}"
readonly SMOKE_PORT="${WIFIMIC_SMOKE_PORT:-6902}"
readonly SMOKE_BIND_ADDRESS="${WIFIMIC_SMOKE_BIND_ADDRESS:-}"
readonly SMOKE_HELPER="${WIFIMIC_CONTROL_SMOKE_HELPER:-}"

REPO_ROOT=""
REVISION=""
RESOLVED_REVISION=""
BINARY_PATH="${WIFIMIC_SERVER_BINARY:-${DEFAULT_BINARY_PATH}}"
UNIT_PATH="${WIFIMIC_SERVER_UNIT:-${DEFAULT_UNIT_PATH}}"
TIMEOUT_BIN=""
SYSTEMCTL_BIN=""
TXN_DIR=""
WORKTREE_DIR=""
OPERATION_LOG=""
SMOKE_OUTPUT=""
PRIOR_BINARY=""
PRIOR_UNIT=""
PRIOR_METADATA=""
MUTATION_STARTED=0
ROLLBACK_ATTEMPTED=0

die() {
    printf 'wifimic server update aborted: %s\n' "$1" >&2
    exit 1
}

validate_positive_integer() {
    local name="$1"
    local value="$2"
    [[ "$value" =~ ^[1-9][0-9]*$ ]] || die "$name must be a positive integer"
}

redacted_log_tail() {
    [[ -f "$OPERATION_LOG" ]] || return 0
    sed -E \
        -e 's#(https?://)([^/@[:space:]]+):([^/@[:space:]]+)@#\1<redacted>@#g' \
        -e 's#([Pp]assword|[Tt]oken|[Aa]uthorization)[=:][^[:space:]]+#\1=<redacted>#g' \
        "$OPERATION_LOG" | tail -c 4000 >&2 || true
}

run_bounded() {
    local seconds="$1"
    local label="$2"
    shift 2

    printf '\n[%s]\n' "$label" >>"$OPERATION_LOG"
    "$TIMEOUT_BIN" --signal=TERM --kill-after=5s "${seconds}s" "$@" >>"$OPERATION_LOG" 2>&1
}

run_bounded_or_die() {
    local seconds="$1"
    local label="$2"
    shift 2
    if ! run_bounded "$seconds" "$label" "$@"; then
        redacted_log_tail
        die "$label failed or timed out"
    fi
}

run_control_smoke_or_die() {
    local seconds="$1"
    local label="$2"
    shift 2

    printf '\n[%s]\n' "$label" >>"$OPERATION_LOG"
    if ! "$TIMEOUT_BIN" --signal=TERM --kill-after=5s "${seconds}s" "$@" \
        >"$SMOKE_OUTPUT" 2>>"$OPERATION_LOG"; then
        cat "$SMOKE_OUTPUT" >>"$OPERATION_LOG"
        redacted_log_tail
        die "$label failed or timed out"
    fi
    cat "$SMOKE_OUTPUT" >>"$OPERATION_LOG"
    grep -Fqx -- 'wifimic-control-smoke: PASS' "$SMOKE_OUTPUT" || {
        redacted_log_tail
        die "$label did not prove a complete control-session Ack exchange"
    }
}

sha256_of() {
    local digest
    digest="$(sha256sum -- "$1")" || return 1
    printf '%s\n' "${digest%% *}"
}

atomic_replace() {
    local source="$1"
    local destination="$2"
    local destination_dir
    local temporary

    destination_dir="$(dirname -- "$destination")"
    temporary="$(mktemp "${destination_dir}/.wifimic-server-update.XXXXXX")"
    if ! cp --preserve=mode,timestamps -- "$source" "$temporary"; then
        rm -f -- "$temporary"
        return 1
    fi
    if ! mv -f -- "$temporary" "$destination"; then
        rm -f -- "$temporary"
        return 1
    fi
}

service_is_active() {
    local state
    state="$(
        "$TIMEOUT_BIN" --signal=TERM --kill-after=2s 10s \
            "$SYSTEMCTL_BIN" --user is-active "$SERVICE_NAME" 2>>"$OPERATION_LOG" || true
    )"
    [[ "${state//$'\r'/}" == active ]]
}

wait_for_service_active() {
    local seconds="$1"
    local deadline=$((SECONDS + seconds))

    while (( SECONDS < deadline )); do
        if service_is_active; then
            return 0
        fi
        sleep 1
    done
    return 1
}

restore_service() {
    local restart_ok=0

    if [[ -f "$PRIOR_BINARY" ]]; then
        atomic_replace "$PRIOR_BINARY" "$BINARY_PATH" || {
            printf 'rollback: prior binary restore failed\n' >&2
            redacted_log_tail
            restart_ok=1
        }
    fi
    if [[ -f "$PRIOR_UNIT" ]]; then
        atomic_replace "$PRIOR_UNIT" "$UNIT_PATH" || {
            printf 'rollback: prior user unit restore failed\n' >&2
            redacted_log_tail
            restart_ok=1
        }
        run_bounded 15 'rollback daemon-reload' "$SYSTEMCTL_BIN" --user daemon-reload || restart_ok=1
    fi

    if ! run_bounded 20 'rollback service restart' "$SYSTEMCTL_BIN" --user restart "$SERVICE_NAME"; then
        restart_ok=1
        run_bounded 20 'rollback service start' "$SYSTEMCTL_BIN" --user start "$SERVICE_NAME" || true
    fi
    if ! wait_for_service_active "$HEALTH_TIMEOUT_SECONDS"; then
        printf 'rollback: service did not become active\n' >&2
        redacted_log_tail
        restart_ok=1
    fi

    return "$restart_ok"
}

on_exit() {
    local status="$?"
    trap - EXIT INT TERM HUP

    if (( status != 0 && MUTATION_STARTED == 1 && ROLLBACK_ATTEMPTED == 0 )); then
        ROLLBACK_ATTEMPTED=1
        printf 'wifimic server update failed; restoring the prior known-good service\n' >&2
        if ! restore_service; then
            printf 'wifimic server rollback could not prove an active service\n' >&2
            status=1
        else
            printf 'wifimic server rollback restored an active service\n' >&2
        fi
    fi

    if [[ -n "$WORKTREE_DIR" && -d "$WORKTREE_DIR" ]]; then
        git -C "$REPO_ROOT" worktree remove --force "$WORKTREE_DIR" >>"$OPERATION_LOG" 2>&1 || true
    fi
    if [[ -n "$REPO_ROOT" ]]; then
        git -C "$REPO_ROOT" worktree prune >>"$OPERATION_LOG" 2>&1 || true
    fi
    if [[ -n "$TXN_DIR" && -d "$TXN_DIR" ]]; then
        rm -rf -- "$TXN_DIR"
    fi
    exit "$status"
}

trap on_exit EXIT
trap 'exit 130' INT HUP
trap 'exit 143' TERM

[[ $# -eq 1 ]] || die 'exactly one explicit Git tag or commit is required'
REVISION="$1"
[[ -n "$REVISION" && "$REVISION" != -* && "$REVISION" != *[[:space:]]* ]] || \
    die 'revision must be a non-empty tag or commit, not an option or whitespace-containing value'
validate_positive_integer WIFIMIC_UPDATE_TIMEOUT_SECONDS "$UPDATE_TIMEOUT_SECONDS"
validate_positive_integer WIFIMIC_HEALTH_TIMEOUT_SECONDS "$HEALTH_TIMEOUT_SECONDS"
[[ "$SMOKE_PORT" =~ ^[1-9][0-9]{0,4}$ && "$SMOKE_PORT" -le 65535 ]] || die 'WIFIMIC_SMOKE_PORT must be a valid UDP port'
[[ "$BINARY_PATH" = /* && "$UNIT_PATH" = /* ]] || die 'binary and unit paths must be absolute'

TIMEOUT_BIN="$(command -v timeout || true)"
SYSTEMCTL_BIN="$(command -v systemctl || true)"
[[ -n "$TIMEOUT_BIN" ]] || die 'timeout is required for bounded update operations'
[[ -n "$SYSTEMCTL_BIN" ]] || die 'systemctl is required for the user service transaction'
command -v git >/dev/null 2>&1 || die 'git is required'
command -v cargo >/dev/null 2>&1 || die 'cargo is required to build wifimic_server'
command -v sha256sum >/dev/null 2>&1 || die 'sha256sum is required'
command -v file >/dev/null 2>&1 || die 'file is required to validate the built binary'
[[ -z "$SMOKE_HELPER" || -x "$SMOKE_HELPER" ]] || die 'WIFIMIC_CONTROL_SMOKE_HELPER is not executable'

REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null)" || \
    die 'the updater must run from a Git checkout'

if [[ -n "$(git -C "$REPO_ROOT" status --porcelain --untracked-files=all)" ]]; then
    die 'source checkout is dirty; refusing to update or mutate it'
fi

if [[ "$REVISION" =~ ^[0-9a-fA-F]{7,64}$ ]]; then
    if ! git -C "$REPO_ROOT" rev-parse --verify --quiet "${REVISION}^{commit}" >/dev/null; then
        mkdir -p "${TMPDIR:-/tmp}"
        TXN_DIR="$(mktemp -d "${TMPDIR:-/tmp}/wifimic-server-update.XXXXXX")"
        OPERATION_LOG="$TXN_DIR/operations.log"
        run_bounded_or_die "$UPDATE_TIMEOUT_SECONDS" 'fetch explicit commit' \
            git -C "$REPO_ROOT" fetch --no-tags origin "$REVISION"
    fi
    RESOLVED_REVISION="$(git -C "$REPO_ROOT" rev-parse --verify --quiet "${REVISION}^{commit}")" || \
        die "revision '$REVISION' is not a reachable commit"
else
    git -C "$REPO_ROOT" check-ref-format --allow-onelevel "refs/tags/$REVISION" >/dev/null 2>&1 || \
        die "revision '$REVISION' is not a valid Git tag or commit"
    if ! git -C "$REPO_ROOT" rev-parse --verify --quiet "refs/tags/$REVISION^{commit}" >/dev/null; then
        mkdir -p "${TMPDIR:-/tmp}"
        TXN_DIR="$(mktemp -d "${TMPDIR:-/tmp}/wifimic-server-update.XXXXXX")"
        OPERATION_LOG="$TXN_DIR/operations.log"
        run_bounded_or_die "$UPDATE_TIMEOUT_SECONDS" "fetch tag $REVISION" \
            git -C "$REPO_ROOT" fetch --no-tags origin "refs/tags/$REVISION:refs/tags/$REVISION"
    fi
    RESOLVED_REVISION="$(git -C "$REPO_ROOT" rev-parse --verify --quiet "refs/tags/$REVISION^{commit}")" || \
        die "tag '$REVISION' is not a reachable commit"
fi

if [[ -z "$TXN_DIR" ]]; then
    mkdir -p "${TMPDIR:-/tmp}"
    TXN_DIR="$(mktemp -d "${TMPDIR:-/tmp}/wifimic-server-update.XXXXXX")"
    OPERATION_LOG="$TXN_DIR/operations.log"
fi
mkdir -p "$TXN_DIR/prior"
SMOKE_OUTPUT="$TXN_DIR/control-smoke.out"
PRIOR_BINARY="$TXN_DIR/prior/wifimic_server"
PRIOR_UNIT="$TXN_DIR/prior/wifimic-server.service"
PRIOR_METADATA="$TXN_DIR/prior/metadata"
WORKTREE_DIR="$TXN_DIR/worktree"

[[ -f "$BINARY_PATH" && -x "$BINARY_PATH" ]] || die "prior binary is absent or not executable: $BINARY_PATH"
[[ -f "$UNIT_PATH" ]] || die "prior user unit is absent: $UNIT_PATH"
service_is_active || die "user service $SERVICE_NAME is not active before update"

prior_hash="$(sha256_of "$BINARY_PATH")" || die 'could not hash prior binary'
prior_file="$(file --brief -- "$BINARY_PATH")" || die 'could not inspect prior binary'
unit_hash="$(sha256_of "$UNIT_PATH")" || die 'could not hash prior user unit'
cp --preserve=mode,timestamps -- "$BINARY_PATH" "$PRIOR_BINARY" || die 'could not preserve prior binary'
cp --preserve=mode,timestamps -- "$UNIT_PATH" "$PRIOR_UNIT" || die 'could not preserve prior user unit'
printf 'binary_sha256=%s\nbinary_file=%s\nunit_sha256=%s\n' \
    "$prior_hash" "$prior_file" "$unit_hash" >"$PRIOR_METADATA"

run_bounded_or_die "$UPDATE_TIMEOUT_SECONDS" 'create detached staging worktree' \
    git -C "$REPO_ROOT" worktree add --detach "$WORKTREE_DIR" "$RESOLVED_REVISION"
run_bounded_or_die "$UPDATE_TIMEOUT_SECONDS" 'build wifimic_server' \
    bash -c 'cd -- "$1" && cargo build --release --bin wifimic_server' _ "$WORKTREE_DIR"

candidate_binary="$WORKTREE_DIR/target/release/wifimic_server"
[[ -f "$candidate_binary" && -x "$candidate_binary" ]] || die 'release build did not produce an executable wifimic_server'
candidate_file="$(file --brief -- "$candidate_binary")" || die 'could not inspect built wifimic_server'
[[ "$candidate_file" == *ELF* || "$candidate_file" == *executable* ]] || \
    die "built artifact is not an executable Linux binary: $candidate_file"
candidate_hash="$(sha256_of "$candidate_binary")" || die 'could not hash built wifimic_server'
staged_revision="$(git -C "$WORKTREE_DIR" rev-parse --verify HEAD)" || die 'could not read staged revision'
[[ "$staged_revision" == "$RESOLVED_REVISION" ]] || die 'staging worktree resolved to an unexpected revision'

MUTATION_STARTED=1
run_bounded_or_die 20 'stop user service before atomic swap' "$SYSTEMCTL_BIN" --user stop "$SERVICE_NAME"
atomic_replace "$candidate_binary" "$BINARY_PATH" || die 'atomic binary swap failed'
run_bounded_or_die 20 'restart user service with candidate' "$SYSTEMCTL_BIN" --user restart "$SERVICE_NAME"

if ! wait_for_service_active "$HEALTH_TIMEOUT_SECONDS"; then
    redacted_log_tail
    die 'candidate service did not become active within the health bound'
fi

if [[ -n "$SMOKE_HELPER" ]]; then
    run_control_smoke_or_die "$HEALTH_TIMEOUT_SECONDS" 'control-session smoke helper' \
        "$SMOKE_HELPER" "$SMOKE_HOST" "$SMOKE_PORT"
else
    python3_bin="$(command -v python3 || true)"
    [[ -n "$python3_bin" ]] || die 'python3 is required for the built-in control-session smoke'
    run_control_smoke_or_die "$HEALTH_TIMEOUT_SECONDS" 'control-session smoke' \
        "$python3_bin" - "$SMOKE_HOST" "$SMOKE_PORT" "$SMOKE_BIND_ADDRESS" <<'PY'
import socket
import sys
import time

host = sys.argv[1]
port = int(sys.argv[2])
bind_address = sys.argv[3]
session = (time.time_ns() // 1_000_000) & ((1 << 64) - 1)
if session == 0:
    session = 1

sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.settimeout(4)
if bind_address:
    sock.bind((bind_address, 0))

def exchange(tag):
    packet = bytes((tag, 1)) + session.to_bytes(8, "big")
    sock.sendto(packet, (host, port))
    response, _ = sock.recvfrom(64)
    expected = bytes((4, 1)) + session.to_bytes(8, "big") + bytes((tag,))
    if response != expected:
        raise RuntimeError(f"unexpected control Ack for tag {tag}: {response.hex()}")

try:
    exchange(1)
    exchange(2)
    exchange(3)
finally:
    sock.close()
print("wifimic-control-smoke: PASS")
PY
fi

printf 'wifimic server update succeeded\nrevision=%s\nprior_binary_sha256=%s\nprior_unit_sha256=%s\n' \
    "$staged_revision" "$prior_hash" "$unit_hash"
printf 'binary_sha256=%s\nbinary_file=%s\n' "$candidate_hash" "$candidate_file"
