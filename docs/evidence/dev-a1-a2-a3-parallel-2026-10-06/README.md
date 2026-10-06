# A1, A2 And A3 Parallel Qualification

Date: 2026-10-06 UTC. Base: `a9206b8f598117067aa572e6d2bafdd1ef875844`.
Three native collaboration agents owned separate implementation/proof lanes;
the primary reviewed integration, ran remote CPU qualification and assembled
this record. No GPU execution or performance benchmark was attempted.

## A1: Retained-Launch Accounting Composition

Two CPU tests compose the existing ordinary repeat-launch gate, Context and
KFD backend at 1024 pending launches across eight independent streams, for
three reuse rounds each. They separately exhaust payload bytes and record
credits, require exact refusal without partial admission, release a competing
reservation and admit the final launch. Queries must not progress work or
refund credits. Interior cancellation must refuse without changing state;
mixed-stream tail cancellation must settle every submission and refund each
unaliased payload. Three retained Arc aliases keep their charges through
terminal release and refund only on their final drop. Normal Context shutdown
must refund table charges.

This changes tests only. Storage is synthetic; no device/queue token exists.
It is not native publication/completion, protected generated-scale execution,
the parked historical primary-queue candidate, aggregate-memory closure or A1
completion.

## A2: Reader Proof Requalification

Two existing queries in `context_read_invariant_v1.rs` use
`#[verifier::spinoff_prover]` to isolate solver context. No theorem, executable
body, precondition, postcondition, mutation, classifier, thread count or time
limit changes. Diagnostics and complete-root qualification are recorded
separately; filtered diagnostic verification is not qualification.

The exact campaign passed positive runs before and after all 16 existing
invariant-sensitivity mutations, under the original 120-second per-case limit
and four verifier threads. Both positives verify 157 obligations with no errors;
each mutation verifies 156 and fails exactly its added test obligation. Raw
records and reconstructed case sources were independently reviewed before the
source pin was refreshed. A passing scoped campaign does not establish a
passing broad Verus suite or whole-runtime refinement.

The qualified source SHA-256 is
`d5c37bc57af77cc4bb3d262920c262c7012da8d78c85ac8c0de11c3c1fd3aeb1`.
All 776 source-pin checks, the 694-file expected-negative inventory audit and
the 190-file pinned Verus distribution closure pass. The inventory audit is
not execution of all 694 negatives. The standalone root also passes 156/0;
the complete runtime verification runner was not rerun.

## A3: Gfx950 Queue-Output Prerequisite

Separately branded gfx950 process-slot, doorbell and CREATE_QUEUE output
observations now join the existing read-only resource geometry. The join compares
the caller-retained full GPU ID with the read-only plan, including IDs whose
16-bit mmap hashes collide, and retains the observed UID, topology generation
and both independent profile identities. Neither full ID nor UID is authenticated
by these numeric observations.
Only private numeric decoding is shared with gfx942; old public brands,
manifest bytes and error ordering remain unchanged.

The new output profile SHA-256 is
`e84371e6caa91667ab4279731296dd30989a0d618af8aca9eb72320fecea07fe`.
An independent C oracle checks reviewed-source hashes, verbatim extracted KFD
macros, the actual UAPI struct layout and reviewed SOC15 numeric geometry.
Changed-source input must be rejected before compilation. These source hashes
do not authenticate a loaded kernel or ROCr binary.

This is an inert prerequisite, not native device/VM identity, syscall success,
queue custody, mmap or dispatch authority. Native gfx950 device/currentness,
memory, CWSR/header/shadow custody and a barrier-only create/complete/destroy
probe remain. Kernel execution and two-GPU XGMI additionally need their separate
semantic, route, peer-memory and SDMA gates. A3 remains incomplete.

## Attempts And Corrections

The initial remote setup lacked `futures-executor` in its offline cache. Missing
crate archives were copied into the owned temporary Cargo home, without
modifying the shared cache. Next, `autocfg` could not execute nested `rustc`:
Cargo's build-script library path omitted the pinned toolchain's LLVM library.
The owned runner now explicitly includes that toolchain's `lib` directory;
the installed toolchain and dependency sources were not changed.

