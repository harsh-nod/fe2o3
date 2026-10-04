# Live Read Replay And Native Issuer Entry

This checkpoint is prerequisite work for #272. It completes no issue milestone
and adds no qualification to the 47-kernel production-to-safe-GPU-launch matrix.
It extends [conditional memory and native publication](conditional-memory-native-publication-20260924.md).

## Compiler Changes

The production ranked owner captures every emitted read occurrence, including
its actual Pliron operation, view, coordinate and extent. Replay walks the live
function in recipe order and matches the complete retained roster, checking the
original graph epoch before and after. Foreign operation pointers are compared
against reached live operations before access. Storage and traversal are admitted
on the original resource accounts.

The live conditional ownership query now accepts descriptive read bounds. It
checks complete occurrence coverage, exact dynamic read-only global views,
actual view extents and the selected global coordinate before using input guard
implications. An explicit empty roster still requires complete read validation;
only the separate legacy entry retains the old output-only contract.

This query is not yet the production conditional MemoryBounds producer. The
source-argument relation, same-invocation conditional bounds payload and its
ownership/semantic consumers still need integration. Ordinary unsupported
bounds remain terminal. No raw read list, digest or clean advisory report gains
proof or launch authority.

Actual-source Vecadd tests distinguish the unannotated default tutorial kernel
from an explicitly auxiliary reference-annotated fixture that includes the same
body. The positive consuming test is authored but remains unvalidated pending
the conditional pipeline and protected runtime. None of these tests qualifies
the default tutorial kernel.

## Native Issuer Changes

An explicitly selected native inherited entrypoint uses the existing native
service. Readiness requires matching launch/client/anchor/policy custody and
durable recovery, then one bounded nonblocking pipe write followed by EOF.
Public cancellation checks the exact joined acknowledgment on the original
account. V1 remains the deployment default; no packet-driven fallback is added.

Native static-image packaging shares the existing image checks and requires
independent image/source pins. It produces an inert package, not an installed
native deployment or a provenance receipt. Protected supervisor transcript tests
are compiled but ignored here. They cover readiness/cancellation, not actual
Prepare/Issue observation or compiler receipt publication.

See [native issuer deployment boundaries](../../deployment/native-issuer-v2.md).
Protected native provisioning, actual cross-UID compiler/artifact observation,
full receipt/currentness lifecycle and GPU execution remain outstanding.

## Validation

Runs use pinned nightly `2026-04-03`, locked offline dependencies, one Cargo job,
one test thread, hidden GPUs, a 12 GiB process virtual-memory limit and a
1200-second deadline. Source/tool snapshots must remain unchanged during each
run. Snapshots include tracked and nonignored untracked files and are not Git
tree identities. Counts overlap between selections, not a workspace total.

The final tested implementation was `0b559fbd0b820aad0ea2822235ce135e2a9bae74`,
including the independently landed production changes through `a9b636ec4`.
Two snapshots are used below; earlier component results are not relabeled as
runs of the merged tree.

- A, 7691 files: `8b91e22d40ef6f3441b8b55d8f95f0903633f574667afcb7a9a999cdb94ee578`.
- B, 7838 files: `0955a5b14285cfc5f6ac6782e6b3c56f15985753e1a6f8b0d66109d3a7f578b2`.

| Guard run | Snapshot | Result | Log SHA-256 |
| --- | --- | --- | --- |
| `conditional-live-pliron-supervisor-r5` | A | Pliron: 1582 passed; supervisor: 178 passed, 19 ignored | `0db2b93bf845f344256b3278e85bed451a464850f259d31657caf50be6a891d4` |
| `merged-final-backend-build-r1` | B | Backend lib test build passed | `8ace9815ea67235eaecf2e564063ba39d95ce53a20112e59cd1efb450c62ff2f` |
| `merged-final-vecadd-source-parents-r1` | B | 2 source parents passed, 14 child observations | `e6ff5dcc2880cd2e907fd803b19f48c7eb8a41ef879a537c7f4393d073740fc3` |
| `merged-final-backend-regressions-r1` | B | 710 passed, 26 ignored | `b97c90d3237a4657308fa1d3a80dd8d80fa29b82d23afd60f6642de439df796b` |
| `merged-final-native-api-doctest-r1` | B | 1 compile-fail test passed | `8364a18939b63337f1a014dbc472dabcbab330508a58a3c0aed7d7d9716882f1` |
| `merged-final-native-tests-r1` | B | 514 top-level tests passed, 33 ignored | `95185e0ce4f67ba979ce885972e76c92f5e9061f53f9c09473e251383002a323` |

