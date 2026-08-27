#!/usr/bin/env bash
exec "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)/run-gfx950.sh" kernel-speculative-transaction
