# Conditional Generated Fields and V4 Transport

This follows [prepared conditional Vecadd replay](prepared-conditional-vecadd-20260925.md).
It integrates the generated-field check into the existing production replay and
adds inert conditional descriptor transport. No #272 milestone closes, and the
47-kernel production-to-safe-GPU-launch qualification matrix is unchanged.

## Production Integration

The existing ranked compilation now requires the generated-field check inside
its retained conditional proof replay. The check borrows the actual materialized
source and authenticated typed roots, after fresh CPU/source checks and signed
receipt reimport, on the same original resource account. It does not reconstruct
an owner from hashes or create a replacement budget.

The relation joins canonical parameters, source and adjusted ABI arguments,
semantic locals and types to the actual generated fields. Canonical arguments
are unique and sorted; read occurrences preserve order and multiplicity.
Canonical argument order is not nominal field order. Hidden arguments, expanded
arguments, tuples and compiler-laid-out inputs remain unsupported by this flat
Direct/Pair ABI projection, rather than being silently compacted.

Consumer errors propagate through the existing replay postchecks. A successful
callback is not completed replay. The compiler still reaches the typed
`FE2O3-COND-FINALIZER-001` refusal; this check neither serializes the contract nor
grants finalizer, artifact or launch authority.

The actual Vecadd test now checks generated-field observations against the same
retained proof, source coordinates and resource history on gfx942 and gfx950.
Those protected actual-source assertions are compiled but **have not run**.
The source mutation with two identical input reads remains a rejection case;
generic repeated-read tests are not a new accepted actual-source fixture.

## Inert Transport

`ConditionalInvocationContractV1` and the separate nominal V4 descriptor format
carry a mandatory contract per kernel. The bounded codec validates the argument,
ordered-read and premise rosters, identities and nominal field/type/layout joins.
It reuses the existing nominal wire primitives. V4 cannot be decoded or converted
through the public V1/V3 APIs to discard its required contract.

Repeated reads of one input retain independent alignment requirements while
agreeing on the element width. Tests include differently aligned reads and a
canonical table whose order differs from the nominal fields and places the
output first in the canonical table.

The move-only FFI owner validates canonical bytes, zero code-object digest,
content identity and prepaid storage. These are structural checks, not compiler
origin authentication. The `.fe2o3.kd.v4` section name is defined, but complete
ELF family routing and authenticated finalization are not implemented here.

## Validation

The five original guards passed on source inventory A: 8047 files,
`4e10667293a06befee2654bb8441d28a42ebc2d5c27572985657dc53b9923a5b`.
The hygiene check then identified explicit panic assertions in the test-only
observer helper. Those assertions moved unchanged into the existing `_tests.rs`
module, with the same helper reexported; production logic did not change.

The combined backend rerun passed on inventory B: 8048 files,
`390bf518063d1d0b93c3aa18ca3aa5a693a24f13fa23cf250d0aa7cb8670f291`.
B also includes this page and its incoming links. Both inventories were stable
through their runs. These are not Git tree hashes. This page was updated after
the rerun; compiler source was unchanged afterward.

The guards used nightly `2026-04-03`, locked offline dependencies, one Cargo job
and test thread, hidden GPUs, disabled HIP, a 12 GiB virtual-memory limit and a
1200-second deadline. They preserved source/tool snapshots and grant no
protected-proof or hardware credit.

| Run | Inventory | Result | Log SHA-256 |
| --- | --- | --- | --- |
| `conditional-generated-transport-r1` | A | Complete lib/integration suites of descriptor, artifacts and compiler FFI: 386 passed | `3e2ba208e487eb8d51785d9fe1e802189011f85cfe0b70243e19522616bcbf8a` |
| `conditional-generated-api-r1` | A | Descriptor and FFI compile-fail doctests: 22 passed | `137a3277216520816be14510aaf9a768709ef48c2dbd0ec5cbcd8a3aecf4d7cb` |
| `conditional-generated-backend-r1` | A | Generated-field and observer tests: 23 passed | `6f95ef53ee9c6520a7a4d79825e35cf4717d93970fc4c9edc98c3cecf00e0b84` |
| `conditional-generated-regressions-r1` | A | Adjacent backend regressions: 76 passed, 11 ignored | `b65a48e814f77c098fe34c43e9335a316a7017ada242a02b4efb6198a0133519` |
| `conditional-generated-production-check-r1` | A | Backend library checked without test-only features | `dbd1f4e688c20ab6c2b1823d03c65ac4920808633e16de3d1fc9180702d6ccfb` |
| `conditional-generated-backend-r2` | B | Both backend selections repeated after the helper move: 99 passed, 11 ignored | `38ff53ed033f3c8fc72b7603ccdf40f6374a9c5d724904092bf497c13adb16d9` |

The backend test commands used `fe2o3-pliron/internal-proof-staging`; the
production library check did not. The regression selection covered conditional
output binding, nominal V3 descriptors, retained phases, reference-effect joins,
Vecadd preparation parsing and Cargo capture. The ignored tests require explicit
actual-source preparation or their parent/runtime environment; none count as a
pass. The 507 passing tests are not a full workspace rerun; the final row repeats
99 of them. The production-library check predates the test-only helper move.

Guard SHA-256:
`20f8ea8ce430f72e9f2925dc4d79b28739229f36dd6d0c46a45b430caffd3642`.
Formatting, whitespace and the dependency-layer policy passed; the latter checks
140 members, eight layers and 509 internal dependency declarations. Compiler
warnings remain. Independent static review found no additional backend bugs.

## Remaining Gates

Protected actual-source replay must exercise the complete retained bridge,
including consumer and late postcheck failures without successful completion
events. Independent complete wire goldens and ELF mixed/unknown-family routing
tests also remain. Generic coordinate and codec tests do not replace these.

Contract serialization from the authenticated relation, conditional finalizer
custody, applicable machine/numerical refinement, native issuance and the existing
generated safe host-launch entry still need integration. Host-premise/runtime
patches remain separate, unintegrated work. No new ISA-equivalence or explicit
floating-point error-bound theorem is established by this checkpoint. No remote
proof job or GPU job was started for this validation.
