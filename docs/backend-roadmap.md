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
- `fe2o3-target-spec` defines vendor-neutral target profile metadata, canonical
  profile text, and validation for compiler, proof, and host contracts. It does
  not parse vendor target IDs, lower target IR, detect runtime devices, or derive
  hardware capabilities.
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
- Historical scaffold note: `cargo-fe2o3 doctor` originally validated
  ROCm/HIP discovery. The current command is KFD-first, keeps compiler tools
  separate, and treats ROCgdb/rocprofv3 as optional observation tools.
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

## Historical Runtime ABI Assumption

The original HIP launch macro packed slice-like values as two kernel arguments:
device pointer then `usize` length. The compiler backend generated matching
kernel entry signatures.

Current generated bindings implement the address-free
`CompilerGeneratedKfdArguments` contract. They retain borrowed host allocations
until the authenticated Worker V3 and direct-KFD runtime boundary materializes
the admitted device allocation and kernarg projection. HIP/HSA argument routes
remain qualification-only and cannot substitute for that production path.

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
[`async-copy-mi300x-2026-09-02.md`](../benchmarks/runtime_gfx942/results/async-copy-mi300x-2026-09-02.md);
the immutable R7 baseline remains in
[`async-copy-mi300x-2026-09-01.md`](../benchmarks/runtime_gfx942/results/async-copy-mi300x-2026-09-01.md).
The R8 result reports a correctness pass but not KFD copy-performance parity.

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

## Runtime R9 Status

R9 implements a bounded low-level native XGMI path for `gfx942:xnack-`.
Generation-retained topology now includes exact directional link records; route
admission requires same-hive endpoints, one enabled type-11 XGMI edge with
nonzero bandwidth, the exact ordinary/XGMI engine inventory, and one-bit
recommended engine selection. PUBLIC HBM owners support canonical two-GPU
mapping and unmapping with cumulative-prefix compensation. The directional
BY_ENG_ID queue exposes move-only asynchronous submission, bounded depth, and
completion-gated ownership return. The matched KFD/HSA/HIP benchmark validates
changing patterns and canaries outside a common submit-through-completion
interval under a two-GPU load gate.

The authenticated LLVM/MC worker and Rust checker also admit a closed gfx942
machine-instruction structure subset for integer RMW atomics, LDS access and
permutation primitives, and workgroup barriers. All `_DPP` spellings remain
rejected pending an exact reviewed roster and fixtures.

The receipt binds exact payload, descriptor, entry, reachable instruction
bytes, and primitive classifications to a loader-prepared dispatch. The safe
structure-required execution wrapper in `fe2o3-runtime-machine-adapter` then
consumes that application together with independent Worker V3 authority and a
checked device, delegates to the sole authorized runtime dispatch, and returns
the retained structure with the completion result. The receipt remains Checked
structural evidence and grants no load or launch authority: instruction
semantics, compiler preservation, ordering/scope, convergence, and coherence
are not proved by opcode classification. Worker V3 remains the semantic and
launch authority.

The additive R9 Verus model proves fourteen abstract mapping, compensation,
route/copy currentness, custody, exact-evidence, and dispatch-publication
obligations. Fifteen corresponding R9 mutations fail as expected; cumulative
coverage is 81 proved obligations and 60 mutations. There is no Rust-to-Verus
refinement theorem. KFD, firmware, XGMI, and coherence remain Contracted, and
hardware correctness/performance become Measured only through the clean-commit
idle-MI300X runner. See the
[R9 claim boundary](../crates/fe2o3-kfd/docs/r9-native-xgmi-machine-structure-v1.md).

Still open at the R9 checkpoint: multiple simultaneous compute dispatches on one KFD device,
checked multi-queue SDMA striping and same-device bidirectional overlap,
persistent facade compute allocations shared with SDMA/XGMI, routing
`RuntimeContextV1` peer copies through the native owners, true authenticated
source-to-machine semantic refinement, native system-coherence evidence,
atomic load/store machine correspondence, and broader closed
atomic/collective language support.

