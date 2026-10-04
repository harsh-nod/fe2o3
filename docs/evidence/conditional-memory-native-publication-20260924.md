# Conditional Memory And Native Publication

This prerequisite checkpoint does not complete #272 M1, another issue milestone,
or the 47-kernel production-to-safe-GPU-launch matrix. Conditional finalization,
host-premise discharge and the actual-source Vecadd launch remain incomplete.
It extends the earlier [formula and preparation checkpoint](conditional-formula-native-preparation-20260924.md).

## CPU Source Binding

The backend distinguishes an unresolved dynamic D1 point/extent relation from
an invalid Rust bounds check. It continues checking all accesses and assertions
before retaining a pending obligation. Ordinary compilation refuses pending
bounds with `FE2O3-CPU-BOUNDS-001`, before opening the proof runtime.

The private conditional source join replays authenticated CPU MIR assignments,
resolved expressions and bounds assertions on the original cumulative resource
account. Each admitted input is joined through raw CPU ABI argument, source
argument, canonical parameter and exact ranked read occurrence. A load must use
the exact point coordinate and have a preceding matching Rust bounds assertion.
Readable access, representable address formation and input/output separation
remain distinct host obligations.

This is a restricted acyclic, single-successor fragment with one checked read
per source input, appearing once in the output value expression. Repeated or
unused GPU read sites, shifted indices, late or unmatched assertions, branches
and nonbounds traps are refused. This is not arbitrary Rust equivalence.

## GPU Bounds Preparation

Canonical read domains are discovered without assuming their own bounds guards.
A second path check can use only exact `i < len(input)` implications from the
retained read roster. Access and address domains remain independent, including
for an empty output or a read executed before the output guard.

Lower-MIR joins canonical read occurrences to checked source arguments and ranked
views before deriving descriptive read bounds. The shared recipe query checks
the complete read roster and each unique, dynamic, read-only global input view's
actual extent. No caller-supplied length can replace that extent.

These helpers are not conditional bounds authority in the nine-stage production
pipeline. The public aggregate still lacks a retained, replayable version of the
lower owner's authenticated canonical-to-source-argument mapping. Physical
parameter ordinals and source argument ordinals may differ. A raw coordinate
vector, hash or caller boolean cannot supply the missing relation. Ordinary
bounds and aggregate coverage continue to refuse unsupported dynamic inputs.

## Byte-Memory Theorem

The verifier now generates a D1 single-store byte-memory transition theorem from
the same checked effect DAG used for scalar equality. It binds exact graph/source
identities, typed roots, CPU subjects, checked load occurrences and the closed
runtime-premise roster. A focused borrowed formula module shares the existing
renderer; ordinary effect replay keeps its previous representation.

The generated model establishes pointwise final-memory equality, preservation
outside the output span, stable input bytes under separation, guarded writes,
coverage and ownership permutation. The obligation domain explicitly names
little-endian bytes and shared abstract IEEE operator interpretation:
`FE2O3/CONDITIONAL-MEMORY-TRANSITION/V1/LE/SHARED-IEEE`.

Recursive state functions and explicit prefix induction prove the transition.
Opaque scalar definitions are revealed in the checked value lemmas; no axiom,
`assume`, external body or raised solver limit replaces a proof. This is not an
LLVM/ISA numerical-refinement theorem, a proof for barriers or arbitrary loops,
or a discharge of the host's runtime premises. The receipt remains callback
scoped and cannot enter the ordinary finalizer.

## Native Publication

The native issuer now has a Worker/external-anchor publication state machine.
Recovery decodes and validates all three journal plans and their exact joins
before mutating any journal. Publication durably prepares the anchor challenge,
reuses that challenge across retry, commits the independently signed anchor
receipt, commits and reacquires the Worker record, marks publication, advances
the issuer, and only then permits an acknowledgment.

Recovery and currentness cannot expose an acknowledgment carriage between
Worker commit and issuer advancement. Currentness requires a fresh external
observation joined to the reacquired exact carriage. The V2 journal shares inert
framing mechanics with V1 without converting V1 records into native authority.

The production serving entrypoint remains V1. These component tests are not a
protected public native session, readiness publication, or distinct-UID
deployment validation. No supplied-subject or permission fallback is added.

## Validation Scope

Local guards use pinned nightly `2026-04-03`, locked offline dependencies, one
Cargo job, one test thread, hidden GPUs, a 12 GiB process virtual-memory cap and
a 1200-second timeout. Source and tool snapshots must remain unchanged during
each run. Content snapshots include tracked and nonignored untracked files;
they are not Git tree identities.

The final source/solver run passed 35 parent tests, including 14 fresh Verus
processes: three accepted cases and eleven deliberately invalid mutations
rejected for semantic failures. Syntax errors and timeouts do not count as
successful rejection. The development runtime is user-writable and earns no
protected execution or hardware credit.

