# Support matrix

fe2o3 is a developer preview. This matrix distinguishes a runnable or qualified
engineering slice from a supported public platform. **No GPU target currently
has a stable public support commitment. This matrix makes no HIP/HSA parity
claim.**

## Status definitions

| Status | Meaning |
| --- | --- |
| Supported | Intended for public use and covered by an explicit compatibility policy |
| Qualified | Passed named tests for an exact revision, target, and environment |
| Experimental | Implemented but incomplete, unstable, or not admitted as a public workflow |
| Unavailable | Not implemented, intentionally rejected, or missing required authority |

Qualification does not imply support. Authority-free evidence does not
authorize compiler publication, loading, dispatch, or a hardware claim.

## Host platforms

| Host | Status | Boundary |
| --- | --- | --- |
| x86-64 Linux | Qualified for generic CI; experimental for users | Primary development host and the only host for hardened Linux file/process/device boundaries |
| Other Linux architectures | Unavailable | Not qualified; several binaries and tests require x86-64 |
| macOS | Unavailable | No KFD; Linux-specific simulator input custody intentionally fails instead of weakening guarantees |
| Windows | Unavailable | No KFD; destructive artifact cleanup fails closed because the required opened-directory semantics are absent |

The repository pins Rust `nightly-2026-04-03` and declares Rust version `1.94`.
Use the pinned toolchain for development-preview qualification.

## GPU targets

| Target | Compile | Direct-KFD execution | Public status |
| --- | --- | --- | --- |
| MI300X, `gfx942:xnack-`, Wave64 | Bounded qualified slices | Bounded diagnostics and exact qualification lanes | Experimental |
| `gfx942` configurations other than the qualified profile | Target-dependent | Not qualified | Unavailable |
| MI350 family, `gfx950` | Selected extraction/KIR and example surfaces exist | No qualified production direct-KFD launch path; separate example-specific hardware companions exist | Experimental, not a public runtime surface |
| RDNA targets including `gfx1151` | Partial compile/code-object slices | No qualified production launch path | Experimental compile surface only |
| Other AMD GPUs | Not qualified | Not qualified | Unavailable |
| NVIDIA or other non-AMD GPUs | No backend | No runtime | Unavailable |

fe2o3 does not infer support from a `gfx` name reported by a tool. Target,
features, wave width, KFD topology identity, artifact metadata, and qualification
policy must agree exactly.

## Execution modes

