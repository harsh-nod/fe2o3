#!/bin/sh
set -eu
if [ "${1:-}" = "--version" ]; then
    printf 'Version: 0.2026.08.02.b677dd5\n'
    exit 0
fi
printf 'fake verifier must never reach proof execution\n' >&2
exit 99
