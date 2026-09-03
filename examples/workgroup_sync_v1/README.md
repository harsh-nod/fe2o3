# Workgroup synchronization V1

This standalone package defines two fixed `64x1x1` kernels with CPU oracles
and formal contracts. The LDS reduction is target-neutral through semantic MIR
and Kernel IR, then binds independently to the production compiler profile for
gfx942 or gfx950.

## LDS publish/read reduction

`lds_publish_read_reduce_i32_v1` is ordinary attributed Rust source. Every
admitted lane publishes one `i32` to its same-index LDS slot. The public
`fe2o3-device` target-neutral workgroup reduction supplies the uniform publish
barrier, deterministic reduction barriers, and final reuse barrier. Lane zero
is the only global output writer. Admission rejects mathematical sums outside
`i32`, so the device's wrapping tree computes the exact mathematical sum.

## Inclusive and exclusive scans

The `lds-scan-u32-kernel`, `lds-scan-i32-kernel`, and
`lds-scan-f32-kernel` features compile ordinary attributed Rust examples using
the same affine dynamic-LDS and workgroup-authority contract. The public
`inclusive_scan_sum` and `exclusive_scan_sum` terminals order prefixes by
linear work-item rank and return one result per lane through a typed
`DisjointSlice`. Their exact compiler, CPU simulation, schedule replay, and
debug evidence contract is documented in
[`target-neutral-workgroup-scan-v1.md`](../../docs/target-neutral-workgroup-scan-v1.md).

The source now requests `DynamicLds::<i32>::exact_current::<64>` and passes
that linear capability directly to the collective terminal. It obtains
`WorkgroupCollectives::current()` and calls
`WorkgroupCollectives::reduce_sum_portable`; neither operation names or selects a GPU
family. It cannot substitute a host/global raw pointer or expose the LDS
pointer. The production importer authenticates this source, its
launch-resource sidecar, and its complete reachable portable-MIR closure. The
generic semantic lowerer creates one aligned, epoch-branded workgroup
allocation shared by all lanes; no workload profile or prebuilt Kernel IR is
selected. V1 admits only sum over `u32`, `i32`, and `f32`, with an exact
one-dimensional power-of-two workgroup in `1..=256`. Every lane participates
in every acquire-release barrier phase. Unsupported scalar types, operations,
geometry, provider identity, and target profiles fail before target-bound
Kernel IR is created.

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

Both kernels are ordinary attributed Rust modules with source-level typed ABIs.
They enter the same feature-independent production transaction: authenticated
rustc collection, semantic MIR, ranked PLIRON, verified Kernel IR, composed
formal/ranked memory checks, target-bound gfx942 or gfx950 LLVM,
compiler-bound handoff, measured upstream LLVM target APIs plus in-process LLD,
and COV6 inspection. There is no workload-profile selector on this route, and
the compiler handoff grants no load or launch authority.

The ignored neutral-collective rustc driver requires the pinned nightly. It
authenticates the compiler-observed provider definition identities and
recomputed complete source-closure pin, then checks semantic MIR, ranked
PLIRON, generic LDS/tree/barrier KIR, target binding, and the LLVM route for
the three reduction and three scan examples on both targets. The separate
protected reduction driver requires the authority launcher and measured Worker
V3 and LLVM build identities. It starts again from the immutable reduction
sources for all three scalar profiles
and checks real Worker/finalizer output for the exact
256-byte static group segment, 288-byte complete kernarg ABI, required
`64x1x1` workgroup, COV6 descriptor, and deterministic two-run HSACO. It does
not dispatch the code object or grant load/launch authority. The scoped-atomic
profile and scan HSACO qualification are outside this protected driver.

This is bounded source-to-code-object evidence, not a compiler-refinement proof
or a claim of generalized memory safety, race freedom, reduction coverage, or
GPU execution. The closed target-neutral V1 reduction contract is supported
only on the pinned gfx942 and gfx950 production compiler profiles. This is
compiler-target evidence for both targets, not gfx950 execution evidence. The
currently qualified direct-KFD execution path is gfx942 only and must enter
through Worker V3 and the pure-Rust KFD runtime. The finalizer uses no COMGR and
no shell linker; it does not shell out to `clang`, `llc`, or `ld.lld`.

## Validation

```sh
export FE2O3_CRATE_BINDING_ID_V1=\
0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
cargo test --locked --manifest-path examples/workgroup_sync_v1/Cargo.toml
cargo test --release --locked --manifest-path examples/workgroup_sync_v1/Cargo.toml
cargo clippy --locked --manifest-path examples/workgroup_sync_v1/Cargo.toml \
  --all-targets -- -D warnings
VERUS=/absolute/path/to/pinned/verus \
  examples/workgroup_sync_v1/run-verus.sh
```
