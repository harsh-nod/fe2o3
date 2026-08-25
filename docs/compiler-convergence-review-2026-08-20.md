# Compiler convergence review, 2026-08-20

> Historical review: the critical importer and route findings below describe
> the 2026-08-20 baseline. The production backend now has one unselected
> transaction from authenticated gfx942 rustc collection through semantic MIR,
> the owner-held middle end, target-neutral Kernel IR, formal-memory admission,
> gfx942 LLVM handoff, and strict Worker V3 publication. Feature-free builds use
> a dedicated production handoff module and do not compile the Worker V2,
> scalar-MIR V2, S09, or semantic-type V2 oracle modules. Current remaining work
> is application-side verifier/effect admission and migration followed by
> deletion of the qualification oracles.

## Scope

This review covers the compiler-facing workspace from rustc collection through
canonical MIR, Kernel IR, Pliron adapters, AMDGPU lowering, LLVM handoff, and
artifact finalization. The review baseline was `5d095d5663`; the target
architecture remains the single transaction in
`production-pipeline-convergence-v1.md`.

The standard for deletion is evidence based:

- code with no workspace caller and no retained compatibility obligation is
  removed;
- exact-profile code with differential or hardware evidence remains an oracle
  until the general route has equivalent coverage;
- transitional raw-Pliron APIs are retired into the closed production session,
  not factored into a larger permanent abstraction;
- unsupported production behavior fails closed and never enters an oracle.

## Findings

### Critical: production still lacks an AMD-target rustc import session

The compatibility backend observes the final crate under a host rustc session.
Its FnAbi and layout facts cannot be labeled as `gfx942` facts. Production must
continue to stop before semantic-MIR admission until both rustc entry paths call
the sole importer under an AMDGPU target session. No cleanup can safely make
`production-v1` complete or default before this is fixed under #176.

### High: one backend exposed fourteen routes without an architectural type

`rustc-codegen-fe2o3` accepted one production route and thirteen compatibility
or exact-profile routes through the same enum and a long parser. The distinction
between production and migration evidence existed only in documentation.

The selector table now has one source of names and an explicit
`PipelinePurposeV1`. An architecture test proves that exactly
`production-v1` is production-capable. All other values are qualification
oracles. Invalid configuration is resolved once before collection or lowering.
Device-function dumps are now verbose-only; differential tests that inspect
rustc export disambiguators opt into that diagnostic surface explicitly.

The remaining oracle dispatch is still physically large. It must move behind
test/tool entry points slice by slice; moving it wholesale before replacement
coverage would discard evidence rather than converge the compiler.

### High: rustc semantics have multiple import boundaries

The repository currently retains:

- `mir_import`: compatibility MIR and Kernel IR input;
- `same_session_rustc_v1`: ordinary scalar qualification custody;
- `mir_import_v2`: S09 compiler-capture observation and hostile recapture tests;
- workload-specific `collected_*` source/MIR recognizers;
- the not-yet-implemented production semantic-MIR importer.

Only the final item may produce production semantic MIR. The other four are
oracles and fixtures. Shared collection/frontend validation was consolidated,
but their semantic import code must not be called from
`ProductionCompilationV1`.

### High: detached Pliron lowering shells duplicate the wrong boundary

`fe2o3-lower-mir-kernel` and `fe2o3-lower-kernel-gpu` repeat registration,
context identity, error, and detached-service machinery around raw Pliron
contexts. They are migration and conformance bridges. A shared raw-context pass
framework would make #140 harder to close. Reuse belongs in the closed
owner-authenticated production session above the dialect crates; the detached
shells are deleted when that session owns their last caller.

### High: exact-profile ownership is still embedded in backend dependencies

The backend directly depends on General GEMM compilation and enables the
General GEMM finalizer feature. This increases build size and allows oracle
types to remain close to production dispatch. Feature-gating alone is not a
semantic boundary. These dependencies can leave the production backend only
after their selectors move to qualification tools and differential tests.

