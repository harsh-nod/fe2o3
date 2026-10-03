# Conditional Fill Coverage

This checkpoint advances the first verified multi-GPU Rust fill workflow. The
genuine protected compiler test now passes the dynamic coverage stage and stops
at a distinct conditional-aggregate admission gate. It does not emit a handoff,
authorize a kernel launch, complete multi-GPU application admission, or establish
HIP/HSA parity.

Parent commit: `8fade808a551e5d51b3f4a5d9c5511721e3e4d7f`.

## Implementation

The ownership pass now recognizes a closed profile in the actual materialized
PLIRON graph: one exclusive dynamic rank-one global output, one global-X index,
the exact `index < output_extent` guard, and one ordinary write at that index.
Both paths must complete normally. Unsupported operations, additional effects,
substituted extents or indices, bypasses, cycles and unreachable blocks do not
receive a conditional record. Generic ExactEffectDomain acceptance is unchanged.

A bounded topological traversal propagates four path states. Its work and storage
are linear in the bounded graph, not in the launch or output size. The recognized
write domain is `[0, min(N, G))`, each coordinate once. Total coverage still needs
the invocation condition `N <= G`. A dynamic zero sentinel is not interpreted as
an empty launch, and workgroup size or maximum grid size cannot discharge the
actual launch condition.

The private-field result stays in the ownership report, which the existing
session/stage/root transition reruns and compares before producing the lowering
owner. It retains the exact view, locations, ranked extent argument, allocation
contract and execution layout. A ranked extent argument is not a packed ABI
ordinal. Coverage also does not establish the stored-value relation.

The compiler's reference join selects a dynamic output only as a candidate, then
requires the independent classifier on the final nine-pass owner after genuine
per-output proof import. A separate conditional staging type reconciles the
owner-held effect contract, V5 evidence, policy-checked receipts and live typed
expression commitments. It retains the reference output ordinal without claiming
that this is an authenticated physical length offset.

Existing TotalView counters and the unconditional total-output gate keep their
original meaning. Conditional staging cannot be supplied to that API. The actual
compiler stops with `ConditionalAggregateRequired` before unconditional semantic
contract derivation, aggregate signing or lineage publication. No old signed
receipt is wrapped in an unsigned conditional sidecar.

## Verification Scope

The graph classifier is trusted Rust analysis, not a new protected Verus theorem.
The actual per-output value formula is verified by the pinned protected runtime.
Conditional aggregate proof, signed condition transport, source-to-machine
refinement and invocation discharge remain unfinished. Fixture signatures used
by the PLIRON staging tests grant no production authority.

## Qualification

Tests use `nightly-2026-04-03`, four Cargo jobs, no incremental/debug information,
test optimization level 1, enabled overflow/debug assertions and serial tests.

- Ownership integration: 24 passed, including symbolic and huge launch extents,
  exact DimensionOp/direct-extent forms and 18 hostile profile mutations.
- V5 evidence/staging integration: 28 passed. The new cases exercise actual
  materialized ValueAccess, reference-proof metadata and typed expressions,
  changed extent evidence, trapping control flow, absent effects, and separation
  from unconditional TotalView.
- Production ranked pipeline integration: 53 passed.
- Compiler library: 524 passed.
- PLIRON library: 195 passed, including the unchanged total-output staging
  rejection tests.
- Genuine AMD extraction under the protected runtime: both tests passed. The
  positive reaches the new conditional-aggregate gate; a changed reference value
  still fails an actual Verus assertion. Neither emits a handoff. The complete
  V9 application-admission acceptance target remains excluded and incomplete.
- Strict library Clippy passed for analysis, PLIRON and compiler with warnings
  denied. Targeted formatting and whitespace checks passed.

[Qualification archive](qualification.tar.xz), SHA256
`9b158b4d8d3437170dc33495685b4386bd02d4fb4e0a47c573bdfe94673a1655`,
contains the candidate source patch, final passing transcripts, CPU commands and
the isolated-runtime harness. The harness records this host's temporary paths;
it is reproduction evidence, not a portable deployment installer. Development
fixture errors and the disk-exhausted build are not passing qualification.

The protected manifest is unchanged, SHA256
`ffef09bd240c90e72cbff31a82bc5173c796ba7ab9af239245e7ad892c25641c`.
The exact runtime was provisioned into private namespace tmpfs at `/opt`, using
the same pinned inputs documented in
[the formula checkpoint](../dev-write-only-formula-2026-10-03/README.md).
Proofs run as UID/GID 1000 without capabilities, GPU devices or network access.
The namespace exit releases the installed runtime; host `/opt` is unchanged.

An intermediate CPU build exhausted local disk. Only this task's Cargo runtime
build cache was cleared (830.3 MiB), then qualification was rerun. No shared
MI300X processes or files were created or removed.
The task's downloaded inputs, private library tree and scratch files were removed
after archive integrity checks and terminal completion of every test process.

## Remaining Multi GPU Path

1. Derive/reconcile a conditional semantic contract and execute its protected
   aggregate proof. Carry the canonical condition in a distinct signed claim and
   decoding path; unconditional decoders must continue rejecting it.
2. Bind the coverage view to the authenticated source argument and physical
   SliceLengthU64 component. Discharge the condition against the original packed
   storage and actual AQL grid before consuming application proof custody.
3. Join the complete guarded index/address/value relation across semantic MIR,
   neutral and optimized KIR, LLVM and machine code. Complete protected application
   admission and current-publication binding.
4. Run lengths 64, 65 and 4097 through fill, staging, tracked upload, native peer
   transfer and readback in both directions on freshly observed idle devices,
   checking guards, credits, peer counters and cleanup. No GPU performance or
   native application completion is claimed by this checkpoint.
