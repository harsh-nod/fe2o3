# Kernel IR V12 Verification

Status: shared compiler infrastructure, not production V12 graph optimization,
target lowering, or formal qualification.

## One Semantic Verifier

The public Module verification APIs and exact canonical V12 construction use
the same semantic engine in
[verification_engine_v1.rs](../crates/fe2o3-kernel-ir/src/verification_engine_v1.rs).
There is no V12-specific admission selector or alternate permissive verifier.
Raw borrowed modules first receive iterative type-depth preflight; freshly
decoded modules reuse the decoder's depth bound.

The engine checks identities and roles, declarations and capabilities, CFG
structure, SSA definitions and uses, operand/result types, and operation
contracts. Registered operations use the same diagnostic collector as core
operations. Passing these checks means that an IR object is well formed under
the modeled contracts, not that its source, transformations, physical execution,
bounds, race freedom, or algorithm have been proved correct.

V12 adds these structurally checked carriers:

- Fixed vectors with 2 through 1,024 lanes, byte-sized numeric scalar elements,
  and contiguous or valid interleaved layouts. Pointer and slice element types
  may contain a vector even when a downstream consumer cannot lower it.
- Vector loads and stores with explicit scalar-pointer provenance, matching
  vector descriptors and SSA types, access mode, address space, and alignment.
  Layout conversion preserves logical element/lane types and changes the
  physical layout.
- Ordered workgroup-pipeline verification events with no SSA results, a
  workgroup pointer, and an index-typed epoch operand. A catalog key is a
  locator, not authenticated evidence that a pipeline contract holds.

The raw V12 codec remains distinct from semantic admission. It can round-trip
structurally encodable but semantically invalid modules.
[VerifiedCanonicalKernelIrV12](../crates/fe2o3-kernel-ir/src/canonical_kir_v12.rs)
requires exact V12 bytes and fresh semantic verification before minting its
version-bound identity. Older encoders and canonical owners keep their frozen
rejection boundaries; accepting V12 types in the shared verifier does not make
them representable by older wire schemas.

## Resource Boundaries

The API name matters. These entry points share semantics, but do not promise
identical resource coverage:

| API | Work coverage | Storage coverage |
| --- | --- | --- |
| Public compatibility APIs such as verify_module and verify_module_ref | Shared checks, without a caller-selected aggregate budget | No caller-visible storage bound; compatibility resource-error formatting is outside the bounded API claim |
| verify_module_ref_with_budget_v1 | Raw-module depth preflight and semantic verification on the caller's cumulative work ledger | Verifier-local scratch and diagnostic materialization; excludes the caller-owned Module |
| VerifiedCanonicalKernelIrV12::from_module_with_work_budget_v1 | Canonical encoding, decoding, equality, and hashing | No allocation bound; semantic work is not charged to this caller ledger |
| from_module_with_resource_budget_v1 and from_module_ref_with_resource_budget_v1 | Canonical work plus semantic verification on one work ledger | Returned semantic receipt covers verifier-local storage, excluding source, decoded Module, and wire owners |
| from_module_ref_with_verification_budget_v12 | Fresh count/encode/decode/verify/equality/hash transaction on one caller ledger | Also accounts for encoder scratch, wire ownership, requested decoded heap payload, and their coexistence with verifier scratch; excludes the borrowed input Module |

The verifier-local receipt's work field describes the semantic verification
interval, not the complete canonicalization transaction. The complete
allocation-budgeted API instead leaves cumulative work, peak storage, and
rejected reservations observable on the supplied ledger and returns a
retained-storage transfer receipt for its canonical owner.

Ordinary Result exits restore the incoming live-storage floor; accepted work
and peak observations remain. Returned diagnostics or canonical bytes transfer
to the caller. Before further budgeted allocations coexist with the returned
canonical owner, the caller reserves its retained-storage receipt. Resource
failure can precede a semantic diagnostic; a rejected charge preserves the
accepted prefix.

These are explicit logical payload conventions, not RSS, allocator metadata,
an operating-system memory limit, or a formal proof of resource safety. Fixed
verifier structures use explicitly defined logical row/cell charges; diagnostic
rows use machine-word-sized cells. Dynamic byte buffers and allocation-aware
owner payloads retain their documented units. Arithmetic is checked; overflow
sentinels must not be interpreted as exact mathematical totals.