| Mode | Input | Status | What it establishes |
| --- | --- | --- | --- |
| CPU KIR simulation | Verified canonical KIR V7 plus typed request | Qualified for the admitted subset | Deterministic semantic execution and observation only |
| CPU source bundle simulation | Rust kernel exported through production source/MIR/KIR stages | Experimental | Exact authority-free bundle content and deterministic KIR execution |
| Direct-KFD diagnostic | Exact admitted artifact and invocation | Bounded qualified lanes | The properties named by that test only; the current runtime child multiplexes at most 65,536 logical streams over exactly two persistent compute lanes with caller-driven FIFO scheduling and disjoint-allocation concurrency |
| Production application dispatch | Ordinary Rust application | Unavailable publicly | Typed Worker V3 refinement-backend/receipt consumption exists, but ordinary public source-to-GPU authority remains unavailable because the protected verifier deployment and concrete issue #214 semantic-machine proof backend/artifact are missing |
| HIP execution | N/A | Unavailable | Not a fallback runtime |
| HSA execution | N/A | Unavailable on the production path | Legacy/qualification code does not define the public runtime |
| Runtime completion, events, and callbacks | Backend-neutral typed handles | Experimental | Submission/event query, poll, and bounded wait share one typed completion state; callbacks discharge exactly once on conclusive status, and stream query/synchronize returns an aggregate observation under one deadline. The optional async engine provides bounded executor-neutral event futures with stable budgeted completion polling and panic containment. Additive progress mode drives a separate bounded roster of pending streams through `flush_stream`; ordinary construction remains observation-only, and registration/future drop never cancels or releases work |
| Typed atomic/collective launch wrappers | Admitted typed kernel plus explicit contract | Experimental direct/multi KFD SPI; no production authority | Operation, scope, success order, compare-exchange failure order/weak mode, geometry, and collective membership are preserved through additive backend SPIs and the exact Runtime Worker V5 transport. KFD ordinary/qualification constructors remain false; an unsafe semantic-authority constructor may enumerate exact non-System atomic and workgroup-collective profiles, which are retained through scheduling and final invocation authorization. Unlisted/invalid profiles reject before custody; later final-authority denial settles the accepted unpublished submission and releases custody before publication. A concrete production authority, broader profiles, native litmus evidence, and formal refinement remain unavailable |
| Runtime Worker subprocess | Exact V1, V4, or V5 handshake | Experimental bounded transport | V1 requires an explicit immediate-progress marker and cannot host KFD. V4 forwards execution capabilities, flush, same-device async copy, cancellation, and deadline-bounded drain. V5 retains that exact surface and adds typed atomic/collective contract carriage. Direct and multi-device KFD implement the V5 server bound; copy-only native XGMI rejects semantic requests before custody and advertises neither capability. V4 bytes and source contracts remain unchanged, cross-version handshakes reject, and capability records fail closed outside the latest successful enumeration. Real child-process tests exercise V4 ordinary and V5 atomic background-flush completion plus timeout, decoded-terminal, and EOF sealing. R24 adds an in-place V5 server and a feature-gated copy-only KFD child so native ownership and shutdown remain on the child main thread; its ignored 256 MiB D2D qualification passed on one idle gfx942 device at exact commit `0631c5be`. The handshake is compatibility negotiation, not worker authentication, and this exact run does not establish broad native progress or performance. |
| Same-host multi-GPU peer copy | Exact admitted KFD devices | Experimental bounded lanes | Generic routing uses bounded host staging; a separate exact two-device copy-only backend retains native XGMI mappings and publishes a deterministic FIFO ready queue in directional prefixes of at most 63. One flush synchronously drains non-final prefixes from its entry snapshot and leaves the final prefix published for host/DMA overlap; poll/wait are observation-only. Opt-in async progress applies only through Send-capable Worker V4/V5 adapters; direct KFD remains thread-affine. Benchmark `outstanding_depth` is queued work on one ordered directional SDMA engine, not engine concurrency. |
| Striped same-device SDMA submission | Exact admitted gfx942 striped queue set | Experimental production custody boundary | One call deterministically balances up to 1,008 requests over an even 2-through-16 queue roster, prepares all shards before the first publication, and advances selection only after complete publication plus closing currentness. Partial failure distinguishes confirmed shards, one optional indeterminate shard, and untouched indexed requests as audit-only process-teardown custody; only no-effect preflight is retryable. It is not rollback-capable or an atomic device transaction. Coordinator fault tests exist, but live native fault-injection and performance evidence are missing. |
| Persistent native-allocation custody | One existing mapped gfx942 device-local allocation | Experimental bounded custody core | A non-Clone, thread-affine KFD owner retains one local mapping or complete canonical two-device peer mapping and tracks at most 64 move-only range uses. Access is derived from a closed operation roster; active writer hazards, stale frontiers, timeout release, and normal extraction while work is live or quarantined fail closed. R18-R23 bind local SDMA and D2D uses, and R25 binds one exact full-allocation compute use to the primary queue. Peer-mapped operations remain classification only; concurrent ranges, native XGMI use, general compute shapes, and concrete/formal refinement remain unavailable. |
| Persistent local SDMA adapter | One promoted device-local buffer and one ordinary host buffer on an exact targeted gfx942 SDMA queue | Experimental single-flight custody boundary | Promotion preserves the queue's existing outstanding-buffer debit and binds queue occurrence, native queue, direction/engine, pool generation, storage identity, and extent. Move-only submit, poll, and bounded wait retain exact allocation, host, range-use, and ticket custody; recoverable prepublication failures restore owners, while retained or later ambiguity is opaque process-teardown custody. Exact quiescent-frontier retirement reclaims bounded ledger history, with 66-cycle and stale/substituted-frontier host coverage. Production transition functions and independent executable/Verus models cover the bounded transition surface. It is not connected to the runtime facade or async progress engine and provides no compute, D2D, peer/XGMI, striped queues, concurrent range leases, hardware evidence, refinement theorem, or performance claim. |
| Directional persistent local SDMA adapter | One promoted pooled or exact-size device buffer plus one ordinary host buffer on the exact gfx942 engine-1 H2D/engine-0 D2H pair | Experimental runtime-integrated window custody | R20 wires the R19 parent/child queue and allocation custody into direct `KfdRuntimeBackendV1` H2D/D2H copies. R22 batches one through 63 canonical packets under one aggregate lease, one write-pointer publication, and one doorbell per window. Exact completion, settlement, and frontier retirement precede continuation publication by `flush_stream`; poll and wait are observation-only. Zero-progress retryable publication is conclusive failure, while partial-progress retryable failure releases all native and scheduler retains into an exact releasable quiescent marker preserved by events, dependencies, shutdown, and Worker V4/V5. R21's private driver seam and R22's native-neutral tests cover the transition and window failure surfaces. R30 binds full-write host content before H2D; R31 specializes one-packet requests; R32 production commit `9f715189b8f35d4adb58be303900f937d88389ad` fuses preparation and publication through one shared-currentness handoff while retaining failure custody and the final close. The aggregate bounded formal runner reports 696 obligations and 310 rejected mutations, but supplies no Rust/native refinement. In the retained [R32 1 MiB MI300X measurement](evidence/mi300x-r32-currentness-handoff-2026-09-05.md), E2E fell 9.32% median unadjusted versus R31 while remaining approximately 3.01x slower than HIP E2E. R25 shares only one exact full-H2D result over fresh or exact-size pooled storage with primary-lane compute; partial or padded pooled compute ranges, XGMI sharing, cross-thread direct progress, hardware refinement, performance parity, and workload-general speedup remain open. |
| Same-device persistent D2D SDMA | Two distinct persistent device-local allocations on one exact gfx942 directional queue pair | Experimental runtime-integrated window custody | R23 gives the source a read lease and destination a write lease, rejects allocation/backing alias and mapped overlap, and publishes one through 63 canonical D2D packets on the fixed H2D child with one doorbell. Pending and timeout retain both owners; exact aggregate completion, paired settlement, and paired frontier retirement precede reuse. Ambiguity quarantines both owners and poisons the session. The public facade supports multi-window copies and dirties only the destination shadow after authenticated retirement. Independent executable and Verus models cover the bounded transition surface, and a matched depth-one KFD/HSA/HIP harness validates both physical allocations outside timing. No MI300X R23 result, executable/native refinement theorem, or performance claim exists. H2H remains unsupported and is outside the V1 copy-engine profile. |
| Background stream progress | Send-capable runtime backend with portable flush | Experimental opt-in host scheduler | `spawn_with_progress` owns a bounded registration roster and bounded cyclic flush budget independent of event polling. R24 atomically pairs an event with its exact source stream, polls before flushing, and retires paired progress before exposing conclusive future readiness while preserving explicit logical/native custody. Ordinary `spawn` remains observation-only; registration drop never cancels, releases, or finally flushes. Retryable flush failures are retained and terminal ambiguity seals the engine. Direct KFD is intentionally non-Send; a feature-gated Worker V5 child preserves KFD main-thread ownership while making the address-free adapter Send-capable. Its exact 63+2 native qualification passed once on an idle MI300X at commit `0631c5be`; this narrow result is not general liveness, fairness, parity, or performance evidence. |
| Persistent compute storage bridge | One exact full-allocation H2D result over fresh or exact-size pooled storage, one device-local global binding, one fixed packet, primary gfx942 compute lane | Experimental runtime-integrated single-use custody | R25 reuses the same mapped HBM allocation through metadata-inspected compute without a second user-data allocation or host round trip. Admission binds the authenticated H2D digest to the runtime shadow; lane-zero contention preserves pending ready custody; selection forbids generic materialization; completion restores and retires the exact allocation frontier. A no-effect full-ring publication retains prepared custody for explicit progress; prepublication cancellation restores the exact ready allocation and releases runtime reservations without false profile events. The bridge serializes against every published SDMA submission and active compute lane; it does not provide overlap. Foreign-queue rejection preserves the exact retryable owner, while uncertainty after native retention leaves opaque teardown-only custody. Address-free performance observation reports `PersistentDeviceReused` and zero user-data materializations. R26 commit `8953f757c6771823e5132708f45a43c32f459081` adds the matched counterbalanced KFD/HSA/HIP 1 MiB in-place harness. R27 retains and replays exact dispatch control; R28/R29 narrow hot currentness while retaining full lifecycle audits; R30 replaces timed post-H2D payload reread/hash work with a bound full-write certificate. The retained [R30 measurement](evidence/mi300x-r30-authenticated-h2d-2026-09-05.md) reports a 60.92% median unadjusted E2E reduction versus the exact R29 baseline, but KFD still measured about 3.30x-3.34x slower than HIP E2E. R31 found no meaningful gain from one-packet request specialization. Partial or padded pooled ranges, multiple bindings, auxiliary lanes, overlap, XGMI, broader hardware refinement, Rust/native refinement, parity, and orders-of-magnitude performance remain unavailable. |
| Distributed kernel execution | N/A | Unavailable | General multi-device kernels, synchronization, and overlap contracts remain open; there is no unified native multi-device compute owner |

