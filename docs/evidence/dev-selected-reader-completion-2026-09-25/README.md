# Selected Reader Completion Development

This packet tracks the validate-once Context reader refactor and source-shared
selected-root journal/retirement proof. See the
[boundary document](../../runtime-context-selected-reader-completion-v1.md).
It is not hardware, performance, complete HashMap/Context, or A1/A2 qualification.

The development campaign from signed source
`aef1c70d9fdf4001a0070f4a6384551988ea93c3` passes all eleven CPU phases and
all eleven proof stages. This is bounded source-bound development evidence,
not acceptance of a complete runtime refinement or a new lane checkpoint.

## Results

- GNU and musl each pass 1,413 runtime tests, with 22 hardware-only ignores.
  All 46 doctests, default-feature checks, strict all-feature/all-target Clippy,
  formatting, four CPU-runner tests and three proof-runner tests pass.
- CPU source brackets retain the same 5,953 inputs. The focused regressions
  cover 40 completion-fault scenarios and eight generated prevalidation cases.
- Both whole-root Verus runs verify 1,303 obligations with zero errors and no
  diagnostics. All seven deliberately defective variants produce accepted
  logical failures. No solver limits are raised.
- Proof source brackets retain the same 482 inputs, with unchanged inherited
  sources and matching before/after 190-file verifier closure checks.
- The complete [retained records](retained/) include failed development attempts
  and both signed-source campaigns. The [retention inventory](retention.json)
  records 4,518 byte-exact copied files, checked again after cleanup.
- [Cleanup receipts](cleanup-after.json) confirm all 23 recorded process groups
  absent and both exact-owned scratch paths removed, with independent parent
  directory checks. Removed path-accounted allocation totals 2,016,534,528 bytes.
  No shared verifier installation or foreign files were removed.

These checks do not constitute a relocated packet replay audit or global proof
registration. The full Context prevalidator, HashMap implementation, allocator
capacity correspondence, quarantine and later completion effects remain outside
the selected-entry proof. No native or performance result is added.

## Reproduction

Use a clean signed source revision, the pinned Verus release, and a disposable
Cargo target. From the repository root:

```sh
python3 -I -B docs/evidence/dev-selected-reader-completion-2026-09-25/run.py --output CPU_OUTPUT --target CARGO_TARGET
python3 -I -B docs/evidence/dev-selected-reader-completion-2026-09-25/proof.py --output PROOF_OUTPUT --verus PINNED_VERUS
```

The CPU runner requires exact passing counts and named regressions, checks source
continuity, and refuses interrupted or unreaped process groups. Its source
inventory is a workspace-development boundary, not complete dependency/tool
authentication. The proof runner authenticates its inherited controller before
executing those exact bytes, preserves unchanged inherited sources, checks signed
source and the 190-file verifier closure, and brackets seven clean logical mutation
rejections with whole-root positives. Neither script is a relocated packet replay
auditor. Retained logs and checksums do not by themselves establish qualification.

## Development History

- The original focused producer-launch suite passed 26 tests, including the
  existing 40-case optional-root matrix and 40 completion-fault scenarios.
- A new generated-domain regression passed eight invalid-roster/domain cases
  across present/absent backend submission records, before the effect hook.
- Initial GNU development validation passed 1,413 runtime tests with 22 hardware
  ignores. This is not the final source-bound CPU campaign.
- The first scoped proof checked 25 obligations but reported a generic Clone
  specification warning. An explicit verified Clone implementation replaces it.
- A composition draft had a macro-hygiene compile error, then a witness exposed
  missing intermediate journal representation in its relational contract.
  The relation now preserves actual/logical representation between stages.
- Broad witness attempts exhausted the unchanged solver resource limit. A first
  constructor/oracle split also failed. These remain rejected; success, stable
  error, and producer error now use separate instances of the same witness body,
  without removing the assertions, presence/error cases, or increasing limits.
- The partitioned witness still exhausted resources while reconstructing actual
  state through the logical model. The adapter contracts now additionally expose
  the actual journal transition already guaranteed by the paired journal calls.
  Witnesses use those stronger contracts; logical correspondence remains proved.
  The final scoped development run passes all 32 obligations with no diagnostics.
  Macro-hygiene and misplaced proof-directive compile failures remain recorded.

All these attempts are development observations, not accepted proof negatives.

## CPU Scheduling Limits

An overlapping manual musl suite failed two deadline-sensitive tests: the Tokio
recovery test reached its outer five-second timeout, and the Worker V4 flush test
returned the expected `ResponseTimeout` but exceeded its 75 ms scheduling grace.
The later source-bound GNU and musl phases both pass all 1,413 tests. The failed
manual run remains failed and retained; no test tolerance or production timeout
is changed to obtain those passes.

Review found equal five-second inner/outer deadlines in the Tokio test and
synchronous child kill/wait/thread joins after the Worker timeout. These failures
are consistent with host scheduling sensitivity; the passing suites do not prove
hard real-time return bounds or robustness under arbitrary CPU starvation.

## Hardware Scope

A read-only MI300X availability screen at 2026-09-25T19:02:07Z through 19:02:09Z
found only GPU 1 individually idle under the campaign criteria. No eligible pair
was available. No remote build, workload, file creation, reset, or cleanup was
performed. This screen is not retained strict-campaign hardware evidence and
adds no performance result.