Logical precharging does not make every host allocation fallible. Pointer and
slice boxes and B-tree nodes still use infallible standard-library allocation;
host out-of-memory can abort even when the logical reservation was admitted.
Fallible Vec/String paths report allocation errors, but these APIs do not claim
universal host-OOM recovery.

## Borrowed Locations, Owned Errors

Public ModuleId, FunctionId, KernelId, and DiagnosticLocation remain
String-backed and owned. Internal
[VerificationDiagnosticLocationV1](../crates/fe2o3-kernel-ir/src/verification_diagnostics_v1.rs)
is Copy and borrows identifier owners from the input Module. Location factories
and copies retain their explicit five-field work charge; they do not clone
identifier strings.

The collector first counts diagnostics and formats messages into an
allocation-free byte counter. Successful verification does not allocate
diagnostic identifiers or messages; this does not mean the verifier needs no
other scratch storage. An invalid-module materialization pass reserves the
complete diagnostic roster and copies identifier contents only for errors that
will be returned.

For one materialized diagnostic:

- R = ceil(size_of(Diagnostic) / size_of(usize)) fixed-row cells.
- L = sum of the visible lengths of its present identifiers; k is their count.
- C = 5 + L + 2*k identifier-copy work.
- U is the supplied message-work bound; M is the counted message byte length.
- Count charges 1 + U. Materialization charges 1 + U + C + U + M + 1.
- A roster of n errors retains n*R; each pending error reserves L + M additional
  bytes while the roster and previously materialized buffers remain charged.

Identifier copies use fresh fallible exact reservations and retain visible
bytes, not spare capacity from the source String. Exact-capacity assumptions
belong to the pinned Rust allocation contract; they are not an allocator-RSS
claim. A bounded formatter rejects oversized chunks before append and latches
failure, including when a Display implementation ignores fmt::Error. Divergent
message lengths reject instead of growing an uncharged buffer.

Partial construction drops pending strings before releasing their reservation.
Sorting retains the full roster and all identifier/message buffers; failure
and abandonment drop owners before releasing storage. Successful completion
explicitly transfers the sorted owned diagnostics to the caller, independently
of the input Module's lifetime.

## Consumer Admission Remains Narrower

Physical memory effects and compiler ordering are separate axes.
Interprocedural summaries propagate both through calls. A marker-only helper
can have an empty memory summary while still being impure for compiler
ordering; is_complete_and_pure requires completeness and both axes to be empty.
Incomplete declaration, recursion, assembly, and resource-limit decisions stay
incomplete. Memory summaries preserve one multi-address-space synchronization
event and keep fences distinct from execution barriers.

Region extraction uses each vector descriptor's full byte width, pointer,
address space, alignment, and caller-supplied epoch. Layout conversion invents
no physical memory effect. Region bindings remain untrusted analysis inputs,
not launch or proof authority. Direct verification events make this
memory-only report incomplete because it cannot represent compiler ordering;
indirect events are reported through unavailable call effects.

The following boundaries remain closed:

- AMDGPU raw-Module entry points reject V12 vectors and verification events
  before LLVM emission, including unused declarations, nested types,
  unreachable blocks, dead results, and types embedded in legacy operations.
- Existing Pliron graph import, legacy optimizer admission, simulation
  containers, and simulator execution retain their unsupported-V12 boundaries.
- Formal-memory extraction does not admit vector operations or verification
  events as completed modeled effects. A call to a marker-bearing helper
  cannot be skipped as a pure call.
- No catalog-key authentication, V12 production graph migration, V12
  optimization or target legalization, tutorial-wide compiler qualification,
  hardware execution, or formal compiler verification follows from this
  shared-verifier boundary.

## Canonical Transition Receipts

The [transition receipt codec](../crates/fe2o3-kernel-ir/src/canonical_kir_transition_receipt_v1.rs)
serializes the fixed scalar/CFG checker's occurrence map, not an executable or
a proof of compiler correctness. Its move-only `InertCanonicalKirTransitionReceiptV1`
owns nine typed row slices and their canonical bytes. Endpoint digest/length
pairs are inert locators; decoding cannot create a verified graph identity.

The [analysis admission API](../crates/fe2o3-kernel-analysis/src/canonical_kir_transition_receipt_v1.rs)
compares these locators against two actual, already verified inventories, then
runs the existing transition checker on those inventories and the decoded rows.
The resulting checked view borrows both supplied inventories and the receipt.
Matching endpoint hashes alone do not admit the transition, authenticate original
source-owner custody, or establish a formal semantic-refinement theorem.

