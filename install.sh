#!/bin/sh
# DriftInk source installer. All work is inside main so a truncated download
# cannot execute a partially received function body when piped into sh.
main() {
    set -eu

    case "${1-}" in
        -h|--help)
            cat <<'HELP'
Install DriftInk from https://github.com/Hell077/DriftINK (Linux / driftwm).

Usage:
  curl -fsSL https://raw.githubusercontent.com/Hell077/DriftINK/main/install.sh | sh
  sh install.sh [--prefix /absolute/path] [--data-dir /absolute/path]
  sh install.sh --uninstall

Downloads source from main and builds with Rust 1.89+ and Cargo. Also requires
Python 3.8+, curl, tar, a C toolchain, pkg-config and libxkbcommon development files.
Installs into ~/.local/bin and the application menu. No sudo or compositor edits.

Set DRIFTINK_REF to a branch, tag or commit using letters, numbers, dot, dash or
underscore (default: main). Prefix/data-dir/binary/uninstall arguments are passed
to the downloaded scripts/install-user.py. Drawings are preserved on uninstall.
HELP
            return 0
            ;;
    esac

    if [ "$(uname -s)" != Linux ]; then
        printf '%s\n' 'DriftInk requires Linux with driftwm.' >&2
        return 1
    fi
    driftink_ref=${DRIFTINK_REF:-main}
    case "$driftink_ref" in
        *[!a-zA-Z0-9._-]*|.|..)
            printf '%s\n' 'Invalid DRIFTINK_REF: use a simple branch, tag or commit.' >&2
            return 1
            ;;
    esac
    for driftink_tool in curl tar python3; do
        if ! command -v "$driftink_tool" >/dev/null 2>&1; then
            printf 'Missing required command: %s\n' "$driftink_tool" >&2
            return 1
        fi
    done
    if ! python3 -c 'import sys; sys.exit(sys.version_info < (3, 8))'; then
        printf '%s\n' 'Python 3.8 or newer is required.' >&2
        return 1
    fi
    driftink_build=yes
    for driftink_arg do
        case "$driftink_arg" in --binary|--uninstall) driftink_build=no ;; esac
    done
    if [ "$driftink_build" = yes ]; then
        for driftink_tool in cargo rustc cc pkg-config; do
            if ! command -v "$driftink_tool" >/dev/null 2>&1; then
                printf 'Missing build tool: %s. See the README build prerequisites.\n' "$driftink_tool" >&2
                return 1
            fi
        done
        if ! pkg-config --exists xkbcommon; then
            printf '%s\n' 'Install the libxkbcommon development package before building DriftInk.' >&2
            return 1
        fi
    fi

    driftink_tmp=$(mktemp -d "${TMPDIR:-/tmp}/driftink-install.XXXXXX")
    trap 'rm -rf -- "$driftink_tmp"' 0
    trap 'exit 130' INT
    trap 'exit 143' TERM
    printf 'Downloading DriftInk source (%s)...\n' "$driftink_ref"
    curl --fail --location --silent --show-error --proto '=https' --tlsv1.2 \
        --retry 3 --connect-timeout 15 --max-time 180 \
        "https://codeload.github.com/Hell077/DriftINK/tar.gz/$driftink_ref" \
        --output "$driftink_tmp/source.tar.gz"
    mkdir "$driftink_tmp/source"
    tar -xzf "$driftink_tmp/source.tar.gz" -C "$driftink_tmp/source" \
        --strip-components=1 --no-same-owner
    for driftink_file in Cargo.toml Cargo.lock scripts/install-user.py packaging/driftink.svg; do
        if [ ! -f "$driftink_tmp/source/$driftink_file" ]; then
            printf 'Invalid source archive: missing %s\n' "$driftink_file" >&2
            return 1
        fi
    done
    python3 "$driftink_tmp/source/scripts/install-user.py" "$@"
}

main "$@"