The principal parity blockers remain ordinary public source-to-GPU authority;
broader native scheduling and concurrency beyond the bounded two-lane,
caller-flush path; full persistent memory, pool, and concurrent-range behavior;
unified compute plus native XGMI custody; production atomics/collectives with
native litmus evidence; broad Rust/device-language support; authenticated GPU
execution profiling; broader target and reset qualification; and concrete
Rust/native refinement. The R26-R32 single-device measurements do not close any
of those gaps or support a generic parity or orders-of-magnitude claim.

The simulator does not model GPU time, occupancy, cache behavior, physical wave
scheduling, or performance. It does not predict performance or prove GPU
equivalence or race freedom.

## Kernel semantics

| Surface | Status | Notes |
| --- | --- | --- |
| Fixed-width integers and booleans | Qualified subsets | Fixed-width wrapping and comparison semantics are explicit |
| F16, BF16, F32, F64 scalar operations | Qualified simulator/compiler subsets | Simulator uses pinned software IEEE evaluation; unsupported transcendental operations fail closed |
| 1D/2D/3D launch indices | Qualified subsets | Typed logical indices; target layout is explicit |
| Checked global buffers and views | Qualified subsets | Allocation-relative bounds, access, initialization, and provenance checks |
| Workgroup memory and barriers | Qualified subsets | Static scalar LDS plus one explicitly sized reachable canonical dynamic LDS base, with convergent barriers and exact initialization/publication/lifetime checks within defined limits; multiple bases and `DynamicAtLeast` fail typed |
| Integer atomics and fences | Qualified simulator subsets; typed runtime and unsafe KFD-authority SPI | Exact supported width/operation/ordering/scope combinations only; ordinary KFD advertises none, while an unsafe semantic authority may enumerate exact non-System profiles without supplying a shipped production authority or native proof |
| Wave32/Wave64 collectives | Qualified simulator subsets; typed runtime and unsafe KFD-authority SPI | Logical collective semantics, not physical `EXEC` emulation; ordinary KFD advertises none, while an unsafe semantic authority may enumerate exact workgroup profiles without supplying a shipped production authority or native proof |
| Rust volatile memory bridge | Experimental bounded slice | Authenticated scalar volatile load/store with explicit bounds and access checks; not broad Rust or general device-language support |
| Helpers and structured control flow | Qualified simulator/compiler subsets | Bounded call depth and explicit unsupported diagnostics |
| General Rust `std`, allocation, unwind, dynamic dispatch | Unavailable | Device subset fails closed |
| General inline assembly, external calls, generic address space | Unavailable | No inferred lowering |
| General matrix/MFMA/WMMA and workload libraries | Partial bounded slices | Not a stable general kernel surface |

