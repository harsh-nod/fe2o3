# ROCgdb native runtime observations — opt-in producer source

SPDX-License-Identifier: GPL-3.0-or-later

This separate GPL tool supplies a bounded native-event producer for exactly
[ROCm/ROCgdb commit 48b1d324e389d2ed5e19822d377ff9050770233d](https://github.com/ROCm/ROCgdb/commit/48b1d324e389d2ed5e19822d377ff9050770233d).
It is not a Rust workspace member, MIT/Apache library, debugger installer, runtime
launcher, portable target profile or GPU test. Nothing here starts GDB,
automatically downloads/builds/installs software, creates a service, attaches to a
process or dispatches a kernel.

The patch series exposes numeric asynchronous MI records from the existing
AMD dbgapi client. These are diagnostic observations, not live authority or an
acceptance token. Controller integration and fresh debugger startup/native
qualification are deliberately separate. No physical registers are captured.

## What is shipped

- Three deterministic patches: bounded original native hooks (correct GPL
  notices from inception), explicit MI safe-point flushing, then lifecycle-cause
  placement. All three are required; intermediate stages are not an admitted
  debugger.
- Exact upstream/stage/final selected-file byte counts and SHA-256 digests in
  source-manifest.json. Patches and their order are pinned.
- Only the two new producer files in src/, byte-identical to the final patched
  versions, for standalone CPU tests. The upstream debugger tree and binary are
  not vendored.
- 20 ledger controls, 23 actual-hook test-double controls, seven MI placement
  controls, 17 lifecycle placement controls, and eleven bounded source-input
  controls. CPU fixtures are not evidence of an actual callback or MI delivery.
- A read-only source checker, GPL text and this explicit opt-in recipe.

These sources match a separately reviewed static debugger build. Packaging and
source identity do not transfer that build's environment or startup qualification.
Consult repository evidence for actual gate results; do not infer a native pass
from this README or a successful CPU test.

## Source preparation — separate reviewed commands

Obtain a **fresh, separate full checkout** of the exact upstream revision above.
Do not patch an installed debugger, a dirty user checkout or the fe2o3 source
tree. The examples use absolute paths chosen by the developer; these are source
and CPU-test paths, never executable/controller overrides. No command is run
automatically by fe2o3.

Set `producer_dir` to this tool directory and `rocgdb_source` to that canonical
absolute checkout. Confirm the upstream commit and clean worktree manually:

```sh
git -C "$rocgdb_source" rev-parse HEAD
git -C "$rocgdb_source" status --porcelain
node "$producer_dir/verify-source.mjs" upstream "$rocgdb_source"
```

HEAD must be the exact revision above and status must be empty. The source
checker verifies only seven selected paths (two absent upstream), not all other
upstream files, Git history, toolchain or loaded libraries. It never executes
code from the checkout.

Review and apply **each** patch in order; stop on any failure. Do not use
fuzz/three-way repair, force, a changed branch or a different source version.

```sh
git -C "$rocgdb_source" apply --check "$producer_dir/patches/0001-native-observation.patch"
git -C "$rocgdb_source" apply "$producer_dir/patches/0001-native-observation.patch"
node "$producer_dir/verify-source.mjs" native "$rocgdb_source"
```

Repeat the same check/apply/verify boundary for 0002-mi-safe-points.patch using
stage `mi-safe-points`, then 0003-lifecycle-causes.patch using stage `lifecycle`.
The final checker includes all seven postimages. The final two source additions
must also match src/ and source-manifest.json. No source check grants launch,
stop, memory-read, physical-register or protected-artifact authority.

## CPU controls — no GDB or ROCm linkage

Use a C++17 compiler and Node with node:test support. Choose a new private output
directory (`cpu_output` below); never overwrite a previous qualification output.
Review compiler choice and flags. Assertions are required: do not define NDEBUG.

```sh
c++ -std=c++17 -Wall -Wextra -Werror -pedantic -O2 "$producer_dir/tests/native-observation-tests.cc" -o "$cpu_output/ledger-tests"
"$cpu_output/ledger-tests"
c++ -std=c++17 -Wall -Wextra -Werror -pedantic -O2 "$producer_dir/tests/native-hooks-tests.cc" -o "$cpu_output/hooks-tests"
"$cpu_output/hooks-tests"
node --test "$producer_dir/tests/source-files-tests.mjs"
FE2O3_ROCGDB_TEST_SOURCE="$rocgdb_source" node --test "$producer_dir/tests/mi-safe-point-tests.mjs" "$producer_dir/tests/lifecycle-placement-tests.mjs"
```

The placement environment variable selects only bounded read-only text from the
exact seven-file postimage roster. A missing variable, different contents,
symlink/special file, invalid UTF-8, source drift or oversized input refuses;
there is no skip or fallback to a synthetic successful tree. The metadata cap is
32 KiB, source cap 256 KiB/file and aggregate 1 MiB. No subprocess is launched by
the checker. This is not a portable authority/configuration API.

All mutation controls establish the complete positive baseline first. The old
blanket-detach negative is reconstructed in memory using the four exact changed
source fragments, then required to match the recorded historical digest; it is
not a second executable, generic patch engine or vendored full upstream file.

## Optional debugger build is a different review boundary

The package intentionally has no acquisition, build, install or debugger-launch
script. A developer choosing to build must use a separate output tree and review
the upstream prerequisites, actual selected toolchain/libraries/configuration,
warnings, resource limits and process cleanup. Configure/make run native compiler
probes and generators; they are not inert text checks.

The source-matched qualification used AMD dbgapi with target
amdgcn-amd-amdhsa, Python 3.12, MPFR/GMP, bundled Readline, no Guile/Babeltrace or
source highlighting, and disabled sim/gdbserver/binutils/gas/ld/gprof/NLS. It
separated top-level configuration, prerequisite/GDB-subconfiguration bootstrap
(`all-gdb TARGET-gdb=Makefile`) and final `all-gdb`. That describes the reviewed
build, not automatically valid paths, library identities or choices on another
host. MAKEINFO=true means documentation was not qualified. Disabling werror
does not waive review of producer warnings. See the qualified build records for
exact dependency/tool/argument hashes.

Do not run make install, replace the system debugger or run the new ELF merely
to discover its version/configuration. Static ELF inspection is not execution.
An intended runtime user still needs fresh source/build/data closure review,
startup observation and an explicitly approved owned controller/family before
using a changed debugger. This producer package supplies none of those
permissions or qualifications.

## State and refusal contract

The ledger holds one owner for one debugger lifetime. It cannot reset or restart
on recycled identities. Its 32-row capacity reserves a terminal refusal slot;
flushing never replenishes it. Events retain original native IDs and actual
acknowledgment results. Query, owner, callback, setup, capacity, unwind and output
errors permanently invalidate the observation. Sent rows are consumed before
output callbacks so exceptions cannot replay successful rows.

MI explicitly flushes the existing ledger before its prompt and after a complete
*stopped record has been emitted/flushed and the MI output rewound. It does not
notify arbitrary CLI before-prompt observers. Both calls are guarded by
HAVE_AMD_DBGAPI. The independent consumer must handle the asynchronous record and
full state/order joins; this package does not relax it or add MI commands.

Cause13 marks actual GDB detach. Internal dbgapi teardown does not choose a
cause. Mourn records cause14 before teardown; existing exec/fork/object/exit
invalidations remain sticky. **Cause14 means lifecycle disappearance, not proof
of normal exit.** Kill and failed-attach cleanup also mourn. A future accepted
normal-exit relation must separately require actual normal MI exit, same-owner
events/callback ACKs/object identity and OS cleanup. Missing or changed terminal
events remain refusals.

This subtree is GPL-3.0-or-later; see LICENSE.md and COPYING. No component is
linked into the dual-licensed compiler or runtime crates.
