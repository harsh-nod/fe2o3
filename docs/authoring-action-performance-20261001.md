# Authoring action performance — 2026-10-01

The four measured authoring actions meet the existing 250 ms warm-action p95 and 64 MiB retained logical-storage targets for the named `assembly_chain` fixture below. This is a fixture-specific result, not a universal module-size envelope, shipping-process memory bound or whole-milestone completion claim.

## Named case and results

The input is a genuine source-derived Bundle V6 containing 16 operation rows. The operations query requests one page, starting at 0 with a limit of 64, and returns all 16 rows. Selection identifies the `BitOr` at function 0, block 6, operation 1. Materialization produces the typed-Rust `budget_candidate` draft for that same selection; it does not compile or publish the draft.

| Action | Warm API p95 (ms) | Retained logical bytes |
| --- | ---: | ---: |
| Inspect snapshot | 0.34655 | 92,315 |
| List the first operations page | 0.24050 | 112,861 |
| Select the bound region | 0.03501 | 94,814 |
| Materialize the Rust draft | 0.05221 | 92,805 |

All four p95 values are at most 250 ms. All four named companion checkpoints are at most 67,108,864 bytes (64 MiB). Timing and storage come from separate processes and are not simultaneous peak measurements. The [compact evidence file](evidence/authoring-action-performance-20261001.json) retains exact nanoseconds, all 140 raw samples, component byte counts, case arguments and output/input/report hashes.

The campaign ran 12 successful processes: one shipping CLI baseline, one storage companion and one warm companion for each action. Eight complete stdout comparisons—each companion against its independently executed shipping baseline—matched byte for byte, including the final newline. The associated regression audit recorded 1,724 passing tests and one ignored test across 50 suites.

Before publication, concurrent main changes through `e9cfe00825f3b55a7c58d8782d9920f198bcefd8` were integrated without overlapping these authoring files. The combined tree again passed all 1,724 selected Rust tests (one ignored), plus 175 tutorial Python tests. The benchmark numbers above remain the original recorded campaign, not a retimed post-merge series.

## Timing boundary

Each warm process admits the bundle and constructs one retained `AuthoringSnapshotV1`, then calls one original action API 35 times. The first five calls are calibration; the final 30 are measured. Across four actions, that is 20 calibration and 120 measured calls. Nearest-rank p95 selects the 29th sorted measured value. No slow sample is removed or replaced.

The clock starts immediately before the original API invocation and stops immediately after its return. It includes the API's own report-limit serialization wherever that API performs it, plus the small closure/clock overhead. It excludes initial input reading, bundle verification, snapshot admission, query parsing, companion output serialization, hashing, equality checks, result destruction, statistics and stream writes. This is not fresh CLI, compilation or end-to-end authoring latency.

Every returned result is independently serialized and compared with the first result outside the timer, then dropped before the next call. The first serialized output and bounded sample records remain retained; their cache/allocator influence is not claimed absent. Repeated calls in one process are not independent trials.

## What the storage number means

The storage companion observes four simultaneously live components immediately before normal output serialization: the actual snapshot, argument vector, intact parsed query and one returned result. The snapshot contributes 90,636 bytes in every case. The evidence file gives each remaining component separately.

The companion deliberately borrows and retains the intact `Query`. The shipping CLI consumes and destructures its query. Therefore, these figures are not proof of identical shipping stack layout or lifetimes, despite exact output equality.

The logical model counts root headers once, actual `Vec`/`String` capacities including spare capacity, owned `Box` payloads, and logical B-tree key/value payloads. It does not count physical B-tree nodes or spare node slots, allocator metadata, decoder/action/serializer temporaries, runtime infrastructure, parent capture buffers, peak heap or RSS. Nor does one returned result cover a persistent session accumulating multiple pages, regions or drafts. These measurements used a 64-bit pointer ABI.

The target is an observation comparison, not a new constructor limit. A complete over-target result remains evidence of a target miss; a failed or incomplete walk does not become a zero-byte result.

## APIs and reproduction

The new [snapshot accounting API](../crates/fe2o3-source-isa-observation/src/multilevel_authoring_retained_storage_v1.rs), `AuthoringSnapshotV1::retained_logical_storage_v1`, returns an `AuthoringSnapshotRetainedStorageV1` breakdown using explicit `LogicalStorageLimitsV1`. The shared `LogicalStorageCounterV1` checks byte/item arithmetic and traversal limits.

The [result accounting methods](../crates/fe2o3-source-isa-observation/src/multilevel_authoring_result_storage_v1.rs), `charge_retained_heap_v1`, cover `AuthoringRegionSelectorV1`, `AuthoringSnapshotSummaryV1`, `AuthoringOperationPageV1`, `AuthoringRegionV1` and `AuthoringRustCandidateV1`. These heap methods exclude the result's inline root header; a caller composing totals must charge that header exactly once and count independently retained owners separately. Accounting adds no validation or execution authority.

The two companion examples are [authoring_action_storage_v1](../crates/fe2o3-source-isa-observation/examples/authoring_action_storage_v1.rs) and [authoring_action_warm_v1](../crates/fe2o3-source-isa-observation/examples/authoring_action_warm_v1.rs). Build them and `fe2o3-author` from `fe2o3-source-isa-observation` with the repository's pinned toolchain; this report used debug binaries, whose hashes are in the evidence file.

First obtain and independently qualify a source-derived Bundle V6. Set `AUTHORING_BUNDLE` to that input file. The following commands consume an existing bundle; they do not generate a fixture or establish its source provenance:

```sh
target/debug/fe2o3-author inspect < "$AUTHORING_BUNDLE"
target/debug/examples/authoring_action_storage_v1 inspect < "$AUTHORING_BUNDLE"
target/debug/examples/authoring_action_warm_v1 inspect < "$AUTHORING_BUNDLE"
```

Use the same action arguments and identical input bytes for all three binaries. Other measured forms are `operations --bundle-identity ID --start 0 --limit 64`, `select --selector JSON`, and `materialize --selector JSON --helper budget_candidate`. The evidence contains the exact selector and arguments for this run; do not reuse those identities or coordinates for a different bundle.

Capture stdout and stderr separately. Require successful exits, exact baseline/companion stdout equality, and a complete action-matching record: `FE2O3_AUTHOR_RETAINED_STORAGE_V1` or `FE2O3_AUTHOR_WARM_ACTION_V1`. A warm record alone is insufficient because normal stdout can still fail afterward. Retain errors, timeouts and partial records without replacement. The audited parent bounded each process to 30 seconds and the complete 12-process campaign to 450 seconds.

## Remaining scope

This result does not cover source creation/publication, call-target editing, const-helper generation/validation, every authoring action, larger or worst-case fixtures, a complete persistent-session owner aggregate, compiler resume or GPU execution. It grants no source authentication or proof authority and closes no whole milestone.

Related evidence: [catalog performance](catalog-performance-20261001.md) and the tutorial site's [live debugger measurements](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/live-debugger-performance-20261001.md). Their different clocks and workload boundaries must not be combined into one latency or memory claim.