Refer to the [implementation roadmap](implementation-roadmap-v2.md) for exact
open semantics. A passing example or test does not generalize beyond its stated
contract.

## Debugging

| Capability | CPU simulator | Live KFD | ROCgdb adapter |
| --- | --- | --- | --- |
| Dispatch/workgroup/work-item hierarchy | Qualified logical view | Bounded topology/queue facts | Tool-dependent, not generalized |
| Logical wave/lane visualization | Qualified | Unavailable as hardware state | Tool-dependent |
| KIR operation and SSA inspection | Qualified | Unavailable | Unavailable unless separately correlated |
| Allocation-relative memory | Qualified | Bounded declared/control facts | Bounded relative inspection |
| Breakpoints and watchpoints | Qualified transcript controls | Unavailable | Bounded MI controls |
| Forward/reverse stepping | Qualified deterministic replay | Unavailable | Forward tool control only |
| Source maps and variables | Bounded compiler-bundle paths | Unavailable | Tool-dependent and incomplete |
| Hardware PC/register/wave state | Explicitly unavailable | Unavailable | Bounded by installed ROCgdb capabilities |
| Structured failure diagnosis | Bounds/barrier classes implemented | Unavailable | Not inferred from a clean stop |
| Agent-facing protocol | Versioned JSONL | Versioned JSONL | Bounded GDB/MI-to-JSONL adapter |
| Simulator/direct-KFD differential | Prepared from exact Bundle V4 plus the admitted structural bridge; Bundle V5 is CPU-simulation custody only | Sealed generated Worker V3 completion only; currently blocked on the unwired protected application verifier | Not applicable |

