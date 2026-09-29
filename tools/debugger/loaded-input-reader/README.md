# Bounded loaded-input reader component

This inactive component checks the complete historical loaded-input selection. Importing its modules performs no file read or native action. It is separate from the pure [loaded-profile package](../loaded-profile/README.md).

## Fixture-free protocol controls

From the repository root:

```sh
node --test tools/debugger/loaded-input-reader/reader-protocol.test.mjs
```

These 50 controls use an in-memory I/O provider. They check the bounded read protocol, failures, aliases, identity checks, absence handling, accounting and descriptor cleanup. They do not read historical operational files or establish fresh physical observation.

## Explicit historical bridge controls

The separate historical-reader-plan-controls.mjs file is deliberately not named *.test.mjs. Its ten controls preserve the historical planner-to-reader checks and require the same explicit 76-role fixture manifest as the loaded-profile package:

```sh
FE2O3_LOADED_REVIEW_FIXTURES=/absolute/path/fixtures.json \
  node --test tools/debugger/loaded-input-reader/historical-reader-plan-controls.mjs
```

The manifest and fixture-reader contract is documented in the loaded-profile README. Missing configuration or changed/missing fixture data fails; there is no skip, download, historical path fallback or data-module execution. No raw host fixture archive is shipped here. The shared reader source-pins all 76 fixture contents; the manifest is bounded/read/parsed/identity-bracketed and may be separately pinned by a root qualification gate.

## APIs and scope

buildHistoricalReadProtocol accepts complete historical Buffer inputs through the pure planner. It preserves all 1,173 named entries: 1,150 readable paths and 23 absence obligations. All 1,024 original-input and 193 loaded-name labels, original duties, extra roles, alias dispositions and inherited member caps remain represented. Its result is a data-plane proposal, not an approved operational graph or input-cap change.

readUnqualifiedProtocol runs with an explicit caller-supplied provider, guard and clock. Its result is always unqualified; a synthetic provider cannot establish real filesystem observation. For the historical bridge’s fixed two-pass protocol, it makes two complete immediate passes with exact-length-or-refuse 64 KiB reads, full hashes, EOF probes, named/target/descriptor identity brackets and final whole-name revalidation. The generic injected protocol admits one or two passes; the two-pass counts below belong to the historical bridge. The 23 missing cache names are bounded canonical-ancestor/ENOENT observations, not fake zero-byte files. Independently named alias targets remain independently paid duties.

The two data-plane passes reserve 3,635,850,472 content/EOF bytes, 59,496 content calls and at most 25,482 explicit metadata-provider calls. One 65,536-byte scratch buffer and one EOF byte are reused. Provider-call counts are not all kernel syscalls, Node loader IO or an exact JavaScript heap theorem. The finite clock guard cannot preempt a blocking synchronous operation; an external scheduler must enforce process timeout.

inspectHistoricalLoadedFiles is the explicit filesystem entry. It has no CLI, default invocation or live-controller wiring. Its external guard must bind separately reviewed finite scope and currentness. It still returns qualified=false and does not approve its own source/tools/input selection, larger gate cap, lease, subsequent debugger operation or global writer exclusion. Do not treat a passing synthetic control or a successful immediate read interval as permission for a later native action.

The exact three historical alias rules remain source-owned DATA. No generic symlink allowance is introduced. Descriptor cleanup runs after guard/deadline refusal but makes no retry or success claim on a close failure; failure accounting retains prior debits.

## Remaining integration boundary

A future operational use still needs the complete individually pinned reader/planner/profile/tool/policy/evidence graph, a separately reviewed finite outer budget, currentness custody and any specifically scoped native coordination. Module-loader IO and the externally supplied historical Buffer reader remain separate from the data-plane meter. The old inner artifact-subset cap is unchanged.

This package does not launch GDB, attach an inferior, dispatch a GPU kernel, capture physical registers, infer complete import history or establish cache execution provenance. Historical failed-startup evidence and every historical stage limit remain intact.
