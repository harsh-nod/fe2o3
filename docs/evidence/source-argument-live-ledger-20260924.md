# Shared Source Argument Checking

This checkpoint advances #272 without completing a milestone or adding any
qualification to the 47-kernel production-to-safe-GPU-launch matrix. It follows
[live read replay and native issuer entry](live-read-native-entry-20260924.md).

The subsequent [source-bound conditional bounds checkpoint](source-bound-conditional-bounds-20260924.md)
integrates this relation into the aggregate and fixed analysis pipeline. The
remaining-work section below describes the state of this earlier checkpoint.

## Scope

The existing complete argument-correspondence checker, structural traversal,
representation rules and marker recognition now live in
`fe2o3-pliron::source_argument_v1`. Lower-MIR's existing argument and helper-call
APIs delegate to that implementation, preserving visitor errors and cleanup
precedence. Dead private forwarding functions and unused modules were removed.

The new checked relation borrows the actual source owner, canonical module,
selected function, association and emission trace. It independently replays the
complete entry ABI, including ignored arguments and component projections.
Whole-parameter bindings preserve physical, source and adjusted ordinals as
separate coordinates. Same-typed input substitutions do not become valid merely
because their types match.

Both the relation and its bindings retain the originating work-borrow lifetime.
An address token alone is not persistent account identity: these values cannot
escape an owned-ledger callback or remain usable after resetting the original
work meter. Runtime comparisons still reject other concurrently live meters.
Public scoped-view queries additionally require the original budget slot and
retained storage floor. Query and postcallback checks are prepaid; callback
replacement and storage undercuts are refused without unchecked subtraction or
refunding another account.

This establishes entry correspondence, not executable-body equivalence, storage
reservation ownership, a numerical theorem, or launch authority. Whole-source
translation replay and authenticated CPU-reference joins remain mandatory.
Consumers must also join the selected root/body association, not just the shared
SSA-owner pointer. The relation must remain inside its live ledger scope.

## Validation

The implementation commit is `ffe7ef852f1c32929789bf3ecd07fedee4997b9d`.
Runs used nightly `2026-04-03`, locked offline dependencies, one Cargo job and
test thread, hidden GPUs, disabled HIP, a 12 GiB virtual-memory limit and a
1200-second deadline. Every completed guard reported stable source/tool inputs.
Source snapshots include tracked and nonignored untracked files, not Git trees.

- A, 7862 files: `707cceb71b3ff9855b7dfef677cc53aa2e08552b27f00099eee02c1c4c70503a`.
- B, 7862 files: `f16a5a78519b8a66f47784374fef2533f33a19881c942dec523f1495d908ce34`.

Between A and B, only six lower-MIR imports were made test-only. No checker,
function body or test changed. The backend run rebuilt the non-test libraries
on B. Earlier results remain evidence for A, not claimed reruns on B.

| Guard run | Snapshot | Result | Log SHA-256 |
| --- | --- | --- | --- |
| `source-argument-regressions-r2` | A | Lower-MIR: 626 passed; Pliron: 196 passed | `957119be33aaa396efda6acfb5d4f2c051f261587e30f672ed10cf60005b496f` |
| `source-argument-pliron-full-r1` | A | Full Pliron library: 1588 passed, none ignored or filtered | `c36a301d15a674718b4226c137287d8769ffce93adb8bef4feebc4e3c68983bd` |
| `source-argument-api-doctests-r1` | A | 19 lower-MIR and 2 Pliron doctests passed | `c7f2f8de1f1d8d4ddf0183cbe67504aa372af75f98a656faee19cd2c137c882e` |
| `source-argument-backend-regressions-r1` | B | Backend: 755 passed, 28 ignored | `f7457add2b64ce69bd7977dbd11977489a1e06af080dd04af3c1e8f8c2a6cb3c` |

Selections overlap; these are not additive workspace totals. The first run uses
`argument`, `call`, `parameter`, `conditional`, `physical`, `complete_body`,
`representation`, `shared_relation`, `shared_view` and `public_shared` filters.
The full Pliron run retains both package selections to reuse dependency features,
but filters every lower-MIR root module; it runs all 1588 Pliron library tests.
Doctests select the argument/call views, source relation/binding and their owner
factories. They include the actual E0506 ledger-reset errors and lifetime errors
for escaping relations and bindings, not unrelated import failures.

Backend filters are `conditional_`, `production_ranked_projection_v1::`,
`input_guard_`, `consuming_continuation_`, `borrowed_continuation_`,
`ordered_composition`, `parameter` and `argument`. Guard JSON preserves full
commands. The runner SHA-256 is
`20f8ea8ce430f72e9f2925dc4d79b28739229f36dd6d0c46a45b430caffd3642`.

The initial build exposed four helper-call compatibility errors, subsequently
fixed. The broad lower-MIR/Pliron run was deliberately stopped after nine minutes
during exhaustive loop-unrolling resource tests. A narrower-package rebuild was
also stopped to retain the already-built dependency-feature configuration. Both
stopped guards recorded SIGTERM with stable inputs and drained process groups;
neither is a passing suite. The full lower-MIR suite remains uncompleted here.

Formatting for the 32 changed Rust files, source-size hygiene and dependency
policy passed. The dependency policy reports 140 members, eight layers and 508
internal declarations. Staged-helper, test-only forwarding-helper and existing
backend/fixture warnings remain. No protected proof execution or GPU run is
claimed, and no tutorial qualification status changed.

## Remaining Work

The conditional aggregate still needs to retain and consume this mapping inside
its original ledger scope. Source-bound allocation origins, exact live reads
and the mandatory conditional MemoryBounds result must feed the existing
nine-stage ownership/semantic pipeline and its fresh replay.

The separately prepared proof-retention, descriptor and host-runtime changes
remain outside this checkpoint pending integration and validation. Conditional
target admission, applicable machine/numerical refinement, authenticated safe
host launch and protected native issuance also remain incomplete. Positive
source-to-GPU tests must pass before updating any milestone or kernel status.
