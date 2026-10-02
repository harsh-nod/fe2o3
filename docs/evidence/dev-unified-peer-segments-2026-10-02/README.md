# Unified Multi GPU Peer Segments

Implementation baseline: `ead9cfab9501d8332bf3f89485dfdafbea51eb7c`.

## Scope

The ordered segment API now runs on `KfdMultiDeviceRuntimeBackendV1` for settled,
initialized PUBLIC DeviceLocal allocations on an admitted native route. There
is no silent host-staged fallback. The immutable checked plan validates original
logical extents, independent bounding envelopes, all descriptors and packet
bounds before owner extraction. Nested cursors avoid a flattened allocation of
up to 4096 descriptors times 4096 packets.

A single native queue, mapping pair and original allocation-owner pair remain
retained through the entire ordered list. Observation does not publish later
packets. Explicit progress publishes at most one next packet; only final native
completion, retirement and restoration settle the logical result and event.
Duplicate and overlapping destinations retain caller order. Admitted errors or
unwinds retain terminal custody and may leave an applied prefix; this is not an
atomic transaction. The scalar path and existing copy-only backend remain.

Context and owned async callers retain one list result and immutable descriptor
snapshot. Pending list outputs cannot impersonate scalar producer coverage.
Pending compute sources and segmented-output deferred compute/readback remain
unsupported until distinct whole-list producer provenance is implemented.

## CPU Qualification

The final source inventory passes all 1943 KFD library tests in five disjoint
exact-roster shards (82/81/81/81/1618), with no ignores or failures. All 2227
runtime tests pass with 32 unchanged hardware ignores, identified against the
accepted baseline roster rather than counted as native passes. The new witness
passes three tests and the existing deferred witness passes thirteen. All 114
KFD/runtime doctests, strict all-feature library/test and witness Clippy,
no-default library checks, changed-Rust formatter and whitespace checks pass.

Nine new KFD tests cover checked plans and actual composed native-owner
adapters, including ordered overlap, multi-packet tails, ring reuse, extent
drift, final-only restoration and injected error/unwind custody. Twelve new
runtime tests cover Context and owned async lists, snapshot mutation, bounded
fairness, disjoint GPU pairs, logical padding, cancellation, terminal prefix
faults and rejection of unsupported pending-producer composition before effects.
Scripted failures are not native device-fault qualification.

## Native Qualification

All ten final-source MI300X cases pass: 1, 4, 65 and 4096 descriptors plus a
three-descriptor/five-packet case, each in both directions between GPUs 6 and 7.
The unchanged R57 qualification constructor admits the devices; no module or
kernel is loaded or launched. Every case checks cancellation before publication,
irreversible cancellation after retained publication, descriptor mutation after
submission, final-only event completion and a logical native count of 0 -> 1.
Full logical source and destination bytes, including untouched guards and
ordered overwrites, match an independent Python oracle and framed SHA-256.

The ten successful lists comprise 8338 descriptors, 8342 planned packets and
16861502 useful descriptor bytes. Ten additional unpublished lists cancel
without changing destination bytes. Each process explicitly releases results,
events, allocations, streams, Context and native resources before reporting PASS.
Each case is a separate process; this does not qualify repeated live-Context
lists or pending compute/list/compute dataflow.

The exact native executable is 7790784 bytes, SHA-256
`7e5334eb5f80b645c03d13290dd576034b4edcebf564c58501fe2f41b4cbcc28`.
Its build, CPU, proof, source-control and hardware receipts bind the same 6129
captured source/build/config files. Documentation is outside that inventory.
The offline native audit validates all 96 transport receipts, exact arguments,
chronology, full report schema, independent byte oracle and executable identity.

Fresh UID/BDF, utilization, VRAM, process-roster, device-attachment and host-memory
checks precede and follow every case. These are point observations, not an
exclusive reservation or continuous census. Only GPUs 6/7 were selected; foreign
work was untouched. The exact owned executable was confirmed inactive,
hash-checked and removed with its scratch directory. Directory absence and final
device/process baseline observations pass. No reset, foreign process termination
or broad temporary-directory cleanup was used.

## Proof and Source Controls

All 32 no-solver source controls pass on the baseline and final snapshot. The
reviewed metadata refresh changes 21 hash literals and seven source counts in
13 checker files. Checker predicates and all 76 existing executable proof files
are unchanged. Two KFD export-file identity updates are restricted to exact
reviewed module/export additions, not a general source-drift exception.

Fresh pinned Verus campaigns verify the existing shared packet arithmetic and
checked-window arithmetic. The packet campaign has 13 stages, five verified
functions, eight rejected logical mutants and five runner controls. The window
campaign has 16 stages, three verified functions, eleven rejected logical
mutants and seven runner controls. Both include positive-before/after proofs,
all negatives and before/after release-closure checks. These cover arithmetic,
not the new nested descriptor/packet composition, ownership, DMA, Context or
machine-code refinement. The R74 model/proof is unchanged and not rerun here.

## Evidence and Limits

`audit-final.json` accepts exact source continuity, test rosters, baseline
controls, metadata restrictions, full proof campaigns and native audit linkage.
The packet retains early compile diagnostics, the rejected first shard parser,
explicitly stopped owned test runs and symlinked-verifier-path rejections. None
counts as accepted qualification. Eleven offline parser tests check the narrow
paired subprocess-header correction. Final campaigns rerun against the final
source after the witness hexadecimal-formatting fix; no source-delta or binary
equivalence exception is used.

General application-kernel authority, native fault isolation, whole-adapter
formal refinement, physical overlap and matched HIP/HSA performance remain
open. Retaining queue/mappings within a list removes repeated setup work but
does not by itself establish any speedup. Logical guards do not inspect hidden
physical pool padding. This checkpoint does not close A3 or establish parity.

`raw.tar.xz` contains commands, outputs, inventories, controllers, independent
audits, diagnostics and the staged source patch. `raw-manifest.json` hashes
every archive member; `SHA256SUMS` binds this README, manifest and archive.
No native executable is distributed.
