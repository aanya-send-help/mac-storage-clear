#!/usr/bin/env bash
#
# claim-orphaned-home.sh — claim a deleted user's home directory.
#
# When you delete a macOS user account but keep their home folder, those files
# remain owned by the deleted user's UID. They become unreadable to any other
# account (including yours) without root. This script transfers ownership of
# everything under the given path to the user who invoked sudo, so the App
# Store build of Mac Storage Clear (with Full Disk Access + a user-granted
# folder) can scan and delete them without needing a privileged helper.
#
# Run from your normal user account prefixed with sudo:
#
#   sudo ./scripts/claim-orphaned-home.sh flek
#   sudo ./scripts/claim-orphaned-home.sh /Users/old-user
#
# This script is intentionally NOT bundled inside the app.

set -euo pipefail

usage() {
    cat <<EOF
Usage: sudo $0 <deleted-username | path-to-home-dir>

Examples:
  sudo $0 flek                    # claims /Users/flek
  sudo $0 /Users/old-user         # claims that absolute path

Transfers ownership of every file under the given path to the user who
invoked sudo, with their primary group. Run from your normal user account
prefixed with sudo — not as root directly.

Refuses to act if:
  - the target is outside /Users
  - a user account with the matching short name still exists
  - the target is your own home directory
EOF
    exit 1
}

[[ $# -eq 1 ]] || usage

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
    echo "Error: must be run with sudo." >&2
    usage
fi

if [[ -z "${SUDO_USER:-}" ]] || [[ "$SUDO_USER" == "root" ]]; then
    echo "Error: run via 'sudo' from a normal user account, not as root directly." >&2
    exit 1
fi

ARG="$1"
case "$ARG" in
    /*) TARGET="$ARG" ;;
    *)  TARGET="/Users/$ARG" ;;
esac

if [[ ! -d "$TARGET" ]]; then
    echo "Error: $TARGET does not exist or is not a directory." >&2
    exit 1
fi

# Resolve to absolute, canonical path so .. tricks can't escape /Users.
TARGET_REAL="$(cd "$TARGET" && pwd -P)"
case "$TARGET_REAL" in
    /Users/*) ;;
    *) echo "Error: refusing to operate outside /Users (resolved $TARGET_REAL)." >&2; exit 1 ;;
esac

# Don't allow operating on /Users itself or /Users/Shared.
case "$TARGET_REAL" in
    /Users|/Users/Shared)
        echo "Error: refusing to chown $TARGET_REAL (top-level shared)." >&2
        exit 1
        ;;
esac

TARGET_BASENAME="$(basename "$TARGET_REAL")"

# Refuse if a user account with that short name still exists in DirectoryService.
if dscl . -read "/Users/$TARGET_BASENAME" >/dev/null 2>&1; then
    echo "Error: a user account named '$TARGET_BASENAME' still exists in DirectoryService." >&2
    echo "Delete the account first, or rename the directory before re-running." >&2
    exit 1
fi

SUDO_HOME="$(eval echo "~$SUDO_USER")"
if [[ "$TARGET_REAL" == "$SUDO_HOME" ]]; then
    echo "Error: $TARGET_REAL is your own home directory; nothing to do." >&2
    exit 1
fi

SUDO_GROUP="$(id -gn "$SUDO_USER")"

echo "About to claim ownership:"
echo "  Path:       $TARGET_REAL"
echo "  Size:       $(du -sh "$TARGET_REAL" 2>/dev/null | awk '{print $1}')"
echo "  New owner:  $SUDO_USER:$SUDO_GROUP"
echo
read -r -p "Proceed? [y/N] " confirm
case "${confirm:-N}" in
    y|Y|yes|YES) ;;
    *) echo "Aborted."; exit 0 ;;
esac

echo "Claiming ownership... (may take a minute on large trees)"
chown -RhP "$SUDO_USER:$SUDO_GROUP" "$TARGET_REAL"
echo "Done. $TARGET_REAL is now owned by $SUDO_USER:$SUDO_GROUP."
echo
echo "Permission bits were not changed. If a file was mode 0600 or 0700 owned"
echo "by the deleted user, you (now the owner) can read and write it. If you"
echo "need broader access for the directory tree, run:"
echo "  chmod -R u+rwX,g+rX $TARGET_REAL"
