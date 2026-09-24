# Disabled one-stop native adapters — source package

SPDX-License-Identifier: GPL-3.0-or-later

This separate, opt-in GPL tooling packages the exact **disabled R4** ROCgDB
adapter source. It is not part of the Rust workspace or a MIT/Apache library.
It is not the later enabled R5 candidate. The real adapter hooks compile in
GDB, but selection_available() is a constant false source gate, before
reservation, file access or breakpoint installation. No environment, command,
JSON, receipt or pointer enables this package.

The base is ROCm/ROCgdb commit
48b1d324e389d2ed5e19822d377ff9050770233d plus the unchanged sibling
rocgdb-runtime-observation-v1 lifecycle patches and then the unchanged sibling
rocgdb-stopped-wave-observation-v1 patch. The new patch changes seven existing
files and adds fifteen standalone headers/hooks. Sixteen standalone leaf
copies are supplied (the fifteen additions plus the changed runtime hook).
No full upstream tree or debugger executable is vendored.

## Read-only source checks

With a separately prepared exact source checkout and a trusted Node executable,
these commands inspect only selected files. They never acquire, patch, build,
install, launch or attach a debugger:

    node tools/rocgdb-one-stop-native-adapters-v1/verify-source.mjs /absolute/source stopped-wave
    node tools/rocgdb-one-stop-native-adapters-v1/verify-source.mjs /absolute/source one-stop-disabled-r4
    node tools/rocgdb-one-stop-native-adapters-v1/verify-api-header.mjs /absolute/amd-dbgapi.h
    node --test tools/rocgdb-one-stop-native-adapters-v1/tests/source-files-tests.mjs

The two source stages require different exact contents; the second command
is only for a separately prepared tree containing this package's patch.
The patch is data, not an automatic installer. Selected-source controls require
an explicit external tree and refuse missing inputs instead of skipping:

    FE2O3_ROCGDB_TEST_SOURCE=/absolute/disabled-r4-source node --test tools/rocgdb-one-stop-native-adapters-v1/tests/placement-tests.mjs tools/rocgdb-one-stop-native-adapters-v1/tests/presentation-tests.mjs

The inert C++ activation-locator.cc fixture can be compiled independently by
a reviewer with C++17. It calls neither GDB nor ROCdbgapi and statically asserts
that activation is disabled. There is no build or execution automation here.
Package controls are newly adapted and need their own qualification; historical
private-overlay results do not count as tests of this package.

## Bounds and source integrity

The verifier pins the exact predecessor manifest, patch, license and bounded
reader. It reads 48 selected R4 source files totaling 1,963,940 bytes under an
unchanged 2 MiB source-payload cap, with a 512 KiB per-file cap. The 39,934-byte
manifest has a separate 64 KiB cap and an embedded exact contract digest.
Canonical absolute paths, regular non-symlink files, exact lengths/hashes,
strict UTF-8 and before/after file stamps are required. Source stages, absence
rows, path roster, predecessor and patch relation are closed.

This is not an atomic whole-tree snapshot, process isolation, full RSS bound,
proof of a library loaded at runtime, or a complete checkout verifier. Tooling
and files must be trusted and externally kept stable. The read-only checker
returns no runtime or source-publication permission. The existing runtime
package's bounded reader is reused byte-for-byte, not changed.

## What is retained — and what is not qualified

The disabled code contains a single-client owner, bounded native query/read
adapters, the original sole-event ACK chain, generic pre-effect resume fences,
actual callback step checks, breakpoint retirement, object removal and
current-stop requery hooks. SOURCE-CONTRACT.md records the boundaries.

historical-evidence.json pins the completed private R4 static build and its
outer receipt. That build produced an ELF; it did not execute it, its target,
or a GPU workload. The target profile retains its exact historical absolute
path and measured hash deliberately: it is not a portable executable selector
or an example launch command. Editing it, lifting activation, or transferring
old startup evidence requires a new source review and qualification.

No startup/owned-family/same-client execution, queue publication, stopped-wave
physical sample, runtime acceptance or milestone completion is established by
this package. External attach/sole ACK, trap/TTMP and sampler exclusion,
current owned process/queue lifetime, loader closure and cleanup prerequisites
remain mandatory before any separately reviewed executable attempt. The old
stopped-wave package is unchanged; its next-command invalidation is not
reinterpreted as a public read exception.

## Package qualification update

Root qualified this relocated disabled package on 2026-09-24: source verifiers,
47 Node controls and a strict inert C++ fixture passed. The activation gate
remains false; no target/debugger was invoked. See the [qualification record](../../docs/source-transport-tiled-debugger-qualification-20260924.md) for exact receipts and remaining boundaries.
