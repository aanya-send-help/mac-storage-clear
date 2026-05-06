#!/usr/bin/env bash
#
# Mac Storage Clear — claim-orphaned-home.sh
#
# Transfer ownership of deleted-user home directories under /Users to your
# account, so the App Store build of Mac Storage Clear (with Full Disk Access
# and a user-granted folder bookmark) can scan and delete them.
#
# Quick install (recommended):
#
#   curl -fsSL https://mac-storage-clear.flek.ai/claim.sh | sudo bash
#
# Or, with an explicit target instead of the interactive picker:
#
#   sudo bash <(curl -fsSL https://mac-storage-clear.flek.ai/claim.sh) <username|path>
#
# Or, from a clone of the repo:
#
#   sudo ./scripts/claim-orphaned-home.sh [username|path]
#
# Source: https://github.com/aanya-send-help/mac-storage-clear/blob/main/scripts/claim-orphaned-home.sh

set -euo pipefail

# ── helpers ────────────────────────────────────────────────────────────────
err()  { printf "\033[31merror:\033[0m %s\n" "$*" >&2; }
note() { printf "\033[34m::\033[0m %s\n" "$*"; }
ok()   { printf "\033[32m✓\033[0m %s\n" "$*"; }

usage() {
    cat <<EOF
Usage:
  Interactive (recommended):
    curl -fsSL https://mac-storage-clear.flek.ai/claim.sh | sudo bash

  With an explicit target:
    sudo bash <(curl -fsSL https://mac-storage-clear.flek.ai/claim.sh) <username|path>
    sudo ./scripts/claim-orphaned-home.sh <username|path>

Refuses to act if:
  - not run as root via sudo
  - the target is outside /Users
  - a user account with the matching name still exists
  - the target is your own home directory
EOF
}

# ── must be root via sudo ──────────────────────────────────────────────────
if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
    err "this script needs root to chown files."
    echo
    usage
    exit 1
fi

INVOKER="${SUDO_USER:-}"
if [[ -z "$INVOKER" ]] || [[ "$INVOKER" == "root" ]]; then
    err "cannot determine the calling user."
    err "run via 'sudo' from a normal account, e.g.:"
    err "  curl -fsSL https://mac-storage-clear.flek.ai/claim.sh | sudo bash"
    exit 1
fi

INVOKER_GROUP="$(id -gn "$INVOKER")"
INVOKER_HOME="$(eval echo "~$INVOKER")"

# ── read from /dev/tty (stdin may be a curl pipe) ──────────────────────────
TTY=/dev/tty
if [[ ! -r "$TTY" ]] || [[ ! -w "$TTY" ]]; then
    err "no controlling terminal; can't prompt interactively."
    err "if running over SSH or in a non-interactive shell, use the explicit form:"
    err "  sudo bash <(curl -fsSL https://mac-storage-clear.flek.ai/claim.sh) <username>"
    exit 1
fi
exec 3< "$TTY"
ask() {
    local prompt="$1" __var
    printf "%s" "$prompt" >&2
    IFS= read -r __var <&3 || true
    printf "%s" "$__var"
}

# ── pick a sensible parallelism level (M-series performance cores) ─────────
detect_workers() {
    local n
    # Apple Silicon: prefer performance cores only.
    n="$(sysctl -n hw.perflevel0.physicalcpu 2>/dev/null || true)"
    if [[ -z "$n" ]]; then
        n="$(sysctl -n hw.physicalcpu 2>/dev/null || true)"
    fi
    if [[ -z "$n" ]] || [[ "$n" -lt 1 ]]; then
        n=8
    fi
    echo "$n"
}
WORKERS="${WORKERS:-$(detect_workers)}"

# ── safety checks shared by check_target + do_claim ────────────────────────
validate_target() {
    local target="$1"
    if [[ ! -d "$target" ]]; then
        err "$target does not exist or is not a directory."
        return 1
    fi

    local target_real
    target_real="$(cd "$target" && pwd -P)"
    case "$target_real" in
        /Users/*) ;;
        *) err "refusing to operate outside /Users (resolved $target_real)."; return 1 ;;
    esac
    case "$target_real" in
        /Users|/Users/Shared|/Users/Guest)
            err "refusing to operate on $target_real (shared/system path)."
            return 1
            ;;
    esac

    local basename_
    basename_="$(basename "$target_real")"
    if dscl . -read "/Users/$basename_" >/dev/null 2>&1; then
        err "a user account named '$basename_' still exists in DirectoryService."
        return 1
    fi

    if [[ "$target_real" == "$INVOKER_HOME" ]]; then
        err "$target_real is your own home; nothing to do."
        return 1
    fi

    printf '%s' "$target_real"
}

# ── check_target: dry-run. Lists known-protected paths quickly via -prune. ─
check_target() {
    local target_real
    target_real="$(validate_target "$1")" || return 1

    note "checking $target_real (read-only)..."

    # macOS exposes a few synthetic file-provider trees and code-signed app
    # bundles whose ownership chown can't touch even as root. We list those
    # so the user knows in advance. find -prune stops at the boundary so this
    # is fast — no full tree walk.
    local protected=()
    while IFS= read -r -d '' p; do
        protected+=("$p")
    done < <(
        find "$target_real" \( \
            -path '*/Library/CloudStorage' -o \
            -path '*/Library/CloudStorage/*' -o \
            -path '*/Library/Mobile Documents' -o \
            -path '*/Library/Mobile Documents/*' -o \
            -name '*.app' \
        \) -prune -print0 2>/dev/null
    )

    if [[ ${#protected[@]} -eq 0 ]]; then
        ok "no known-protected paths inside $target_real."
    else
        echo "  Found ${#protected[@]} known-protected path(s); chown will report errors on these:"
        local i=0
        for p in "${protected[@]}"; do
            echo "    $p"
            i=$((i + 1))
            [[ "$i" -ge 12 ]] && { echo "    ... (truncated, ${#protected[@]} total)"; break; }
        done
        echo
        echo "  These are either (a) virtual file-provider mounts (CloudStorage,"
        echo "  Mobile Documents) with no real local bytes, or (b) code-signed .app"
        echo "  bundles macOS protects even from root. They're harmless: the parent"
        echo "  directory will still be deletable once it's yours."
    fi
}

# ── do_claim: parallel chown with heartbeat + summarized errors ────────────
do_claim() {
    local target_real
    target_real="$(validate_target "$1")" || return 1

    note "claiming $target_real → $INVOKER:$INVOKER_GROUP (parallel: $WORKERS workers)"

    # Heartbeat so the user sees we're alive on multi-GB trees.
    local start=$SECONDS
    (
        sleep 10
        while true; do
            local e=$((SECONDS - start))
            printf "    ... still claiming, %d:%02d elapsed\n" "$((e / 60))" "$((e % 60))" >&2
            sleep 30
        done
    ) &
    local hb_pid=$!
    trap 'kill '"$hb_pid"' 2>/dev/null || true' RETURN INT TERM

    # Capture chown stderr so we can summarize protected-path errors instead
    # of dumping a wall of text.
    local err_log
    err_log="$(mktemp -t claim-errors.XXXXXX)"

    # Parallel chown:
    #   find -print0           NUL-delimited paths (handles spaces safely)
    #   xargs -0 -P N -n 500   N concurrent workers, 500 paths per chown invocation
    #   chown -h               don't follow symlinks
    local rc=0
    find "$target_real" -print0 2>/dev/null \
        | xargs -0 -P "$WORKERS" -n 500 chown -h "$INVOKER:$INVOKER_GROUP" 2>"$err_log" \
        || rc=$?

    kill "$hb_pid" 2>/dev/null || true
    wait "$hb_pid" 2>/dev/null || true
    trap - RETURN INT TERM

    local elapsed=$((SECONDS - start))
    local err_count
    err_count="$(grep -c '' "$err_log" 2>/dev/null || echo 0)"

    if [[ "$err_count" -eq 0 ]]; then
        ok "$target_real claimed in ${elapsed}s — no errors."
        rm -f "$err_log"
    else
        ok "$target_real chown finished in ${elapsed}s."
        printf "  \033[33m⚠\033[0m %s path(s) refused (protected app bundles or file-provider mounts).\n" "$err_count"
        echo "    Examples:"
        head -5 "$err_log" | sed 's/^/      /'
        if [[ "$err_count" -gt 5 ]]; then
            echo "      ... ($((err_count - 5)) more)"
        fi
        echo "    Full log: $err_log"
        echo "    Harmless. The parent directory can still be deleted by you (now its owner)."
    fi
}

# ── explicit target mode ──────────────────────────────────────────────────
if [[ $# -ge 1 ]]; then
    arg="$1"
    case "$arg" in
        -h|--help) usage; exit 0 ;;
        /*)        target="$arg" ;;
        *)         target="/Users/$arg" ;;
    esac

    echo "Selected (will become owned by $INVOKER:$INVOKER_GROUP):"
    echo "  $target"

    while true; do
        echo
        action="$(ask 'Action — [c]heck (preview), [d]o (chown now), [q]uit: ')"
        case "$action" in
            c|C|check) check_target "$target" || true ;;
            d|D|do)    do_claim "$target"; exit $? ;;
            q|Q|quit|'') note "aborted; nothing changed."; exit 0 ;;
            *) err "invalid choice: '$action'" ;;
        esac
    done
fi

# ── interactive picker mode ───────────────────────────────────────────────
note "scanning /Users (top-level only)..."
echo

CANDIDATES=()
DISPLAYS=()
idx=0
for entry in /Users/.[!.]* /Users/*; do
    [[ -d "$entry" ]] || continue
    name="$(basename "$entry")"

    # Skip the obvious shared/system entries.
    case "$name" in
        Shared|Guest|.localized) continue ;;
    esac

    # Skip the invoker's own home.
    [[ "$entry" == "$INVOKER_HOME" ]] && continue

    # Skip currently-active accounts.
    if dscl . -read "/Users/$name" >/dev/null 2>&1; then
        continue
    fi

    # Cheap top-level metadata only — no recursive size walk (du -sh on a
    # large home dir can take minutes).
    owner="$(stat -f '%Su' "$entry" 2>/dev/null || echo '?')"

    idx=$((idx + 1))
    CANDIDATES+=("$entry")
    DISPLAYS+=("$(printf '  [%d]  owner=%-12s  %s' "$idx" "$owner" "$entry")")
done

if [[ ${#CANDIDATES[@]} -eq 0 ]]; then
    ok "no orphaned home directories found in /Users."
    echo
    echo "If you have a deleted user's home in a non-standard location, use:"
    echo "  sudo bash <(curl -fsSL https://mac-storage-clear.flek.ai/claim.sh) /path/to/home"
    exit 0
fi

echo "Found ${#CANDIDATES[@]} orphaned home director$( [[ ${#CANDIDATES[@]} -eq 1 ]] && echo y || echo ies ):"
echo
for line in "${DISPLAYS[@]}"; do echo "$line"; done
echo

selection="$(ask "Enter indexes to claim (e.g. '1 3'), 'all', or 'q' to quit: ")"

case "$selection" in
    ''|q|Q|quit|exit) note "aborted."; exit 0 ;;
    all|ALL) TO_CLAIM=("${CANDIDATES[@]}") ;;
    *)
        TO_CLAIM=()
        for token in $selection; do
            if ! [[ "$token" =~ ^[0-9]+$ ]]; then
                err "invalid index: $token"; exit 1
            fi
            i=$((token - 1))
            if (( i < 0 || i >= ${#CANDIDATES[@]} )); then
                err "index out of range: $token"; exit 1
            fi
            TO_CLAIM+=("${CANDIDATES[$i]}")
        done
        ;;
esac

if [[ ${#TO_CLAIM[@]} -eq 0 ]]; then
    note "nothing selected; aborted."
    exit 0
fi

echo
echo "Selected (will become owned by $INVOKER:$INVOKER_GROUP):"
for d in "${TO_CLAIM[@]}"; do echo "  $d"; done
echo

# check/do/quit loop. 'check' runs a fast read-only scan, lists known-protected
# paths, and loops back. 'do' runs the parallel chown and exits.
while true; do
    echo
    action="$(ask 'Action — [c]heck (preview), [d]o (chown now), [q]uit: ')"
    case "$action" in
        c|C|check)
            for d in "${TO_CLAIM[@]}"; do
                check_target "$d" || true
                echo
            done
            ;;
        d|D|do)
            for d in "${TO_CLAIM[@]}"; do
                do_claim "$d" || true
            done
            break
            ;;
        q|Q|quit|'')
            note "aborted; nothing changed."
            exit 0
            ;;
        *)
            err "invalid choice: '$action'"
            ;;
    esac
done

echo
ok "all done. Mac Storage Clear (App Store, with FDA + folder grant for /Users) can now scan and delete the claimed paths."