## Runtime R10 Status

R10 adds explicit execution-detail capability discovery plus cancellation and
drain to the public facade without changing the frozen Runtime Worker V1 wire
protocol. R13's separately negotiated Runtime Worker V4 transports those
additive SPIs; this runtime transport is distinct from compiler/proof Worker V3. The
direct KFD backend now retains pooled native host/HBM SDMA allocations, fully
zero-initializes new HBM allocations, scrubs them before recycle, trims the
pool at shutdown, and admits allocation-disjoint compute/copy overlap. Native
SDMA dependency chains are bounded to 256 before publication-state mutation.
The exact two-device XGMI backend now has a public-facade benchmark alongside
the lower-level retained-mapping, single-doorbell diagnostic.

The closed executable model covers concurrent compute/copy state, event
dependencies, all-or-nothing batch publication, cancellation, quarantine, pool
generations, peer ownership, exact atomic ordering/scope/fence/value records,
and Wave64 barriers, reductions, and scans. Twenty R10 Verus obligations and
eleven expected-negative mutations bring the pinned totals to 101 obligations
and 71 mutations. Six deterministic public-runtime traces differentially check
the executable facade against that model. A fail-closed report checker compares
matched KFD rows against both HSA and HIP using caller-supplied p50/p95 latency
and p50 bandwidth thresholds.

Still open for the parity profile: concurrent compute queues on one device;
one persistent allocation shared by compute and SDMA/XGMI; native XGMI routing
inside the compute-capable multi-device context; persistent facade peer
mappings and batched publication; device-clock profiling; public general
atomic/collective operations backed by authenticated source-to-ISA refinement;
native system-scope litmus evidence; and current clean-machine correctness and
performance evidence. R10 does not claim HIP/HSA parity.

## Runtime R15 Status

R15 adds Runtime Worker V5 without changing the exact V4 wire or source
contract. V5 preserves typed atomic/collective operation, scope, ordering,
compare-exchange, participant, and geometry declarations across the process
boundary and rejects malformed requests before backend custody. It does not
create native semantic or proof authority. An opt-in canonical semantic
profile sidecar separately binds those declarations to frozen Runtime Profile
V1 publications; distinct V2 timestamp custody and query APIs join the sidecar
without changing the V1 producer or report paths.

The gfx942 kernel-analysis slice now checks one authenticated, separate scalar
binary32 multiply/add recurrence step and supplies a pinned APFloat candidate
model. It does not yet establish the machine-loop backedge, AMDGPU floating
semantics, compiler refinement, or Worker V3 authority. The SDMA benchmark can
publish balanced depth shards over an admitted even set of two through sixteen
striped native queues before waiting, but each shard retains an independent
currentness envelope; this is diagnostic concurrency, not a production
all-or-nothing multi-queue transaction.

R15 still does not establish HIP/HSA parity. The open items above remain, plus
a production multi-queue SDMA custody API and clean, idle-hardware evidence for
the new striped profiles. Orders-of-magnitude speedups are accepted only for an
exact matched workload with retained measurements; they are not a general
runtime target or current claim.

## Runtime R16 Status

R16 adds an opt-in bounded async progress mode without changing the original
observation-only engine constructor. Registered pending streams receive cyclic,
budgeted `flush_stream` attempts on the single owner thread. Registration drop
does not cancel, release, or finally flush work; retryable failures remain
observable and terminal ambiguity seals the engine. Direct KFD remains
thread-affine, so this cross-thread mode currently applies to Send-capable
Worker V4/V5 adapters. It is a host progress mechanism, not a native liveness
proof.

The gfx942 KFD surface now has a production striped SDMA submission boundary for
2 through 16 queues and at most 1,008 requests. It prepares all balanced shards
before the first publication and reports exact confirmed, indeterminate, and
untouched observations after partial failure. Terminal custody is audit-only
and retained until process teardown; only no-effect preflight returns retryable
requests. It provides no rollback or atomic device-transaction guarantee.
Preparation/publication injection executes the shared production algorithms;
closing-currentness injection covers the shared state transition but not the
outer live-session currentness/poison path. Live native fault-injection evidence
remains unavailable.

