# verus_vecadd and fill

This example keeps the executable path compilable as ordinary `no_std` Rust and
places the Verus proof harnesses in `verus/vecadd.rs` and `verus/fill.rs`. The
split is temporary: it lets this spike test the contract shape without adding
Verus or `vstd` to the normal Cargo dependency graph.

The executable example checks that buffers match the logical launch domain,
constructs an `IdentityWriteIndex`, and performs either one checked `u32`
addition or one fill write per thread. The Verus harness proves the
corresponding target-neutral models. For fill it establishes:

- each identity index is in bounds;
- modeled byte-address arithmetic remains below an explicit address-space size;
- distinct logical threads select disjoint singleton output locations;
- a per-thread write changes only its owned output location; and
- a completed hardware-thread set establishes the full fill postcondition.

The vecadd proof retains its bounds, disjoint-write, and frame properties.

Run the ordinary tests with:

```text
cargo +stable test --manifest-path examples/verus_vecadd/Cargo.toml
```

With a Verus binary and `vstd` available, run both positive proofs and the three
negative proof mutations with:

```text
examples/verus_vecadd/run-verus.sh --require
```

Set `VERUS=/absolute/path/to/verus` for a non-`PATH` installation. Without
`--require`, the runner reports `SKIP` and succeeds when Verus is unavailable,
so ordinary Cargo builds remain independent of Verus. A negative fixture counts
as an expected rejection only when Verus emits a postcondition-failure
diagnostic; syntax and tool failures do not masquerade as successful tests.

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
space. It does not prove pointer provenance, allocation size, Rust layout, or
that a target identity supplied the correct hardware address-space limit.

There are no `assume` or `admit` statements. This spike does not prove that the
Rust function, canonical kernel IR, AMDGPU lowering, code object, or HIP launch
refines the Verus model. It also does not model scheduling, barriers, atomics,
floating-point semantics, or arbitrary index functions. Proof records can bind
the model and environment identities as evidence, but identity equality alone
does not establish compiler refinement or create verified-artifact authority.
