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

## Separate disabled physical-V2 successor

The additive [physical-V2 package](physical-v2/README.md) has its own source
contract and protocol. This V1 package and its historical qualification remain
unchanged. No activation, runtime binding or hardware acceptance is transferred.

## Separate disabled checkpoint diagnostic successor

The additive [physical-v7 package](physical-v7/README.md) labels first-failure checkpoint guards while preserving every original predicate and all disabled gates. It includes exact reversible source transforms and CPU controls. Physical-v6 and earlier packages remain unchanged; this source-only diagnostic is not a native-capture qualification or permission to ignore a refusal.

## Separate disabled scalar checkpoint successor

The additive [physical-v8 package](physical-v8/README.md) retains the checkpoint diagnostics and corrects the scalar host presentation mask from zero to lane-zero bit one. Its cumulative v6-to-v8 patch and self-contained controls leave physical-v7 and earlier packages unchanged. All activation, capture and publication gates remain disabled; source/CPU checks do not qualify a native capture.

## Disabled paired completion successor

The optional [physical-v9](physical-v9/README.md) source layer follows physical-v8 and adds the bounded, host-first paired CPU/wave completion transaction with actual-return and final-commit checks. Public activation, capture and publication remain disabled; source/CPU checks do not confer native qualification.

## Disabled owned/legacy query separation successor

The additive [physical-v10](physical-v10/README.md) layer follows physical-v9 and explicitly rejects the simultaneous legacy stopped-wave profile only while the owned profile is selected. It preserves all five real owned queue-health pairs and the original ten-query provider cap. The unselected legacy route and disabled public gates are unchanged; no native completion is qualified by this source-only correction.

## Disabled physical-v11 diagnostic successor

physical-v11/ retains the disabled v10 query-separation layer and adds only a fixed first-denied-budget record and bounded refusal fields. No cap or public authority gate changes; no native success or behavioral fix is claimed. New private products require fresh build, type measurement, startup and one-use readiness authority.
