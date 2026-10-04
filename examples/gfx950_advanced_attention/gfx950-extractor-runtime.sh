#!/usr/bin/env bash

# Resolves the extractor and the runtime-library directories Cargo must retain
# while invoking it as RUSTC_WORKSPACE_WRAPPER. Callers receive canonical paths
# in EXTRACTOR, EXTRACTOR_RUNTIME_DIR, and EXTRACTOR_DEPS_DIR.
resolve_gfx950_extractor_runtime() {
    if [[ $# -ne 1 ]]; then
        printf 'resolve_gfx950_extractor_runtime requires one extractor path\n' >&2
        return 2
    fi
    local candidate=$1 directory
    if [[ ! -f $candidate || -L $candidate || ! -x $candidate ]]; then
        printf 'generic rustc extractor must be an executable regular non-symlink file: %s\n' \
            "$candidate" >&2
        return 1
    fi
    EXTRACTOR=$(cd -- "$(dirname -- "$candidate")" && pwd -P)/$(basename -- "$candidate")
    EXTRACTOR_DIR=$(dirname -- "$EXTRACTOR")
    if [[ $(basename -- "$EXTRACTOR_DIR") == deps ]]; then
        EXTRACTOR_DEPS_DIR=$EXTRACTOR_DIR
        EXTRACTOR_RUNTIME_DIR=$(dirname -- "$EXTRACTOR_DIR")
    else
        EXTRACTOR_RUNTIME_DIR=$EXTRACTOR_DIR
        EXTRACTOR_DEPS_DIR=$EXTRACTOR_DIR/deps
    fi
    for directory in "$EXTRACTOR_RUNTIME_DIR" "$EXTRACTOR_DEPS_DIR"; do
        if [[ ! -d $directory || -L $directory ]] ||
            [[ $(cd -- "$directory" && pwd -P) != "$directory" ]]; then
            printf 'extractor runtime directory is absent, symlinked, or noncanonical: %s\n' \
                "$directory" >&2
            return 1
        fi
    done
}
