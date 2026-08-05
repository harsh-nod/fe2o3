# verus_vecadd and fill

This example keeps the executable path compilable as ordinary `no_std` Rust and
places the Verus proof harnesses in `verus/vecadd.rs` and `verus/fill.rs`. The
split is temporary: it lets this spike test the contract shape without adding
Verus or `vstd` to the normal Cargo dependency graph.

The executable algorithm remains only in `src/lib.rs`: the proof files add
target-neutral specifications and ghost lemmas, not another executable kernel
body. The example checks that buffers match the logical launch domain,
constructs an `IdentityWriteIndex`, and performs either one checked `u32`
addition or one fill write per thread. For fill, the Verus harness establishes:

- each identity index is in bounds;
- modeled byte-address arithmetic remains below an explicit address-space size;
- distinct logical threads select disjoint singleton output locations;
- a per-thread write changes only its owned output location; and
- a completed hardware-thread set establishes the full fill postcondition.

The vecadd harness models each access as a symbolic allocation identity,
address space, and half-open byte region. It establishes:

- logical bounds and byte-address representability for both reads and the
  output write;
- compatibility of the two shared reads, including exact input aliasing;
- incompatibility of an overlapping exclusive write and shared read;
- pairwise-disjoint exclusive output writes for distinct identity indices; and
- frame behavior for untouched output elements and other allocations.

Run the ordinary tests with:

```text
cargo +stable test --manifest-path examples/verus_vecadd/Cargo.toml
```

With a Verus binary and `vstd` available, run both positive harnesses and the
six negative proof mutations with:

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
new `external_body`. This spike does not prove that the Rust function,
canonical kernel IR, AMDGPU lowering, code object, or HIP launch refines the
Verus model. It also does not model scheduling, barriers, atomics, fractional
read tokens, floating-point semantics, or arbitrary index functions. Proof
records can bind the model and environment identities as evidence, but identity
equality alone does not establish compiler refinement or create
verified-artifact authority.
