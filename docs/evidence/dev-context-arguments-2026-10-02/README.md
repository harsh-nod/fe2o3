# Generated Ordinary Context Arguments

Implementation baseline: `25ba6ee2a78e524d98820ac4a57908b9f64a4167`.

## Scope

Typed scalar/ordinary-slice kernels now generate owned, address-free
`ContextArguments` implementing the existing `RuntimeArgumentsV1`. Three host
descriptors retain checked allocation-relative metadata with fixed Read, Write
or ReadWrite access. The canonical ABI supplies offsets, sizes and signature;
scalar/count bytes are little-endian, pointer slots and padding remain zero.
Mapped slices, device-global pointers and compiler-laid-out aggregates do not
receive this adapter. Existing borrowed and Worker V3 owned adapters remain
unchanged. See [the public flow](../../runtime-context-arguments-v1.md).

Descriptors do not retain an allocation borrow or owner. Context admission
revalidates live allocation identity, device and bounds. Descriptor metadata
does not provide alias, initialization, compiler, machine-code or launch
authority. The existing backend execution checks remain mandatory.

## Qualification

Selected final-source runs pass:

- 307 host and 64 macro library tests, with four existing host ignores.
- 2,198 runtime library tests, with 32 existing hardware ignores.
- Five real-host downstream fixture tests: exact ABI bytes/access/signatures,
  all ten primitive scalar widths, invalid allocation/device/range rejection,
  backend alias/authority denial, and a queued two-device pipeline.
- Eleven macro integration tests, including seven new compile-fail cases for
  unsupported profiles, typed substitution and private fields. The launcher
  also reruns the five fixture tests; that is not five additional unique tests.
- Four host UI suites covering 62 compile-fail cases, plus 30 host doctests.
- Strict host/macro Clippy, formatting of the changed packages/fixture files,
  and whitespace checks.

The pipeline uses the public journaled Context API and a scripted CPU backend.
Two producers feed distinct windows of one gathered allocation; one compute
consumer feeds a return peer and host readback. All seven operations are queued
before effects, public events are released, and only final-stream flush drives
completion. Every returned element, including the untouched gather-frame
elements, is checked. The callback fires once and explicit shutdown checks an
empty backend resource roster. These are simulated effects, not execution of
the generated kernel bodies or proof of native overlap.

`audit-01.json` validates all selected command receipts, exact source identity,
test totals and executable hashes/rosters. Runtime, model and KFD source are
unchanged from the baseline. Source inventories include 6,119 files. All runs
are local CPU qualification; no hardware, performance or solver campaign ran.

## Retained Diagnostics

The preliminary library build completed while fixture sources were changing and
is not selected. The first fixture attempt could not find a cached dependency;
the first fetch lacked the local CA bundle. A verified-CA fetch succeeded and
subsequent tests used the locked offline dependency graph. The fixture lockfile
refresh preserves all third-party versions and repairs stale workspace edges.

The next fixture build rejected a renamed-qualified device marker spelling.
The existing frontend recognizes imported marker names or canonical
`fe2o3_device::...` names, not `gpu_device::...`. Importing both the slice and
pointer markers fixes the positive fixture and makes the pointer negative test
exercise its intended profile. No frontend grammar was widened. Earlier
integration failures and successful runs spanning that fixture edit are retained
but not selected; all selected receipts have the identical final source roster.

Workspace-wide and whole-fixture formatting checks report pre-existing
differences in six untouched files. They were not reformatted. Changed-file
formatting passes. An initial audit roster listing needed the pinned Rust
toolchain library path for the proc-macro test executable; the final audit
records that listing environment explicitly.

## Evidence And Limits

`raw.tar.xz` retains the bounded commands, stdout/stderr, source snapshots,
executable rosters, audit, candidate patch and diagnostics. `raw-manifest.json`
hashes every retained file; `SHA256SUMS` binds this README, manifest and archive.
No shared-host files or GPU resources were created. Only the inactive local
incremental build cache was removed before builds; source and evidence stayed
intact.

This increment does not qualify generated arguments on hardware, an async-owner
snapshot integration test, a production verifier/refinement provider, native
late-gather admission, native fault isolation or full HIP/HSA parity. Historical
native gathered-consumer receipts remain scoped to their original finite R57
authority and are not reused as hardware qualification for this API.