The native total excludes two nested child test summaries already covered by
their parents. It selects all tests in the broker authority service, compiler
execution client, issuer and supervisor packages. The doctest selects
`serve_native_with_readiness` and reports the intended E0308 V1/V2 manifest
type mismatch, not an unrelated compile failure.

Backend checks select `conditional_`, `production_ranked_projection_v1::`,
`input_guard_`, `consuming_continuation_`, `borrowed_continuation_` and
`ordered_composition`, with `fe2o3-pliron/internal-proof-staging`.
The source run selects only the two exact parent tests
`actual_manifest_vecadd_source_requires_reference_annotation` and
`actual_vecadd_input_guard_cpu_read_and_source_argument_negatives` under
`production_rustc_driver_v1::checked_output_source_v1_tests::conditional_bound_source::vecadd`,
using `--ignored --exact --nocapture --test-threads=1`. Private child roles are
never invoked directly. Guard JSON reports preserve the full commands; the
runner SHA-256 is
`20f8ea8ce430f72e9f2925dc4d79b28739229f36dd6d0c46a45b430caffd3642`.

The default manifest source matches ten CPU/simulator scenarios, rejects two
short-input cases and reaches the named test-only unannotated boundary before
ranked compilation. The three auxiliary mutations are checked in separate
gfx942 and gfx950 compiler captures: a half-length write guard, a wrong CPU
read index and a duplicated source input. Each reaches its exact GPU guard or
reference-read rejection with zero proof-runtime requests. CPU observations
also expose the intended counterexamples.

Both compiler targets use the shared `SimulationTargetV1::amdgpu_64()` profile:
64-bit indices and little-endian scalar layout. These are canonical-KIR CPU
observations, not target-specific ISA simulation, protected proof execution,
machine refinement or hardware evidence.

Earlier failures remain failures. Initial integration runs found fixture
budget/field/genesis assumptions and Rust borrow/error-formatting mistakes.
Source-negative r1 expected a later CPU-bounds error instead of the actual
GPU write-guard rejection; r2 was deliberately stopped to correct the separate
ambiguous-read expectation. Source-negative r3 exposed constant indexing outside
the admitted CPU-reference fragment. The fixture now uses supported dynamic
`point ^ 1` indexing, and the final run reaches the intended read mismatch.
No production acceptance check was relaxed and no arbitrary earlier refusal
was counted as a semantic-negative pass.

Formatting passed for the 32 changed Rust files. Source-size and workspace
dependency policies passed; the latter reports 140 members, eight layers and
508 internal dependency declarations. Native package rejection tests and shell
syntax checks passed. Source-manifest validation reports 50 expected fixtures
with `qualified=false`; the existing `gemm-proof-plan` source binding remains
pending. Lockfile-related digest pins changed without upgrading any status.
Unused staged-helper, existing nominal-policy and fixture warnings remain.

After these runs, `936e4578f` was merged without conflict to preserve concurrent
assembly tests, fixture lockfiles and documentation. A file-by-file delta check
found no production-code changes relative to the tested implementation. This
annotation follows that merge. The Rust results above remain evidence for
snapshot B, not a claimed rerun of the final test-only merge or the whole
workspace. No shared GPU host was used; private test leftovers were removed.

## Remaining Integration

The authenticated source-argument relation and conditional MemoryBounds producer
still need to feed the same-invocation ownership and semantic dependencies in
the fixed nine-stage pipeline. Retained conditional proof consumption, mandatory
descriptor transport, host-premise discharge, the generic conditional finalizer
and applicable machine refinement remain incomplete.

The separately prepared source-relation, proof-retention, descriptor and runtime
patches are outside this checkpoint pending integration and validation.
Protected native deployment and the complete issuance/currentness lifecycle
also remain outstanding. The positive source, simulator, protected-runtime and
target-matched GPU matrix must pass before any milestone or tutorial kernel is
upgraded.