The wire schema fixes V12 endpoints and checker policy 1. All integers are
little-endian; the 132-byte header contains:

```text
magic[8] = "F2NTR1\0\0"
schema:u16 = 1 | checker_policy:u16 = 1 | total_length:u32
input_digest[32] | input_canonical_length:u64
output_digest[32] | output_canonical_length:u64
n_functions:u32 | n_blocks:u32 | n_segments:u32 | n_operations:u32
n_definitions:u32 | n_definition_outputs:u32 | n_uses:u32
n_edges:u32 | n_edge_arguments:u32
```

The nine slices follow this count order. Their fixed row widths are respectively
8, 16, 24, 36, 28, 24, 40, 24, and 32 bytes. Coordinates use function/block/operation
ordinals and successor occurrences, not raw pointers, host enum layouts, or
debug strings. Repeated edges to the same block remain distinct. The
[row grammar](../crates/fe2o3-kernel-ir/src/canonical_kir_transition_receipt_v1_rows.rs)
defines the closed coordinate, origin, connector, and descendant tags.

Unknown tags or policies, nonzero padding, malformed count/length products,
truncation, trailing data, and nonpartitioning ranges reject. Block ranges must
be nonempty and partition all segments; definition ranges partition descendants
and may be empty only at the current cursor. Graph-dependent coverage and rewrite
legality are still checked by analysis, not by the codec.

The complete frame is capped at 4 MiB. An enclosing association must additionally
enforce its own aggregate limit; a maximum-size row receipt need not fit beside
other association data. Its inert digest is SHA-256 of the 39-byte domain
`FE2O3/CANONICAL-KIR-TRANSITION-ROWS/V1\0`, the frame length as u64, and the frame.

For frame length L, total row count N, block-plus-definition count Q, and domain
length D=39, the codec's logical resource contract is:

```text
retained_storage = sizeof(receipt) + L + sum(count_i * sizeof(typed_row_i))
encode_work = 1 + Q + 2*L + N + D + 8
decode_work = 1 + 3*L + N + Q + D + 8
```

The complete retained storage is reserved before owned allocation, and every
returned Vec capacity must equal its requested count. Caller-owned input bytes
or rows remain separately reserved. Ordinary errors and caught unwinds restore
the incoming storage floor after dropping codec-owned temporaries. Success
transfers an explicit retained-storage receipt for reservation before subsequent
controlled allocation. Work, peaks, and failure history accumulate. These are
logical payload bounds, not allocator metadata or RSS limits. Analysis adds 80
work units for the two endpoint comparisons before invoking the unchanged checker.

This infrastructure does not activate a production optimizer, admit new host
proof formats, transport source contracts by itself, or establish verified output
memory. Those require their own complete replay and admission paths.

## Regression Targets

These commands exercise this boundary; listing them is not a passing test
receipt or hardware/formal qualification:

~~~sh
cargo test --locked -p fe2o3-kernel-ir --lib verification_diagnostics_v1
cargo test --locked -p fe2o3-kernel-ir --lib canonical_kir_v12
cargo test --locked -p fe2o3-kernel-ir --lib canonical_kir_transition_receipt_v1
cargo test --locked -p fe2o3-kernel-ir --doc InertCanonicalKirTransitionReceiptV1
cargo test --locked -p fe2o3-kernel-analysis --lib canonical_kir_transition_v1
cargo test --locked -p fe2o3-kernel-ir --test inert_v12 --test vector_v12 \
  --test verification_contract_v12 --test wire_preflight_order \
  --test v12_effect_consumer_closure
cargo test --locked -p fe2o3-amdgcn-model --lib v12_preflight
cargo test --locked -p fe2o3-amdgcn-model --test v12_consumer_rejection
cargo test --locked -p fe2o3-kernel-opt --test inert_v12_admission
~~~

The focused tests include borrowed/owned diagnostic lifetimes, long and
spare-capacity identifiers, exact and one-under budgets, partial construction
and formatting failures, canonical inverse checks, vector effects, marker
propagation, and explicit backend rejection. Repository-wide validation and
ordinary-source compiler-to-KFD evidence are separate qualification steps.