Actual reference-annotated fill sources reached the expected conditional
preparation boundary on gfx942 and gfx950. The unannotated fixture reached
`UnannotatedRefused`, rather than failing Rust compilation. Default manifest
fill produced checked native output, passed the simulator cases, and rejected
the separate missing-signed-receipt case. None of these checks launched a GPU.

The source/solver run used snapshot A. Afterwards, the CFG path checker moved
into `conditional_total_view_paths_v1.rs` to meet the source-size policy. Its
three function bodies were compared byte-for-byte, allowing only the required
`pub(super)` visibility change. The component, protocol, API and all-target
checks then used snapshot B. These content snapshots precede this results
annotation; source/solver evidence is not relabeled as a run of snapshot B.

- A, 7670 files: `badc5360a936ba171829a44ed8b8d17ee605a66fdea7fb18890d99d92bd76edf`.
- B, 7671 files: `3f5f11e870f8b1c312483a3e3d26494856b90f8c95d5395f43214e7ed0e5f1c9`.

Every successful run below had unchanged source and tool snapshots. Counts
are per invocation and overlap between selections; they are not a full
workspace test count.

| Guard run | Snapshot | Result | Log SHA-256 |
| --- | --- | --- | --- |
| `conditional-memory-final-source-solver-r2` | A | 35 passed | `1434161b601584acd31a421b8bc3e67d3cd6e489eee95bef306fd74f2693303f` |
| `conditional-memory-component-tests-r6` | B | 995 passed, 14 ignored | `a7a181d03380f45e0458e3319e6bee5cb05813c99f5bc14175024ababe9b1511` |
| `conditional-memory-protocol-integration-r1` | B | 35 passed | `fddc8514724549bffa8c1771edae628ffb0cdb59e8e13fe9e8f0d4a27fb30cee` |
| `conditional-memory-api-doctests-r1` | B | 3 compile-fail tests passed | `98325dce0570de359cbf843f2198673ab2f7bc4d69dee959aabd93fa5106d6f9` |
| `conditional-memory-final-all-targets-r1` | B | seven-package check passed | `f6f84706dd935170d5094fd374aa931286d01f115076d1ea6fe7b789c127bc94` |

The seven-package selection is `rustc-codegen-fe2o3`, `fe2o3-verifier`,
`fe2o3-broker-authority-service`, `fe2o3-compiler-execution-protocol`,
`fe2o3-kernel-ir`, `fe2o3-lower-mir-kernel` and `fe2o3-pliron`, with
`fe2o3-pliron/internal-proof-staging`. Component filters cover conditional
analysis, CPU read premises/replay/bounds, native issuer/anchor transport,
journal recovery/publication, ranked projection and consuming continuations.
Protocol integration runs `worker_anchor_journal`, `native_receipt_publication`
and `receipt_publication`; doctests select `CompilerExecutionWorkerAnchorJournalV2`
and `serve_native_preparation`. All-target checking uses `cargo check --all-targets`.
The local guard JSON reports preserve the exact commands; their runner SHA-256 is
`20f8ea8ce430f72e9f2925dc4d79b28739229f36dd6d0c46a45b430caffd3642`.

Failed attempts remain failed evidence. Earlier component runs exposed test
budget/phase expectations and optional-predicate lookup regressions, and one
was stopped for correction. Source/solver r1 exposed a fixture enabling two
`fill` exports; its feature exclusion was fixed before r2. Component r5 reached
the original 1200-second combined build/test limit after a 13-minute rebuild;
r6 reran the unchanged selection and snapshot with the built binaries under
the same limit. No timeout, partial run or setup failure is counted as a pass.
Earlier failed development solver attempts are also retained.

Formatting passed for all 38 changed Rust files. The workspace dependency
policy passed with 140 members, eight layers and 507 internal declarations.

The separate [protected consuming-fill run](conditional-consumer-protected-20260924.md)
tested frozen commit `dec40d0f41445da86fd9253d8aae97f21e0756e8`, not this memory
theorem or these publication changes. It reached real conditional formula
execution and then the expected `FE2O3-COND-FINALIZER-001` refusal; the changed
store was rejected by Verus. It did not run a GPU or qualify a tutorial kernel.

## Remaining Integration

1. Retain and replay the authenticated read/source-argument mapping at the fixed
   Pliron producer boundary, on the original owner and resource account.
2. Add a distinct conditional bounds payload at the existing MemoryBounds slot,
   retaining unrelated ordinary diagnostics as terminal failures.
3. Consume that same-invocation payload in conditional ownership and nested
   effect/semantic ownership, including two-run replay and current-epoch checks.
4. Finish descriptor transport, generated host-premise discharge and the generic
   conditional finalizer. Validate actual-source Vecadd through safe launch.
5. Integrate and validate the protected native public service and readiness path,
   then run the required simulator, protected-runtime and hardware matrix.

No issue milestone or tutorial qualification is upgraded by this checkpoint.