The first actual A1 run failed both tests because the initial fixture expected
arbitrary interior cancellation. Static reviews missed that tail-only backend
precondition. The corrected fixture asserts interior `TooLate` with exact
unchanged snapshots, then cancels each stream in reverse FIFO while permuting
the stream order. Every actual cancellation still requires `Cancelled`; no
production rule was weakened. Initial transport and failure logs are retained.

The scale-only runtime test build exposes an existing unused test helper
`MaterializedSourceEventV1::snapshot`; its callers require
`cpu-runtime-fixtures`. Strict lint qualification therefore covers the
production scale library and the combined scale/CPU-fixture library/tests,
without an unrelated source edit or suppressing warnings.

## Qualification Record

CPU Cargo commands run on `mi350` with pinned nightly `2026-04-03`,
`--locked --offline`, two build jobs, nonincremental builds and owned temporary
Cargo/target/tmp directories. Test concurrency is four except the targeted A1
cases, which use one thread. Verus uses the existing local pinned
`0.2026.08.09.92f466f` installation read-only. No shared installation or cache
was changed.

| Check | Result |
| --- | --- |
| A1 corrected 1024-pending scale cases | 2 passed |
| A1 complete payload-accounting module | 13 passed |
| Complete runtime library with `scale-qualification` | 2329 passed, 34 hardware tests ignored |
| Complete runtime library with `scale-qualification,cpu-runtime-fixtures` | 2356 passed, 34 hardware tests ignored |
| Runtime doctests with `scale-qualification` | 71 passed |
| Runtime strict Clippy | Scale library and combined scale/CPU-fixture library/tests passed |
| KFD UAPI integration tests | 63 passed |
| KFD UAPI target-separation doctests | 4 passed |
| KFD gfx950 doorbell-plan tests | 2 passed |
| Complete KFD library | 1981 passed, 2 live-device tests ignored |
| KFD doctests | 56 passed |
| UAPI all-target and KFD library strict Clippy | Passed |
| Gfx950 independent C oracle | Positive accepted; changed source rejected |
| A2 standalone proof | 156 verified, 0 errors |
| A2 unchanged whole-root campaign | 2 positives and 16 expected negatives passed |
| A2 checker self-tests | 16 mutation constructions and 11 adverse-source controls, plus inherited controls passed |
| A2 source, roster and distribution audits | 776 source checks, 694-file roster and 190-file distribution passed |

Scoped Rust formatting and final diff checks also pass. Test counts across
configurations overlap and must not be added as unique coverage. Earlier
checkpoint evidence keeps its original source and target scope. No A1/A2/A3
exit, HIP/HSA parity, physical overlap or performance superiority follows from
this checkpoint.

## Evidence And Cleanup

[`qualification.tar.gz`](qualification.tar.gz), checked by [`SHA256SUMS`](SHA256SUMS),
contains commands, per-stage arguments/exit codes/raw
logs, failed-attempt records, source overlays and post-run identity checks.
Its `agent-evidence.tar.gz` contains all three lane notes, the A2 case sources,
proof and diagnostic records, source/roster/distribution audits, and A3 reviewed
source/oracle inputs. `PROVENANCE.txt` records source transport identities and
the exclusion of the base-source/dependency cache transports, which are not
needed in this evidence archive. The exact base commit and final source overlays
identify the CPU qualification inputs; proof source is separately captured.

All owned solver groups and remote build/test processes were reaped. Remote
postchecks confirm matching final source hashes and runner scripts, record
compiler/Cargo identities, and find no surviving owned build/test process or
other process retaining the owned directory as its cwd. The roughly 1.9 GiB
MI350 scratch tree is removed after complete log retrieval. All three agent
scratch trees are compared with their archive before removal. Shared caches,
toolchain installations and unrelated user evidence remain untouched.
The primary local scratch tree is also removed after the final archive matches
its contents and passes its SHA-256 check.
