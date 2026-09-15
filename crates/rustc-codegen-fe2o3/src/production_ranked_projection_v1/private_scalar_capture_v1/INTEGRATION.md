# Private Shared Scalar Capture Checkpoint

Confirmed mixed20 source observation:
`all47-source-report-mixed20/report.json`, `gfx942-wave64-collectives`,
`diagnostic.textTruncated=false`: expanded bb77 statement 1, local 119,
type 40 dereferenced to type 31, Read, no allocation/private/checked origin.
The checked origin is instance 11, function 7, block 4, statement 1, local 6,
source `bad40ed3227c:50:64`, caller instances 11 -> 3 -> 1 -> 0.

## Mounted Scope

- New `private_scalar_capture_v1.rs` and `private_scalar_capture_v1/state.rs`.
- The ranked root obtains a classifier only from its existing SSA owner and
  checked execution view. Parameter/return transfers and frame storage events
  remain those of the replayed expansion; no independent capability graph.
- One optional classifier argument on `project_statement_accesses`, with the
  existing three test callers explicitly opting out. The destination is still
  audited before classifying the sole RHS private scalar read.
- New `private_scalar_capture_tests.rs`, twelve internal flow/budget tests and
  one final-read budget test; filter `private_scalar_capture_` selects nineteen
  owned tests (in addition to parent-owned regressions).
- No MIR, KIR, SSA adapter, lowerer, backend, or simulator schema edits.

## Closed Behavior

Only an exact retained `scalar_local = copy(*shared_reference)` assignment can
be classified. The pointee must be an initialized primitive bool, integer, or
FP32/FP64 scalar local, not a root argument, capability, raw pointer, or global
allocation. Reference types must be ordinary metadata-free Rust references.
Origin transport supports typed tuple/aggregate fields, Copy/Move, private
borrows, and checked shared reborrows of private mutable environment references.
Reference origins never come from pointer layout, an opaque value, or an
intrinsic result. The existing global singleton guard is unchanged.

Sparse CFG states track initialization, dead storage, exact reference origins,
and escapes. Ambiguous joins lose reference origins; writes/moves/storage kills
invalidate aliases. Raw address/cast and opaque-call escapes cannot retain
private reference proof. Calls initialize results only on CallReturn edges.
Classification is published after fixed-point convergence, never on a transient
visit. Limits remain one million work units, 131072 simultaneously live logical
storage nodes, sixteen aggregate fields, and depth eight. Exhaustion discards
every tentative result and leaves the original ranked rejection in force.

## Follow-Up To Compiler21c

The five checked-owner tests failed at fixture admission, not analysis. Their
eight-byte memory aggregate incorrectly used Rust Indirect argument passing;
`validate_abi_value` rejects that mode for sized Rust memory aggregates <=8
bytes. The fixture now uses an eight-byte integer Cast with plain attributes,
matching its unchanged memory layout and default noundef property. No ABI
validator or production importer was changed.

Compiler22b still rejected fixture admission because the reused neutral
reference-type helper describes ZST pointees (guaranteed size zero). The
fixture's frozen shared reference to f32 requires size 4; its mutable/shared
environment references require size 8. `validate_type_abi_properties` requires
these exact sizes even when the value is only a local. The scoped fixture helper
now records sizes/alignments 4/4 and 8/8, and the leaf's Direct shared-reference
argument carries size 8/alignment 8 with its existing frozen-reference flags.
A new independent fixture test checks all three type records against their
actual pointee layouts and the leaf argument against that type record.

Storage is now preflighted against a shared reservation counter. Retained block
states coexist with source and successor clones; joins include both inputs,
result and target-set scratch. Final reads count map entries plus references
while unvisited entries and the active flow remain live. CFG reservations cover
entry/queued arrays, a block-capacity pending queue and two successor pairs per
block; successors stay a set instead of creating a simultaneous result vector.
Operand/capture reservations include retained fields and moves. Escape checks
include the reference roster, bounded pending capacity, nested sets and poison
growth. Value node counts include unused field-vector capacity. Reservations
release on errors, and failed growth does not change the counter or read map.
This is a conservative logical-node ceiling, not an allocator byte measurement;
join inputs already retained may be counted twice, never omitted.

The initial storage-event scan, argument roster, reference/empty-field walks,
operand rosters and set transfers also consume the unchanged work budget.
Seven new tests cover exact ceiling/+1 and overflow, source/successor/value
clones, join scratch, nested cleanup, reference-roster work and final-read
accumulation. The new checkpoint is separate from compiler21c's initial mount.

Review after compiler22b found no additional missing live-state window in this
accounting. Escape's pending capacity is reference-count + 1: every stored
reference edge is enqueued at most once after its containing local is first
visited. The four-reference-count scratch allowance covers pending, visited,
nested, and newly escaped entries together. Joins cover result and killed sets
while both inputs remain live. Captured field capacity remains reserved across
operand evaluation, and moved values remain reserved across destination kills.
The work ceiling remains conservative, including both initialization scans and
reference rosters; the real wave64 workload must still establish that it fits.

## Coordination And Qualification

Pauli continues to own Context SSA and capability authority. This classifier
only accounts for a private read in the ranked memory-effect audit. Original
source MIR, reference operands, assignments, and value lowering remain intact;
downstream SSA/KIR must still lower the actual read or reject it. It grants no
numerical, convergence, epoch, launch, or artifact proof.

No Cargo/build/export was run by this worker. Central compilation, the nineteen
owned tests, and the real wave64 rerun remain the integration gates. Compiler21c
reported 910 pass / 6 fail / 35 ignored on the initial mount; five failures were
the fixture ABI above, and one was the parent's protected OpenSSL environment
failure. No actual wave64 success is claimed before the rebuilt exporter rerun.
Compiler22b reports all twelve listed internal capture flow/resource tests pass,
with the same five fixture-admission failures; its log does not list the mounted
source/successor-clone test, which therefore still needs central execution.

## Core25 And Mixed24

Parent reports Core25 passes all nineteen owned private-capture tests. The full
compiler result is 928 pass, one protected environment failure, and 58 ignored.
The final fixture correction casts its shared f32 reference into a distinct
typed raw-pointer local. That case now requires successful SSA construction
before asserting zero classified reads, so canonical rejection cannot satisfy
the capture-analysis test. No validator or production classifier was weakened.

The frozen mixed24 exporter run clears the original Wave64 private-capture
gate. `gfx942-wave64-collectives` now fails later at an exact Global write
contract request (`v11 bb45 op0`). This is source-stage progress, not a bundle
or launch proof. Parent reports all47 still fail, zero bundles and cleanup true
for all47; no broader qualification is claimed. The private-capture production
files remain frozen while the matrix constructor checkpoint proceeds.
