# verus_vecadd and fill

This example keeps the executable path compilable as ordinary `no_std` Rust
without adding Verus or `vstd` to the normal Cargo dependency graph. Vecadd is
single-sourced in `src/vecadd_body.rs`: ordinary rustc expands that body from
`src/lib.rs`, and Verus expands the same body from `verus/vecadd.rs`. There is no
second executable vecadd algorithm in the proof harness.

The shared body checks the logical launch extent, constructs an
`IdentityWriteIndex`, checks `u32` addition overflow, and performs one output
write. The Verus harness surrounds that exact expansion with target-neutral
specifications and ghost evidence. For fill, the separate reference harness
establishes:

- each identity index is in bounds;
- modeled byte-address arithmetic remains below an explicit address-space size;
- distinct logical threads select disjoint singleton output locations;
- a per-thread write changes only its owned output location; and
- a completed hardware-thread set establishes the full fill postcondition.

The vecadd harness models each access as a symbolic allocation identity,
address space, and half-open byte region. A ghost launch brand connects a 1D
index-space extent to one branded thread witness. Initialized shared-read
capabilities cover both inputs, and an exclusive capability covers the output
element. The harness establishes:

- logical bounds and byte-address representability for both reads and the
  output write;
- compatibility of the two shared reads, including exact input aliasing;
- incompatibility of an overlapping exclusive write and shared read;
- pairwise-disjoint exclusive output writes for distinct identity indices; and
- frame behavior for untouched output elements and other allocations;
- initialization of the output element after the shared body writes it; and
- exact per-thread `u32` vecadd behavior plus full functional correctness when
  a complete branded launch supplies every identity index.

Run the ordinary tests with:

```text
cargo +stable test --manifest-path examples/verus_vecadd/Cargo.toml
```

With a Verus binary and `vstd` available, run both positive harnesses and the
nine negative proof mutations with:

```text
examples/verus_vecadd/run-verus.sh --require
```

Set `VERUS=/absolute/path/to/verus` for a non-`PATH` installation. Without
`--require`, the runner reports `SKIP` and succeeds when Verus is unavailable,
so ordinary Cargo builds remain independent of Verus. A negative fixture counts
as an expected rejection only when Verus emits both the fixture's stable
proof-function marker and a postcondition-failure diagnostic. Syntax and tool
failures do not masquerade as successful tests.

## Trusted boundary and limits

`hardware_thread_id` in each Verus harness is marked
`#[verifier::external_body]`. The fill contract explicitly requires a 1D active
launch slot to observe the same global logical ID. Consequently the set of
active slots is in bounds, unique, and covers the logical domain. Passing the
active slot to the external function is ghost modeling for this spike, not the
signature of a GPU intrinsic. The backend, launch geometry, and runtime must
eventually refine that model.

The address proof uses mathematical naturals and an explicit exclusive
`address_space_size`. It proves the modeled element range cannot overflow that
space. Allocation IDs, extents, base addresses, address spaces, and element
sizes are symbolic launch-environment inputs. This slice does not authenticate
those inputs, connect them to Rust references, or create linear runtime tokens.
It therefore does not independently prove pointer provenance, Rust layout, or
that the target supplied the correct address-space limit.

There are no `assume` or `admit` statements, and the region model introduces no
new `external_body`. Verification of the mechanically shared Rust body is
source-model evidence only. It does not prove machine-code refinement from the
Rust expansion through canonical kernel IR, AMDGPU lowering, a code object, or
a HIP launch. It also does not model scheduling, barriers, atomics, fractional
read tokens, floating-point semantics, or arbitrary index functions. The ghost
brand and region capabilities are erased proof inputs; they neither mint nor
upgrade runtime `Verified` authority. Proof records may bind this evidence to
external identities, but identity equality alone cannot authorize loading or
launching an artifact.
