# ROCgdb stopped-wave observations — separate opt-in GPL source

SPDX-License-Identifier: GPL-3.0-or-later

This package adds an observation-only successor to the exact lifecycle stage of
[the runtime observation tool](../rocgdb-runtime-observation-v1/README.md). It is
for [ROCm/ROCgdb commit 48b1d324e389d2ed5e19822d377ff9050770233d](https://github.com/ROCm/ROCgdb/commit/48b1d324e389d2ed5e19822d377ff9050770233d).
The older tool directory, runtime ledger and old no-queue refusal are unchanged.
This is not a Rust workspace member or a MIT/Apache library.

Nothing here acquires software, starts GDB, attaches to a process, creates a
queue, dispatches a kernel, reads physical registers or memory, or supplies
sample authority. There is no runtime launcher, new MI command, build script or
installed-debugger replacement. Packaging does not complete hardware debugging.

## Source and protocol boundary

The single patch layers on all three patches of the existing runtime tool.
It changes seven files and adds four; only five standalone hook/header leaves
are shipped in src/. No full upstream source tree or executable is vendored.
source-manifest.json pins 19 selected source paths, the exact predecessor
package inputs, the one patch, GPL text and external AMD API header.

The new asynchronous MI tag is `amd-stopped-wave-observation-v1`, schema
`fe2o3-gfx950-native-stopped-wave-observation-v1`, profile
`fe2o3.gfx950.same-client-stopped-wave-observation.v1`. These do not reuse the
old no-queue schema: kind 5 here is a completed stop observation, kind 6 is
invalidation. An old consumer must not reinterpret these rows.

The same real AMD client/event owner supplies the native process, callback and
sole acknowledgment. A handled WAVE_STOP with successful ACK can stage a row;
it cannot publish one. Publication follows a complete flushed MI *stopped row,
the exact current stopped GPU thread and a second successful identity query.
Every subsequent MI command invalidates before dispatch, including CLI-through-
MI and Python execute_mi. Resume, writes, context changes, failures and lifetime
changes also invalidate. One lifetime, one generation, no retry/reset.

The current profile is deliberately narrow: gfx950, Wave64, DEBUG_TRAP, one
64×1×1 workgroup/grid, wave zero, group zero, HSA AQL, supported agent, no queue
error, and zero private/group segment sizes. Missing identity information
refuses. See [SOURCE-CONTRACT.md](SOURCE-CONTRACT.md).

Every row says `queue-provenance="unavailable"`. Observed queue/dispatch IDs
do not establish owned creation, packet/allocation lifetime or cleanup. No
compiler entry/descriptor relation, register roster, memory extent, trap/CWSR
ownership or read capability is established. No PC, EXEC or GPU address bits
are reported as samples.

## Prepare a separate source tree

Explicitly obtain and review a fresh full checkout at the exact commit. Apply
and verify the existing runtime tool's three patches in order, ending at
`lifecycle`. Do not modify an installed debugger, dirty user checkout or the
fe2o3 source tree. Set `producer_dir` to this directory and `rocgdb_source`
to that canonical absolute external checkout.

```sh
node "$producer_dir/verify-source.mjs" lifecycle "$rocgdb_source"
git -C "$rocgdb_source" apply --check "$producer_dir/patches/0001-stopped-wave-observation.patch"
git -C "$rocgdb_source" apply "$producer_dir/patches/0001-stopped-wave-observation.patch"
node "$producer_dir/verify-source.mjs" stopped-wave "$rocgdb_source"
```

Stop at any failure; no fuzzy/three-way repairs or changed revision. The checker
reads only the pinned 19 paths, not the entire checkout or Git history, and
does not apply patches. Four new leaves must be absent at the lifecycle stage.
It reuses the unchanged predecessor's bounded file reader, whose exact bytes
are pinned. Existing runtime source tests target the lifecycle tree, not this
successor tree whose shared postimages intentionally differ.

Inputs must be canonical absolute paths and stable regular files, without
symlink/special-file substitution; strict UTF-8, byte count and SHA-256 must
match. Limits are 32 KiB for metadata, 512 KiB per source/header and 2 MiB
aggregate selected-source payload. The larger per-file bound includes the
pinned 363,460-byte infrun.c context. These are payload checks, not a whole-tree
atomicity/isolation or process-memory guarantee. No source check grants native
execution or current-stop authority.

## Explicit CPU controls

The fixtures include the exact standalone producer leaves and the unchanged
runtime tool header by a fixed relative include. The AMD API header remains an
external dependency, not copied or relicensed here. Set `amd_api_include` to
a reviewed include root containing that header and choose a fresh `cpu_output`
directory. Verify the header before compiling; no API library is linked.

```sh
node "$producer_dir/verify-api-header.mjs" "$amd_api_include/amd-dbgapi/amd-dbgapi.h"
c++ -std=c++17 -Wall -Wextra -Werror -pedantic -O2 "$producer_dir/tests/ledger-tests.cc" -o "$cpu_output/ledger-tests"
"$cpu_output/ledger-tests"
c++ -std=c++17 -Wall -Wextra -Werror -pedantic -O2 -I"$amd_api_include" "$producer_dir/tests/native-hooks-tests.cc" -o "$cpu_output/hooks-tests"
"$cpu_output/hooks-tests"
node --test "$producer_dir/tests/source-files-tests.mjs"
FE2O3_ROCGDB_STOPPED_WAVE_TEST_SOURCE="$rocgdb_source" node --test "$producer_dir/tests/source-contract-tests.mjs"
```

The 79 ledger and 106 actual-hook/test-double controls retain the original
semantics. Fourteen external-tree placement controls require the complete
positive postimage before mutations; no absent-env skip or synthetic success
fallback. Sixteen additional package-input controls exercise bounds, roster,
predecessor, license, patch, encoding and external-header refusals. None proves
a native callback, live stop or MI delivery. Paths above select only source
inputs/output locations, not executable/controller overrides.

The upstream source-identical overlay had 199 CPU controls and a fresh full
static debugger build pass before packaging. [historical-evidence.json](historical-evidence.json)
records those exact receipts and ELF identity. The shipped include-path
adaptation, new checker and package layout need their own qualification; they
do not inherit a package-test pass. The new debugger was not executed by those
gates. Source and ELF hashes do not transfer startup or runtime qualification.

## Optional build is a separate review boundary

There is deliberately no acquisition, configure, make, install or launch
automation. After source and CPU checks, a developer may independently review
the exact external dependencies, tools, configuration and resource/process
supervision and choose a separate build directory. Configuration and make
execute compiler probes and generators; they are not inert checks.

The historical build used AMD dbgapi, target amdgcn-amd-amdhsa, Python 3.12,
MPFR/GMP and bundled Readline; Guile/Babeltrace/source highlighting and
sim/gdbserver/binutils/gas/ld/gprof/NLS were disabled. Top-level configuration,
dependency/GDB-subconfigure bootstrap (`all-gdb TARGET-gdb=Makefile`) and
final `all-gdb` were separate reviewed stages. Check the pinned Makefile
contract before using that bootstrap override. MAKEINFO=true leaves
documentation unqualified. These choices are not a portable loader closure or
a guarantee that another host is safe/configured equivalently.

Do not run make install, replace a system debugger or execute the new ELF to
discover its version. Any intended native use still requires fresh built-ELF,
data/Python/loader closure review, startup qualification, exact owned process/
queue/trap/resource lifetime and cleanup controls, plus independent same-stop
admission. This source package supplies none of them.

See [LICENSE.md](LICENSE.md) and COPYING. No component is linked into fe2o3's
dual-licensed compiler/runtime crates; no milestone acceptance count changes.
