#!/bin/sh
set -eu

if [ "${1:-}" = "--version" ]; then
    printf '%s\n' \
        'Verus' \
        '  Version: 0.2026.08.02.b677dd5' \
        '  Profile: release' \
        '  Platform: linux_x86_64' \
        '  Toolchain: 1.97.1-x86_64-unknown-linux-gnu'
    exit 0
fi

printf 'fake verifier must never reach proof execution\n' >&2
exit 0
