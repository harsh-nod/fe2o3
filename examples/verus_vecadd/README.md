# verus_vecadd

This example keeps the executable path compilable as ordinary `no_std` Rust and
places the Verus proof harness in `verus/vecadd.rs`. The split is temporary: it
lets this spike test the contract shape without adding Verus or `vstd` to the
normal Cargo dependency graph.

The executable example checks that all buffers match the logical launch domain,
constructs an `IdentityWriteIndex`, and performs one checked `u32` addition per
thread. The Verus harness proves the corresponding target-neutral model:

- each identity index is in bounds;
- distinct logical threads select distinct singleton output locations; and
- a per-thread vecadd write changes only its owned output location.

Run the ordinary tests with:

```text
cargo +stable test --manifest-path examples/verus_vecadd/Cargo.toml
```

With a Verus binary and `vstd` available, verify the harness with:

```text
verus --crate-type lib examples/verus_vecadd/verus/vecadd.rs
```

## Trusted boundary and limits

`hardware_thread_id` in the Verus harness is marked
`#[verifier::external_body]`. Its postcondition is the explicit trust boundary:
the eventual backend/runtime must supply one logical ID below `thread_count` for
each active hardware thread. The race argument additionally relies on different
active hardware threads receiving different logical IDs; the harness proves the
identity mapping is injective once those IDs differ.

There are no `assume` or `admit` statements. This spike does not model scheduling,
barriers, atomics, floating-point semantics, arbitrary index functions, or the
correctness of MIR/LLVM lowering. It is not a complete SIMT verifier.