The R16 abstract Worker V5 boundary adds 21 Verus obligations and 10
expected-negative mutations, bringing authenticated totals to 193 and 121. It
models reachable already-decoded request states, attempted versus accepted and
indeterminate custody, response sealing, exact contract retention, and an
ordered exhaustive semantic sidecar sequence join. It is not a byte parser,
subprocess, concrete backend-call, Rust-to-Verus, or native execution refinement.

R16 still does not establish HIP/HSA parity. The next native architecture step
requires one persistent thread-affine allocation owner shared through bounded
move-only compute, local-SDMA, and exact two-device XGMI use leases; the current
linear owners cannot represent that safely. Clean idle-MI300X correctness and
matched HIP/HSA performance evidence also remain open. General
orders-of-magnitude superiority is not a credible acceptance criterion; each
claim must name and retain its matched workload and measurement scope.

## Runtime R17 Status

R17 adds an addressless, thread-affine KFD custody core around one existing
mapped device-local allocation. A fixed 64-slot ledger derives read/write
access from closed compute, local-SDMA, and peer-mapped classifications;
excludes overlapping active writers; preserves move-only custody across
prepare, publication, timeout, completion, settlement, and caller-reported
quarantine; and requires an exact successful host frontier before a later
overlapping hazard can be reserved. The peer-mapped names deliberately grant
no XGMI route, topology, engine, publication, or completion authority.

An independent executable model adds exact home-VM/queue and directional R9
route-metadata predicates, reusable slot generations, and a private registry
incarnation that rejects reconstructed-registry transition-token and dependency
substitution. Numeric observation keys remain non-authoritative. Its pinned
Verus summary model adds 32 obligations and 14 expected-negative mutations, bringing
the authenticated totals to 225 and 135. The proof consumes mathematical
summary predicates for dependency readiness and conflicts; it is not a
Rust-to-Verus, KFD, SDMA, firmware, or hardware refinement.

Four real Python child-process tests now drive Runtime Worker V4/V5 through
pending event observation and background `flush_stream` progress. They cover
ordinary V4 completion, atomic V5 completion, response deadline sealing, and
decoded-terminal plus EOF sealing. This is subprocess/protocol evidence, not
native progress or performance evidence.

R17 still does not establish HIP/HSA parity. The KFD custody core is not yet
connected to compute AQL, local SDMA, native XGMI, live currentness observation,
or the public runtime facade. The next tranche must bind an exact queue
occurrence and native publication/completion ticket to each persistent use and
retain terminally ambiguous native authority until process teardown.

## Runtime R18 Status

R18 connects the persistent-allocation custody core to one targeted local SDMA
queue. Promotion preserves the existing queue-owned device-buffer debit and
binds the exact parent queue occurrence, native child queue, directional engine,
pool generation, mapped storage identity, and extent. A move-only submission
retains the device allocation, host buffer, range use, and native ticket across
publication, nonblocking poll, bounded wait, completion, and settlement. Only a
confirmed publication enters `Published`; recoverable prepublication failure
restores the exact owners, while retained or postpublication uncertainty becomes
opaque process-teardown custody. A consuming retirement transition reclaims
settled ledger history only for the exact quiescent frontier and returns both
allocation and frontier custody unchanged on stale or substituted input.

The independent R18 executable model composes the R17 registry and lease rather
than duplicating its lifecycle as shadow state. Its pinned Verus summary adds 34
obligations and 24 expected-negative mutations, bringing the authenticated
totals to 259 and 159. It covers exact local-SDMA bindings, direction and engine,
ticket occurrence, ranges, timeout custody, completion restoration, and
quarantine. It remains a standalone abstract model, not a Rust-to-Verus, KFD,
firmware, or hardware refinement.