### Medium: collection had quadratic diagnostic state and incomplete bounds

The collector previously cloned the full root-to-function call chain for each
new function. A linear call graph therefore retained a quadratic number of
labels. It now stores one predecessor and one label per function and rebuilds a
chain only on rejection. Normal traversal storage is linear in the collected
graph; deterministic tree/set operations remain logarithmic.

Collection now rejects more than 4,096 reachable functions, more than 65,535
blocks in one function, or more than 131,072 blocks in the closure before later
records allocate from that input. These limits match the existing rustc
frontend boundary.

Remaining bounded costs:

- reachable-inline-assembly reconciliation traverses the call graph once per
  kernel root in the worst case;
- initial kernel counting and actual collection both inspect registration/FFI
  discovery surfaces;
- device-FFI lookup is linear in a hard-bounded set of at most 128 contracts.

These are not current hot-path correctness defects. Optimize them only with a
representative multi-kernel profile and deterministic equivalence tests.

### Medium: dead and side-only compiler state obscured ownership

This review removed:

- the 2,331-line `executable_scalar_control_flow_v1` subsystem, which had no
  caller outside its own tests and was superseded by the active V2 oracle;
- an unused scalar admission entry point;
- unused authenticated-owner registration/hash copies and accessors;
- an unused per-module array describing target properties that no consumer
  checked;
- unused General GEMM receipt accessors;
- broad module-level `dead_code` suppressions.

MIR-V2 recapture and decoding helpers used only by hostile tests are now
compiled only in test builds. The normal backend library builds without dead
code warnings.

### Medium: an integration helper replaced an already-linked backend dylib

The collected scalar/control-flow integration binary rebuilt
`rustc-codegen-fe2o3` in the parent `CARGO_TARGET_DIR`. Later integration
binaries had already linked against the prior dylib and could fail at process
startup with an undefined Rust symbol. The source-correspondence helper now
builds in a private target directory, pins that backend into its existing sealed
memfd, and removes the temporary directory when initialization completes.

The adversarial suites deliberately use clean targets and remain expensive.
Future CI work may cache an immutable, content-addressed backend input, but it
must preserve the tests' path-substitution and build-custody properties.

## Retained compatibility surfaces

| Surface | Why retained | Retirement condition |
|---|---|---|
| `legacy-v1` | Explicit compatibility oracle; never a default or fallback | Equivalent evidence is retained by production-neutral tests or tools |
| `kernel-ir-v1`, `kernel-ir-worker-v2` | Generic compatibility and Worker evidence | Production transaction reaches equivalent inspected artifact |
| `collected_*` selectors | Differential workload and hardware oracles | Matching slice passes general-route differential gates |
| `mir_import`, `same_session_rustc_v1`, `mir_import_v2` | Existing semantic/custody observations | Sole AMD-target importer subsumes each required fact and hostile test |
| detached MIR/Kernel/GPU Pliron services | #140 migration and conformance evidence | Closed session constructs and transforms the same graphs through opaque handles |

The dormant `fe2o3-legacy-compiler` fixture and every compiler API
implementation selector were subsequently removed. The compiler API and driver
now expose one production backend contract; non-authoritative comparison remains
qualification tooling rather than a second route.

## Required next sequence

1. Implement the AMD-target rustc driver adapter and one consuming semantic-MIR
   importer under #176.
2. Make `ProductionCompilationV1` retain the semantic record and owner-held MIR
   graph; prohibit all oracle imports by type and dependency.
3. Complete the closed #140 construction/transformation service and route
   MIR-to-Kernel plus Kernel-to-GPU through it.
4. Reach one inspected scalar/control-flow HSACO and typed host interface through
   only `production-v1`.
5. Switch the default, then remove selectors as differential slices pass.

Do not add another workload-specific `QualificationOracleV1` variant, importer, raw
context callback, textual identity boundary, or in-process profile finalizer.
