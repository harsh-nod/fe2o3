# Workgroup synchronization V1

This standalone package defines two separate fixed `64x1x1` gfx942 source,
oracle, and formal profiles.

## LDS publish/read reduction

`lds_publish_read_reduce_i32_v1` is ordinary attributed Rust source. Every
admitted lane publishes one `i32` to its same-index LDS slot. The public
`fe2o3-device` workgroup reduction supplies the uniform publish barrier,
deterministic reduction barriers, and final reuse barrier. Lane zero is the
only global output writer. Admission rejects mathematical sums outside `i32`,
so the device's wrapping tree computes the exact mathematical sum.

The source now requests `DynamicLds::<i32>::exact_current::<64>` and
consumes that linear capability directly into collective scratch. It cannot
substitute a host/global raw pointer or expose the LDS pointer. The exact
collected compiler profile authenticates this source and its complete reachable
portable-MIR closure, then selects the closed semantic profile containing one
aligned, epoch-branded workgroup allocation shared by all lanes.

## Scoped atomic add

The second profile admits one coherent global `u32` atomic object, relaxed
ordering, system scope, and exactly 64 eligibility declarations. Eligible
lanes add once; ineligible lanes do not touch the object. Overflow is rejected
by host admission so the final value is an exact mathematical sum.

Its ordinary attributed Rust source is compiled from `src/scoped_atomic.rs`.
The signature uses `DeviceGlobalMutPtr<u32>` to state global address space
without pretending the concurrently shared object is a Rust slice. Generated
host bindings accept only a one-element initialized `u32` device region held
under an exclusive borrow; they expose neither a raw pointer nor a launch path.
The macro registration binds global address space, mutability, pointee type,
physical pointer layout, and exclusive alias admission.

## Evidence boundary

The checked CPU oracles and deterministic debug/release tests are usable now.
They fail before output mutation and reject missing or divergent barriers,
stale epochs, duplicate or wrong owners, invalid lane counts, incorrect atomic
address space, ordering, scope, target, eligibility, overflow, and substituted
outputs. Verus models initialization, convergence, epoch reuse, ownership,
exact integer sums, and atomic eligibility, with expected-negative mutations.

Both kernels are ordinary attributed Rust modules with source-level typed ABI
and LDS capabilities. Their two exact collected compiler profiles authenticate
the source, kernel root and `FnAbi`, frozen provider-terminal manifest, and
complete reachable portable-MIR closure, then select the corresponding closed
semantic Kernel IR profile. That is reviewed source-to-profile and
source-to-terminal correspondence, not generic lowering or a compiler-
refinement proof; the collected compiler paths stop before LLVM and Worker V2.
Exact compiler profile authentication therefore exists, but it does not prove
source-to-IR semantics or IR-to-machine correspondence. The artifact admission
and MI300X execution evidence remain separate bounded lanes.

A separate configured finalizer test is ignored with the exact prerequisite
`requires the measured direct LLVM/LLD worker built for gfx942`. It constructs
the bounded inert handoffs and uses the pinned upstream LLVM target-machine and
in-process LLD worker to produce both reproducible opaque COV6 admissions. The
two protected hardware tests are ignored with `requires measured direct
LLVM/LLD worker pins and gfx942:xnack-`. These independently bounded test lanes
do not prove source-to-machine correspondence, generalized memory or race
safety, or general GPU support. The production-directed finalizer uses no COMGR
and no shell linker; specifically, it does not shell out to `clang`, `llc`, or
`ld.lld`.

## Validation

```sh
cargo test --locked --manifest-path examples/workgroup_sync_v1/Cargo.toml
cargo test --release --locked --manifest-path examples/workgroup_sync_v1/Cargo.toml
cargo clippy --locked --manifest-path examples/workgroup_sync_v1/Cargo.toml \
  --all-targets -- -D warnings
VERUS=/absolute/path/to/pinned/verus \
  examples/workgroup_sync_v1/run-verus.sh
```