Native-neutral KFD tests drive the production custody transitions for
publication, pending, timeout, completion, restoration, and retirement. They
include 66 sequential publish/complete/retire cycles and stale/cross-allocation
frontier rejection. These are ownership-transition tests, not native execution
or liveness evidence.

R18 still does not establish HIP/HSA parity. The bridge is single-flight and
local to one targeted queue; it is not wired into the public runtime facade or
async progress engine and provides no persistent compute AQL, concurrent range
leases, directional or striped queue-set integration, peer/XGMI use, or unified
multi-device graph. The shared MI300X host was busy during validation, so this
tranche carries no native execution, copy-performance, or comparative HIP/HSA
evidence.

## Runtime R19 Status

R19 adds a directional persistent local-SDMA adapter for the exact queue shape
used by the direct KFD runtime: one parent occurrence with distinct engine-1
H2D and engine-0 D2H children. It preserves the existing buffer-accounting
debit, binds logical and physical pooled extents separately, and restricts
every copy to `0 < range <= logical <= physical <= 256 MiB`. The public R19
submit surface does not accept a dependency frontier: exact completion,
settlement, and frontier retirement are the mandatory reuse gate. The next use
may repeat the same direction or select the other child.

All moved resources are represented explicitly on failure. Clean
prepublication failure returns the allocation and host buffer; retained or
currentness-ambiguous publication, promotion, demotion, or completion returns
opaque process-teardown custody. The active packet path uses bounded
operational currentness checks, while full topology and aperture observations
remain lifecycle gates. A stack-sized single-request lower path avoids batch
roster allocation for each packet.

The independent R19 executable model has 23 focused tests. Its pinned Verus
summary adds 46 obligations and 20 expected-negative mutations, bringing the
authenticated totals to 305 and 179. This is an abstract proof of the bounded
model, not a correspondence theorem for executable Rust, KFD, firmware, or
hardware behavior.

R19 is still a low-level prerequisite, not HIP/HSA parity. It is not connected
to the runtime facade, async engine, or workers. The native linear packet cap is
`0x003f_ffe0` bytes and the persistent allocation is single-flight, so a
256 MiB transfer requires 65 complete submit/poll/restore/retire cycles. H2H,
D2D, multi-packet batched publication, persistent compute/XGMI sharing, and
matched hardware measurements remain open. This tranche therefore carries no
copy-performance or comparative HIP/HSA claim.

## Runtime R20 Status

R20 connects the R19 directional persistent allocation to the direct
`KfdRuntimeBackendV1` facade. Device-local allocations are promoted into exact
engine-1 H2D/engine-0 D2H custody, while host-visible allocations retain their
ordinary queue-owned buffers. Facade copy admission accepts only H2D and D2H;
H2H and D2D reject before handle, stream-tail, dependency, allocation, or
completion-reservation mutation. Packet publication is capped at
`0x003f_ffe0` bytes, and each completion must settle and retire its exact R19
frontier before the allocation can publish the next packet.

Poll and wait do not publish continuations. Explicit `flush_stream` may publish
one dependency-ready packet, and cancellation is available only while the copy
is unpublished with zero completed bytes. A zero-progress retryable publication
failure becomes an ordinary conclusive failed record. A retryable failure after
one or more completed packets releases every native and logical retain, then
stores an exact releasable quiescent-without-result marker that poll, wait,
drain, events, dependencies, stream destruction, shutdown, and Worker V4/V5
preserve. Upload, download, zeroing, dirty-shadow reconciliation, and release
scrub use packet-bounded transient staging; a retryable demotion or recycle
failure retains explicit device or cleanup-only demoted custody.

The independent R20 executable model has 14 focused tests. Its pinned Verus
summary adds 31 obligations and 15 expected-negative mutations, bringing the
authenticated totals to 336 and 194. Worker V4 and V5 child-process tests each
exercise three quiescent copy cycles through flush, poll, drain, event-gated
release, and orderly shutdown. These are abstract and host-protocol results, not
a refinement theorem or native execution evidence.

