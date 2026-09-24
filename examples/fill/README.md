# Fill

`fill` writes `42.5_f32` to each in-bounds output element reached by a launched
invocation. Its existing kernel body, `DisjointSlice<f32>` ABI and 64-thread
workgroup requirement are unchanged.

## Default Learning Workflow

The default feature set is empty. From the repository root:

```sh
bash scripts/quickstart.sh no-gpu
```

This exports the actual default fill source and checks its CPU simulation,
including untouched canaries. It does not require a protected proof runtime,
grant GPU launch authority or claim compiler-proved CPU/GPU equivalence. The
ignored `default_manifest_fill_quickstart_simulates_without_reference_proof`
backend regression executes this exact command, not a mock Cargo or substitute
kernel. Its execution for this migration remains required before merge.

The optional, nondefault `reference-proof` feature uses mutually exclusive
`cfg_attr` annotations on the same GPU function. Enabling it selects
`#[kernel(typed, reference = fill_reference, ...)]`, binding an independent safe
Rust point reference: `fn(usize, &mut f32)`. The leading argument is the logical
output coordinate; the mutable scalar corresponds to one element of the GPU
output view. The reference has its own constant store, not a call to the kernel
or a shared implementation. Its required result is exactly `0x422a0000`; this
constant assignment needs no approximate error allowance.

## Current Proof Boundary

The annotation requests the existing production reference-extraction and signed
functional-refinement path. It is not itself proof that the GPU kernel is
equivalent, and it does not supply a signed source capsule or launch authority.

Complete output coverage also requires enough launched invocations for the
output length, legal inactive dimensions and representable addresses. A guarded
store alone does not prove those dynamic premises. The compiler's conditional
coverage-to-production discharge remains incomplete; it must not be replaced by
an unconditional successful ownership result. General floating-point operation
proofs have additional limitations described in
[Functional Refinement Receipt V2](../../docs/functional-refinement-receipt-v2.md).

The focused CPU tests check only the declared reference, including exact result
bits, initial special values, empty outputs, boundary lengths and neighboring
elements:

```sh
cargo run --locked -p cargo-fe2o3 --bin cargo-fe2o3 -- \
  test --locked -p fe2o3-fill --test reference
```

Use a clean host environment without compiler/wrapper, rustdoc or Cargo runner
overrides; even empty override variables are rejected. This binding-only test
does not require protected Verus. Add `--features reference-proof` before the
test-harness separator to check the host build of that selection too; host tests
do not execute GPU proof production.

They do not execute the GPU body or establish compiler equivalence. The tutorial
manifest's fill source/contract pins have been rederived; the historical display
remains pinned to the old source and its current-source association is pending.
The [protected-effect integration test](../../docs/evidence/manifest-fill-source-proof-20260924.md)
explicitly selects and records `reference-proof`, requiring a fresh genuine
proof of this source and a failing GPU-store mutation. It is an auxiliary
selection, not the default `gfx942-fill-simulation` row (whose features remain
`[]`), and earns no qualification credit. These proof tests still require
execution in the coordinated protected environment.
Evidence for the previous unannotated source does not qualify this revision.
The compile runner and unsupported host executable are unchanged.
