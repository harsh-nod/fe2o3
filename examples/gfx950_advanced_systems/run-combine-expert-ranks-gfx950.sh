#!/usr/bin/env bash
if ! source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)/scripts/tutorial-hardware-runner.sh"; then
    exit 1
fi
fe2o3_tutorial_hardware_entry "$0" "$@" || exit
exec "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)/run-gfx950.sh" kernel-combine-expert-ranks
