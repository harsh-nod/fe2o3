# Conditional V4 Artifacts and Context Fields

Continuation of [contract retention and runtime transport](conditional-contract-retention-20260925.md)
for issue #272. Local integration passes; the separate protected actual-source
replay fails. **No milestone closes and no kernel gains end-to-end qualification.**

## Implemented

The existing HSACO finalizer now has distinct V4 inspection, finalization and
raw-reconstruction APIs. Every kernel retains its mandatory conditional contract.
The entire zero-digest source descriptor must match, only the code-object digest
is patched, and independent reinspection checks the result. V3 and V4 share
physical ABI checks and byte-integrity operations without a public V4-to-V3
downgrade. All inspectors reject mixed and unknown `.fe2o3.kd.*` sections.

These are inert artifacts, not authenticated compiler output. The Worker schema
still admits only V1 and nominal V3. Scratch declarations cover logical typed
extents, not the inherited ELF/AMDHSA parser heap or whole-process memory.

Conditional generated-field queries now derive positions from admitted source
and adjusted FnAbi mappings instead of equating those ordinals with field indices.
Eliding a logical context requires the existing compiler-retained receipt,
matching source digest and root/helper identities, exact forwarding operands,
and a zero-sized, ignored, by-value `KernelContext`. Semantic tags alone do not
authorize elision. Without context custody, the query independently rechecks
transparent-body selection. Temporary maps use the original resource account;
work history persists and storage returns to the caller's floor after disposal.

This query does not make the complete context-first production path available:
upstream canonical helper selection and context admission still need integration.
The ordinary finalizer refusal `FE2O3-COND-FINALIZER-001` and authenticated host
conditional-family refusal remain unchanged. Neither public bytes nor matching
hashes supply proof, publication or launch authority.

## Local Validation

The pinned nightly `2026-04-03` guard uses locked offline dependencies, one Cargo
job/test thread, hidden GPUs, disabled HIP, a 12 GiB virtual-memory limit and a
1,200-second deadline. Source and tool inventories were stable in every passing
run. These inventories are not Git tree hashes.

- A: 8,142 files, `69e9fa8f6461cbcaf8511b480b294841d9181eb8b9bf7492af2efcb4207063ec`.
- B: 8,142 files, `ef9bb0e45b76c5e667a987bb3b450a234b8c190e57c300965ba55361154c8313`.

B adds only three regression tests and their helpers to A. One test rebuilds a
matching synthetic receipt through the existing bind/seal checks and proves that
same-type operand reordering reaches the forwarding rejection, not merely a stale
digest check. The others reject non-flat helper arguments and invalid FnAbi
adjustments at their actual admission boundaries. No production constructor was
opened. Documentation was updated after these guards; Rust code was not changed.

| Guard | Snapshot | Result | Log SHA256 |
| --- | --- | --- | --- |
| `conditional-v4-artifact-integrated-r2` | A | 176 library, 54 public API, 32 Worker tests passed; 2 ignored | `45aae52474b3ea5a2dc21981f207e3f7e7c20706e11719e45ccf52c091ce8cc9` |
| `conditional-v4-artifact-api-r1` | A | 22 compile-fail doctests passed | `1439b6f53d25d8a4c2575b40665470c92d5e1c33ac96c56c207e64d7bace340d` |
| `conditional-context-fields-integrated-r1` | A | 138 passed, 13 ignored | `845d7f9ee90402ff9baca3229ef58a6cacba95324e3c4bf8dc5e425b10bba478` |
| `conditional-context-fields-integrated-r2` | B | 141 passed, 13 ignored | `cffe20eff7e9d6fe939f5c6f2721d34de230232f31e8564b8b42efcc4dd21d75` |
| `conditional-context-production-check-r1` | A | Normal backend library check, no test-only features | `b6a9b7503bc8b9abdcf9f64ed8ee0bd5bbff7b9be397fe42038e6e653a90f227` |
| `conditional-v4-artifact-targets-r1` | A | Finalizer all-targets check | `62c5c02b25ad7a0fe2dfcd78e11eac5653eb7f0605cf7a178b83317157705145` |

