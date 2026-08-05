# Verus vecadd and fill source models

The primary milestone in this directory verifies the executable body of the
real `f32` `#[kernel]` in `examples/vecadd`. The operation, index extraction,
guarded `DisjointSlice::get_mut`, two input indexes, addition, and output write
live once in `examples/vecadd/src/vecadd_body.rs`, with arithmetic supplied by
an explicit adapter. Both the GPU kernel and `verus/vecadd.rs` mechanically
expand that control/index/read/write fragment.

The fragment has two explicit adapter boundaries. The GPU expansion calls
`fe2o3_device::thread::index_1d()` with no argument, while the Verus expansion
passes a modeled launch witness to `model_gpu_thread::index_1d`. The adapter
returns that same identity witness and introduces no `external_body`. Proving
that the target intrinsic returns the corresponding launch witness remains a
backend-refinement obligation. The production arithmetic macro expands to the
exact expression `lhs + rhs`; a source-shape test and the `kernel-ir-v1`
compile lane enforce that fact. Verus instead expands the same arithmetic
adapter position to a total operation over local `ModelFloat` tokens.

For the real shared body, Verus establishes:

- an arbitrary rounded-up thread performs no input index or output write when
  `DisjointSlice::get_mut` rejects its identity witness;
- an in-range witness is in bounds for the output and both equal-length inputs;
- arbitrary in-range model operands require no arithmetic-domain premise;
- `ThreadIndex::get` and the consuming output access select the same index;
- distinct identity witnesses select distinct output elements;
- the guarded write changes no other modeled output element;
- symbolic input-read and exclusive-output regions are compatible; and
- every modeled four-byte element address ends within both its allocation's
  address space and `usize::MAX`.

The `ModelGpuDisjointSlice` adapter owns a `Vec<ModelFloat>` so Verus can reason
about the shared body's mutation and frame behavior for arbitrary values. A
model add XORs opaque tokens to provide a total executable operation, but no
contract exposes that result. Neither `ModelFloat` nor the model add is claimed
to refine IEEE `f32` or production addition. `ModelGpuDisjointSlice` is also not
a refinement of the raw pointer and length stored by
`fe2o3_device::DisjointSlice`. Allocation IDs, base addresses, extents, and
permissions remain caller-supplied ghost facts; the harness does not
authenticate them against Rust references or a launch.

`real_kernel_arbitrary_in_range_operands_are_memory_safe` exposes the in-range
bounds, ownership, and frame guarantees without constraining model operand
values. `real_kernel_rounded_tail_is_noop` composes for an out-of-range thread
without arithmetic or region-evidence premises. No claim is made about the
stored sum, and memory-safety proofs do not depend on the arithmetic result.
The real-body model adds no `assume`, `admit`, or `external_body`.

## Reference proofs

The older `u32` CPU/reference vecadd remains separate in
`src/vecadd_body.rs`, `src/lib.rs`, and the first part of `verus/vecadd.rs`. It
proves an exact per-thread integer result under an explicit no-overflow
precondition. It is not the GPU kernel and is retained as a stronger
target-neutral functional example, not as evidence for `f32` semantics.

The fill harness additionally proves identity indexing, modeled address
representability, disjoint writes, frame behavior, and its launch-level fill
postcondition. Its `hardware_thread_id` model is the one existing
`#[verifier::external_body]`; the real vecadd source model does not call it.

## Running the checks

Run the ordinary Rust tests with:

```text
cargo +stable test --manifest-path examples/verus_vecadd/Cargo.toml
cargo test -p fe2o3-vecadd
```

Run both positive Verus harnesses and all twelve expected proof rejections with:

```text
VERUS=/absolute/path/to/verus examples/verus_vecadd/run-verus.sh --require
```

The real-kernel negative mutations independently reject an input read moved
ahead of the output guard, a real shared-body expansion through a constant-zero
thread adapter, and output/input allocation aliasing. For these fixtures the
runner requires the exact Verus error class and failed source clause in addition
to a stable marker; parser and unrelated proof failures do not pass.

## Remaining refinement gap

This is source-model evidence, not machine-code verification. It does not yet
prove that the model thread witness is the value returned by the AMDGPU
intrinsic, that `ModelGpuDisjointSlice` refines the actual raw device pointer,
that production `f32 +` refines the total model arithmetic adapter, that ghost
allocation facts came from admitted runtime arguments, or that the shared Rust
expansion refines canonical Kernel IR, LLVM, HSACO, and execution. It does not
create or upgrade runtime `Verified` authority. Closing that gap requires
authenticated compiler-generated proof bindings and a refinement chain from
the real types, intrinsic, and arithmetic adapter through the loaded artifact.