R20 still does not establish HIP/HSA parity or a speedup. Direct KFD remains
thread-affine; only a Send-capable worker adapter can use the background async
engine. The single-flight allocation still requires 65 publication/completion/
retirement cycles for 256 MiB, and H2H, D2D, batched local SDMA, persistent
compute/XGMI sharing, and a unified native multi-device owner remain open. No
MI300X or matched HIP/HSA performance measurement is attached to this tranche.

## Runtime R21 Status

R21 places a private scripted transition driver behind the same move-only
directional SDMA facade used by the native backend. Production builds retain a
type-preserving native branch; test builds can supply exact FIFO results and
opaque, driver-bound owners. Sixteen concrete tests cover promotion, demotion,
publication, dependency pending, poll/wait, completion metadata, retirement,
recycle, hidden cleanup, multi-chunk H2D/D2H/zero/shadow work, scrub, orderly
shutdown, and abort-on-live-custody Drop. Mixed and cross-driver owners fail
closed before consuming script state.

This coverage found a concrete D2H continuation defect. Once an earlier chunk
has changed host-visible bytes, a later retryable publication can no longer
return a retryable rejection; it releases retained ownership and records the
same exact quiescent-without-result class used for partial device mutation. The
independent executable model represents host and device mutation separately
with 17 focused tests. Its pinned Verus artifact adds 37 obligations and 15
rejected mutations, bringing authenticated totals to 373 and 209.

R21 closes the scripted facade-failure coverage item, not native refinement.
The seam does not inject kernel-driver failures, prove correspondence between
the Rust and Verus state machines, or establish cleanup liveness, hardware
correctness, HIP/HSA parity, or performance. Batched large-copy publication is
the next local-SDMA throughput requirement.

## Runtime R25 Status

R25 adds the first bounded bridge from runtime-owned persistent HBM copy
storage to fixed compute dispatch. After one exact full-allocation H2D transfer,
the direct gfx942 backend can bind the same mapped, exact-extent device
allocation to one metadata-inspected compute packet on primary lane zero. The
bridge accepts exactly one full-range global binding and preserves the allocation,
storage identity, content authentication, use generation, dispatch generation,
completion custody, and exact settled frontier through publication, polling,
recycle, detach, restoration, and retirement. Once selected, this path cannot
fall back to generic host-data materialization. Its public performance
observation reports `PersistentDeviceReused` and zero user-data
materializations.

Read and read/write access require the exact authenticated H2D content digest.
Write effects invalidate the runtime host shadow after completion; read-only
effects preserve it. Primary-lane contention leaves the submission pending with
the H2D-ready owner untouched. The bridge serializes against every published
SDMA submission and every active compute lane on the queue; it does not claim
compute/copy or multi-lane overlap. Clean prepublication rejection restores the
exact input. A no-effect full-ring publication remains pending in prepared
custody and is retried by explicit progress. Cancellation before publication
withdraws that prepared dispatch, restores the exact H2D-ready allocation, and
releases its runtime reservations without inventing profile publication or
completion records. Any uncertainty after native retention poisons the queue
and keeps the batch, completion, detached data, or mapped storage in opaque
terminal custody until process teardown. Foreign-queue rejection preserves the
exact retryable owner.

The independent executable model has 17 focused tests. Its pinned Verus model
adds 38 obligations and 18 expected-negative mutations, bringing the abstract
totals to 532 obligations and 293 rejected mutations. These models cover exact
storage/range/generation retention, derived access, initialization, no
materialization fallback, completion authentication, quarantine, restoration,
and frontier retirement. They are not a correspondence theorem for the Rust
implementation or evidence about KFD, firmware, or hardware truth.

R25 does not establish HIP/HSA parity or a speedup. It admits fresh or
exact-size pooled storage, but not partial or padded pooled ranges, multiple
global bindings, auxiliary compute lanes, concurrent SDMA, persistent XGMI
sharing, or a unified multi-device compute owner. Native
MI300X execution and matched KFD/HSA/HIP timing for this exact path remain the
next qualification step. The separate measured local-copy throughput gap also
remains open.