Logical CPU observations are never relabeled as hardware observations. Native
paths, descriptors, addresses, and unverified declarations are not agent
authority.

## Profiling

| Capability | Status | Boundary |
| --- | --- | --- |
| rocprofv3 dispatch JSON/CSV import | Experimental, bounded implementation | Strict reviewed dialects, KFD identity join, bounded collector custody |
| Dry-run collection planning | Experimental | Produces no fabricated collection recipe when prerequisites are unavailable |
| Direct-KFD runtime observation | Experimental, MI300X-qualified slice | Opt-in bounded lifecycle, host staging, queue, AQL publication/completion, and runtime-authenticated process-local host timing. Fresh recorder occurrences distinguish `Instant` epochs. A low-level KFD GPU/CPU/system counter sample supports clock-domain calibration only; neither source is an authenticated GPU start/end timestamp or rocprof correlation claim |
| rocprof wrapper host-wall comparison | Experimental, MI300X-observed slice | Explicitly authorized alternating raw/wrapped process timing with exact identities, bounded outputs, complete per-leg outcomes, and a caller candidate budget. This is wrapper-path wall time, not counter/PC/ATT/debugger or kernel-capture overhead; empty collector inventory keeps capture overhead and loss unavailable |
| Authenticated GPU execution profiling | Unavailable as protected qualification | Current tests use deterministic/fake collector inputs plus real KFD host observation where gated; no authenticated GPU dispatch start/end timestamps, copy-engine timestamps, or complete source-to-ISA execution lineage is available |
| Runtime/copy attribution | Incomplete | Direct-KFD logical runtime and host staging are observed. A separate opt-in, canonical semantic sidecar and authenticated timestamp-report V2 expose exact runtime-authorized atomic/collective contract declarations without changing frozen Runtime Profile V1; device copy-engine events, semantic execution history, and full treatment lineage remain unavailable |
| ATT/thread trace import | Experimental, authority-free decoded interchange | Strictly admits a canonical external ROCprofiler SDK 7.2.4 callback export with exact manifest/raw/header/library/exporter identities, bounded paging, and loss/incomplete truth. A separate exact supplied decoded-ATT/HSACO/Characteristic binding maps ELF PCs through authenticated kernel symbols to every matching sparse source/MIR/KIR/LLVM/ISA occurrence without exposing symbol names or addresses; raw decoding, authenticated decoder custody, beta collection, and full-grid coverage remain unavailable |
| Source/IR/ISA causal localization | Incomplete | Exact artifact and source-map associations remain open |
| Performance prediction | Out of scope | CPU simulation and profiler import make no prediction claim |
| Agent-native queries | Experimental | Read-only typed facts with explicit provenance and availability |

Optional ROCgdb, rocprofv3, and ROCProfiler SDK components are tools, not fe2o3
runtime dependencies or fallbacks. Their installed versions and measured bytes
are part of the exact workflow admission where required.

## Stability

| Surface | Stability |
| --- | --- |
| Rust crates and macros | Unstable developer preview |
| `cargo fe2o3` command line | Unstable; production route is singular but installation is not released |
| KIR and simulation bundles | Versioned strict formats; no blanket forward-compatibility promise |
| Debugger/profiler JSONL | Versioned strict protocols; capabilities remain additive only where their contract permits |
| Evidence/receipt protocols | Frozen per document version; not a public authority service guarantee |
| Examples | Regression and qualification inputs; not all are runnable production applications |
| GPU compatibility | Exact qualified profiles only; no family-wide implication |

Before relying on a capability, record the fe2o3 commit, exact target/features,
kernel and artifact identities, driver/ROCm/LLVM versions, and named validation
lane. Open a bug when an advertised qualified lane fails under its documented
environment; open a feature request for unavailable combinations.
