# Support matrix

fe2o3 is a developer preview. This matrix distinguishes a runnable or qualified
engineering slice from a supported public platform. **No GPU target currently
has a stable public support commitment.**

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
| Direct-KFD diagnostic | Exact admitted artifact and invocation | Bounded qualified lanes | The properties named by that test only |
| Production application dispatch | Ordinary Rust application | Unavailable publicly | Missing general Worker V3 application verifier/release deployment |
| HIP execution | N/A | Unavailable | Not a fallback runtime |
| HSA execution | N/A | Unavailable on the production path | Legacy/qualification code does not define the public runtime |
| Multi-GPU/distributed execution | N/A | Unavailable | Topology, distributed kernel, synchronization, and overlap contracts remain open |

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
| Workgroup memory and barriers | Qualified subsets | Static scalar LDS and convergent workgroup barriers within defined limits |
| Integer atomics and fences | Qualified simulator subsets | Exact supported width/operation/ordering/scope combinations only |
| Wave32/Wave64 collectives | Qualified simulator subsets | Logical collective semantics, not physical `EXEC` emulation |
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

Logical CPU observations are never relabeled as hardware observations. Native
paths, descriptors, addresses, and unverified declarations are not agent
authority.

## Profiling

| Capability | Status | Boundary |
| --- | --- | --- |
| rocprofv3 dispatch JSON/CSV import | Experimental, bounded implementation | Strict reviewed dialects, KFD identity join, bounded collector custody |
| Dry-run collection planning | Experimental | Produces no fabricated collection recipe when prerequisites are unavailable |
| Direct-KFD runtime observation | Experimental, MI300X-qualified slice | Opt-in bounded lifecycle, host staging, queue, AQL publication/completion, and host-monotonic timing; no device-clock or rocprof correlation claim |
| Real GPU-dispatch round trip | Unavailable as protected qualification | Current tests use deterministic/fake collector inputs plus real KFD observation where gated |
| Runtime/copy attribution | Incomplete | Direct-KFD logical runtime and host staging are observed; device copy-engine events and full semantic treatment lineage remain unavailable |
| ATT/thread trace import | Experimental, authority-free decoded interchange | Strictly admits a canonical external ROCprofiler SDK 7.2.4 callback export with exact manifest/raw/header/library/exporter identities, bounded paging, and loss/incomplete truth; raw decoding, authenticated decoder custody, beta collection, full-grid coverage, and source/IR/ISA correlation remain unavailable |
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
