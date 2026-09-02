# fe2o3 Backend Roadmap

For the full milestone plan, see [implementation-plan.md](implementation-plan.md).

## Implemented Inventory

This inventory includes the historical elementwise MVP. The
production-directed architecture now uses an isolated worker backed by one
pinned upstream LLVM build: LLVM target-machine APIs emit relocatable objects
and in-process LLD library APIs link HSACO. That path uses neither COMGR nor
shell invocations of `clang`, `llc`, or `ld.lld`.

The 2026-08-18 ownership refactor is infrastructure, not a compiler
promotion. Issues [#134](https://github.com/harsh-nod/fe2o3/issues/134) and
[#135](https://github.com/harsh-nod/fe2o3/issues/135) remain open. The working
compiler enters one unselected production transaction inside
`rustc-codegen-fe2o3`; legacy and exact-profile compiler implementations have been removed. `FE2O3_QUALIFICATION_ORACLE_V1` is rejected, and the managed Worker V3 route is the only compiler composition.

- Project naming and reserved symbol namespace use `fe2o3`.
- `fe2o3-mir-model` now owns the canonical Pliron-independent MIR executable,
  type, memory, constant, control-flow, wire, and mem2reg models formerly
  implemented behind `dialect-mir`. `dialect-mir` remains a compatibility
  re-export and exposes a bounded Pliron `mir.*` module/function/block shell
  only with its non-default `pliron` feature.
- `fe2o3-compiler-api` defines bounded target-neutral contracts for one production request and output. `cargo-fe2o3` and `rustc-codegen-fe2o3` own the sole managed production composition, with no selector or fallback slot.
- `fe2o3-pliron` pins Pliron v0.17.0 commit
  `5bdf861bf03e7f20242b25717fb653336d02e487` and implements a bounded D0
  context, private identity anchor, registration, verification, and pass-plan
  shell. It does not expose generic pass execution over contextless pointers. Seven target-neutral
  representation shells exist for `kernel.*`, `schedule.*`, `tile.*`,
  `gpu.*`, `proof.*`, `dispatch.*`, and `autotune.*`. They perform no connected
  lowering, target selection, artifact production, or launch.
- `fe2o3-lower-mir-kernel` retains a narrow bounded MIR-to-kernel conformance
  service. Detached KIR-envelope and kernel-to-GPU lowering services were
  removed; the production compiler owns canonical KIR custody and target
  lowering without an alternate selector or fallback.
- `fe2o3-amdgcn-model` now owns the existing strict AMDGPU target vocabulary
  and lowering implementation. `dialect-amdgcn` is its historical compatibility
  facade, not an implemented AMD Pliron dialect.
- `fe2o3-host-api`, `fe2o3-service-model`, and `fe2o3-service-host` provide
  inert host-operation records, executable-free persistent-service semantics,
  and authority-free borrow-retaining lifecycle typestates. They do not compile,
  allocate, load, launch, wait, persist, or execute a service.
- A HIP runtime wrapper can allocate buffers, copy data, load HSACO modules, look
  up kernels, and launch them with packed parameter arrays.
- `#[kernel]` emits strict V1 registration metadata with a direct function
  pointer. The collector rejects malformed, duplicate, inconsistent, or
  unregistered prefix-only candidates transactionally.
- `cargo-fe2o3 doctor` validates ROCm/HIP toolchain discovery.
- `cargo-fe2o3 build` builds and loads `librustc_codegen_fe2o3.so`.
- `rustc-codegen-fe2o3` wraps `rustc_codegen_llvm` for host codegen and detects
  kernel candidates in rustc codegen units.
- The backend collects device-reachable MIR functions from validated
  registrations, skips intrinsic placeholder bodies, rejects actual `std`
  reachability, and dumps a deterministic collection summary.
- `FE2O3_DUMP_MIR=1` imports the collected device MIR into a small
  Pliron-facing scaffold and prints function, block, statement, and terminator
  shape without changing the current HSACO emission path. The scaffold also
  builds a flat typed `mir.*` operation-record stream for the future Pliron
  builder, including typed locals, statement destination and operand labels, and
  terminator call callee, destination, and operand labels, plus the first
  operation-specific lowering records such as `mir.assign`, `mir.load`,
  `mir.store`, `mir.gep`, `mir.slice_len`, arithmetic ops, comparisons, and
  casts. Evaluated integer constants are appended to constant operand labels
  when rustc can resolve them. The dump also builds a first record-driven
  lowering-plan summary from the flat record stream. The AMDGPU emission path
  consumes that plan to cross-check kernel argument types, required store/return
  ops, thread-index calls, record load coverage, and selected index/arithmetic
  shape markers before emitting through the existing MIR recognizer. Load/store
  record place labels are parsed into a small access sketch, helper/raw index
  records are parsed into a linear index sketch, and slice reads/writes are
  combined into a slice-access sketch keyed by ABI arg, MIR local, and affine
  index. The sketch tracks direct slice accesses plus
  `DisjointSlice::get_mut`/`get_mut_at` element references through option
  projection into the final deref load/store. The AMDGPU validator now checks
  read-only slice loads, direct `&mut [T]` output stores, and disjoint output
  read-before-write stores from that record-derived slice sketch. A record
  expression sketch also binds slice-load leaves, disjoint output element
  leaves, scalar args, float literals, unary/binary expression ops, and store
  roots so the validator can cross-check expression requirements. When that
  sketch can reconstruct the full expression root, the AMDGPU path now uses the
  record-derived `ElementwiseExpr` for LLVM IR emission; raw rustc MIR remains
  the temporary fallback for shape discovery the record plan does not yet own.
- `rustc-codegen-fe2o3` contains the first real backend utilities:
  - ABI validation for supported kernel arguments from monomorphized MIR locals.
  - A narrow MIR recognizer and AMDGPU LLVM IR emitter for `f32`/`f64` elementwise
    expression kernels using read-only slice operands, scalar operands, one
    mutable output slice, in-place reads from that output slice, float literal
    constants, unary negation, and leaf-only copy stores.
  - the historical `legacy-v1` `.ll -> .o -> .hsaco` sidecar path using ROCm
    command-line clang and `ld.lld`; this is compatibility history, not the
    production-directed finalizer.
- The production-directed direct LLVM/LLD worker parses and links modules,
  optimizes, emits relocatable ELF through pinned upstream LLVM target-machine
  APIs, and links HSACO through in-process LLD library APIs. It does not use
  COMGR or a command-line compiler or linker.
  The removed `FE2O3_CODEGEN_PIPELINE` environment is rejected.
- `fe2o3-amdgcn-model`, reached through the `dialect-amdgcn` compatibility
  facade, lowers that verified fill subset to deterministic AMDGPU LLVM. Its
  code-object regression checks target/features, ELF and metadata versions,
  exact kernel symbol and descriptor, ABI, address space, and fixed workgroup
  metadata. Unsupported IR fails with located diagnostics.
- `cargo-fe2o3 build/run` writes `.ll` and `.hsaco` artifacts under
  `target/fe2o3`; `fe2o3-copy` covers a leaf-only store,
  `fe2o3-downsample` covers a constant-stride input load,
  `fe2o3-fill` covers a literal-root store,
  `fe2o3-gather-odd` covers a stride-plus-offset input load,
  `fe2o3-shift` covers a constant-offset input load,
  `fe2o3-previous` covers a negative constant-offset input load,
  `fe2o3-stencil` covers multiple derived loads from one input slice,
  `fe2o3-raw-add-index` covers affine reads formed by adding two raw index
  expressions,
  `fe2o3-raw-const-minus` covers constant-minus-index reads with a negative
  stride,
  `fe2o3-raw-parenthesized-sub` covers index subtraction that collapses to a
  constant read index,
  `fe2o3-raw-disjoint-inplace-shift` covers raw `usize` arithmetic for a
  `DisjointSlice<f32>` output read-before-write store,
  `fe2o3-raw-disjoint-shift` covers raw `usize` arithmetic for a
  `DisjointSlice<f32>` output store,
  `fe2o3-raw-gather` covers raw affine `usize` index arithmetic,
  `fe2o3-raw-neighbors` covers raw `usize` add/sub neighbor reads,
  `fe2o3-raw-output-shift` covers raw `usize` arithmetic for an indexed
  `&mut [f32]` output store,
  `fe2o3-saxpy` covers a multi-op expression tree, and
  `fe2o3-axpy-inplace` covers indexed `&mut [f32]` output with read-before-write.
- `fe2o3-add-inplace` covers `DisjointSlice::get_mut` output read-before-write.
- `fe2o3-negate` covers `fneg` emission from MIR unary negation.
- `fe2o3-normalize` covers `f32` literal constants, `fsub`, and `fdiv`.
- `fe2o3-vecadd-f64` covers double-precision elementwise emission.
- The `vecadd`, `add-inplace`, `copy`, `downsample`, `fill`, `gather-odd`,
  `scale`, `shift`, `previous`, `stencil`, `raw-add-index`,
  `raw-const-minus`, `raw-parenthesized-sub`, `raw-disjoint-inplace-shift`,
  `raw-disjoint-shift`, `raw-gather`, `raw-neighbors`, `raw-output-shift`,
  `saxpy`, `axpy-inplace`, `negate`, `normalize`, `pipeline`, and
  `vecadd-f64` examples load their HSACO files from `FE2O3_HSACO_DIR`, which is
  set by `cargo-fe2o3 build/run`.
- Qualification artifact generation is selected by an explicit closed manifest
  route. The current `kernel-ir-v1` route covers only `fe2o3-fill`; protected
  production builds enter through `cargo fe2o3 authority release`.
- The selector-free manifest-wide `cargo-fe2o3 smoke` command is retired. The
  checked-in manifest remains source inventory, host-check policy, and bounded
  artifact-qualification policy rather than a claim that every example has a
  production code-generation route.
- Generated HSACO files are validated with `llvm-readobj --notes` when available
  to confirm AMDGPU format, target metadata, and kernel name metadata.
- `cargo-fe2o3` infers `FE2O3_TARGET` from `rocminfo` when the environment
  variable is not set.
- At the earlier elementwise-backend checkpoint, end-to-end `vecadd`,
  `add-inplace`, `copy`, `downsample`, `fill`,
  `gather-odd`, `scale`, `shift`, `previous`, `stencil`, `saxpy`,
  `raw-add-index`, `raw-const-minus`, `raw-parenthesized-sub`,
  `raw-disjoint-inplace-shift`, `raw-disjoint-shift`, `raw-gather`,
  `raw-neighbors`, `raw-output-shift`, `axpy-inplace`, `negate`, `normalize`,
  `pipeline`, and `vecadd-f64` ran successfully on `gfx1201` using TheRock
  ROCm `7.13.0a20260509`.

## Remaining Compiler Milestones

1. Extend the integrated structured path beyond the implemented exact fill,
   vecadd, alpha/zeta, and advanced `gfx942` profiles to every current example,
   preserving strict rejection and transactional cleanup before making it the
   default and removing the temporary elementwise recognizer.
2. Move the remaining legacy `ElementwiseShape` output/source discovery off raw
   rustc MIR and onto
   the record-derived access/expression sketches.
3. Broaden the existing bounded device-operation lowering rules:
   - `thread::thread_idx_*` -> `llvm.amdgcn.workitem.id.*`
   - `thread::block_idx_*` -> `llvm.amdgcn.workgroup.id.*`
   - `sync::syncthreads` -> `llvm.amdgcn.s.barrier`
   - `block_dim_*` and grid dimensions -> dispatch packet reads
4. Generalize the explicit device kernel ABI beyond the reviewed exact
   profiles:
   - Rust slices lower to pointer plus `usize` length.
   - `DisjointSlice<T>` lowers to mutable pointer plus `usize` length.
   - Plain scalars pass by value.
5. Generalize bundle embedding and artifact placement beyond the reviewed
   exact profiles and legacy sidecars in `target/fe2o3`.
6. Broaden the repeatable protected hardware gates beyond the current exact
   target and kernel profiles.

## Runtime ABI Assumption

The launch macro currently packs slice-like values as two HIP kernel arguments:
device pointer then `usize` length. The compiler backend should generate matching
kernel entry signatures.

## Runtime R8 Status

Implemented for `gfx942:xnack-`:

- a classic KFD SDMA queue with nonblocking generation-tagged submission,
  deadline polling/waiting, exact completion custody, and batches of at most 63
  so the 64-slot ring always retains one empty slot;
- host-to-host, host-to-HBM, and HBM-to-host linear copies through the same
  move-only buffer API;
- per-device best-fit host/HBM pools with completion-gated recycle, stale-lease
  rejection in the model, observations, and explicit trim;
- process-wide admission of multiple physical devices before any queue exists,
  independent child queues, and mandatory explicit reverse-order qualification
  teardown;
- a multi-device runtime router with globally unique facade handles and bounded
  cooperative asynchronous host-staged peer copy; and
- aligned-traffic KFD, HSA, and HIP single-device and two-device copy harnesses
  with common host submit/wait timing boundaries.

The R7 Verus proof covers abstract lease generations, non-reuse while retained,
quarantine, dependency-gated publication, and exact cross-device coordinates.
It does not prove the Rust implementation or native hardware. Frozen UAPI and
SDMA manifests plus executable tests provide separate checked evidence. Native
correctness and performance require a retained, commit-identified MI300X result
artifact before they are described as measured. The current bounded result is
[`async-copy-mi300x-2026-09-01.md`](../benchmarks/runtime_gfx942/results/async-copy-mi300x-2026-09-01.md);
it reports a correctness pass but not KFD copy-performance parity.

The additive `RuntimeAsyncCopyBackendV1` SPI and typed `copy_async` facade are
implemented. The router can drive same-device and cross-device host-staged
copies with one logical range request of at most 64 KiB per read/write poll;
child reconciliation can still touch allocation-wide native-dirty or
copy-on-write state. The additive R8 Verus abstraction proves ten
scheduling, resource-binding, atomic-linearization, and unique-member
collective-phase obligations with eleven expected-negative mutations. It is a
whole-resource mathematical model, not a refinement of the ranged Rust copy
state machine or native copy-engine overlap.

The executable gfx942 kernel-semantic model checks exact device, code,
artifact, mapping, atomic-object, ordering/scope, coherence-premise, collective
geometry, convergence-premise, and LDS bindings for the reviewed integer
atomic and collective roster. Those premises are caller declarations and the
result remains `ModelOnly`; this is Checked admission evidence, not proof of
the loaded instructions, GPU coherence, or execution.

Still open: multiple simultaneous compute dispatches on one KFD device,
persistent compute allocations shared with the KFD SDMA engine, native XGMI
peer mapping and copy, authenticated code-object semantic refinement, native
system-coherence evidence, and broader atomic/collective language support.