These are 403 distinct selected unit/integration tests plus 22 API doctests,
not a workspace-wide run. Ignored tests do not count as passes. Backend unit tests
use `fe2o3-pliron/internal-proof-staging`; the production check does not. Existing
unused-code warnings remain. Guard SHA256:
`20f8ea8ce430f72e9f2925dc4d79b28739229f36dd6d0c46a45b430caffd3642`.

The first artifact run passed 176 library and 52 public API tests but failed two
new fixtures: their purported valid substitutions duplicated an adjusted-argument
ordinal. The fixtures now use a distinct ordinal. No production validator was
weakened. Failed-run log SHA256:
`1aa29b8a07475b00542a2124ecd751aaa5d2e7b7ede6663b7037ca1477abd944`.

## Protected Replay Failure

The MI350 run froze the earlier published commit
`fd1b32e8d36f72a2d478424471287d322a05506e`, before the V4/context changes above.
Its inventory has 8,136 files:
`858efd6fd0363b479facc2063c9d3aed0673322cbcd360db3c7cdea869b7914f`.
Backend/verifier executable selection, genuine two-target preparation, frozen
input provenance and root-owned runtime audit passed. All three Verus controls
passed: installed closure audit, positive proof execution and false-proof rejection.

The actual-source parent then failed on the auxiliary annotated Vecadd shared
body for gfx942. The compiler's reference-effect join reported:

```text
ReferenceBoundsCheck { block: 2,
  detail: "ranked extent %6 is not an exact constant, argument, or index expression" }
```

`production_reference_bounds_v2.rs::extent_expr` refuses this extent before the
expected conditional-finalizer boundary. The run did not complete source-proof
replay or reach gfx950. Passing Verus controls are not a Vecadd proof. No GPU was
exposed, no kernel launched, and no final artifact or launch authority was issued.
The diagnostic runner uses a normalized external image base, not the pinned
production-qualification base; its qualification credit remains false.

Exact preserved evidence in the issue-272 production-next evidence directory:

| Evidence | SHA256 |
| --- | --- |
| r9 diagnostic runner; 69 offline harness tests passed | `35a7efed1d80d38f85421a784b7b78da993f15515bc6ee1c36e0e6a6a2fea82a` |
| Frozen request | `fcc785fc1635815c05747c4b97cd4fa6ba4ea8aa82db4aa5fac7d5292047a2d4` |
| Complete compressed inputs | `4fbcd7cb1ffd09313614671f9e92de1b2143f5d5873f82e844f7bcfad4be4e9a` |
| Complete compressed results and terminal state | `a72ba6b332190e294c9d0551b60178848d2dbb81329c9d0480d05df2ef96d521` |
| Inside report | `ff0affdd410282fb50a71e66fb7dbdb31d3e9f6fa1cd95b14174181e29dd12a6` |
| Drained state | `183c599366e67cad1007b098274389e3274eab0861ee16621762e2e997c59d0d` |

The job exited unsuccessfully and its cgroup was confirmed empty. After preserving
results, identity-checked cleanup removed the root run. All five private remote
scopes and the never-started export container were removed; the proof service and
slice were absent. The shared image, protected runtime and other workloads were
left untouched. The failed job was not restarted.

## Remaining Work

First trace the rejected dynamic extent back to its exact source and canonical
argument, and integrate supported dynamic bounds through the existing conditional
premise proof without treating unknown extents as unconditional safety facts.
Then rerun fresh protected actual-source inputs for both targets.

The production path still needs conditional final-graph custody through target
lowering, applicable machine/numerical refinement, authenticated V4 Worker
finalization/publication, and the existing generated safe host entry on the
original account through completion. Full context integration, hostile
substitutions, simulator and target-matched hardware tests, advanced kernels and
the complete 47-kernel matrix remain. This checkpoint establishes no new ISA
equivalence or explicit floating-point error-bound theorem.
