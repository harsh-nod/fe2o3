# Local R91 Queue Construction Handoff Evidence

Implementation baseline: signed planning commit
`b66e52d3246a88d76eb30252b9cfe465fa2c9882`, above signed R90
`022e90ea04587f1335aa7d8f679ce4df7b09ee1d`, on both topic remotes.
This packet implements the private
[NATIVE-2A.1 lower-handoff contract](../../runtime-queue-construction-handoffs-v1.md).
Its containing implementation commit records source and evidence together;
publication to the topic branch is not a main-branch merge.

## Status

All seventeen frozen-source gates and eight auxiliary checks pass, with 5,633
non-documentation source identities unchanged. Ten new CPU test functions cover
production-used helpers. Three compiled mutations fail at the intended assertion,
and exact restored source passes. Three read-only implementation reviews found no
blocking issue. Primary owns edits, integration, tests and publication.

R91 is lower-handoff acceptance only. Primary and auxiliary callers now use the
helpers, but their local holders still drop on later constructor error/unwind.
The complete primary root and its integrated acceptance remain NATIVE-2A.2/.3;
auxiliary and replacement/insertion retention remain NATIVE-2B/C.
No SSH, GPU workload, remote staging or solver was started, so no shared-machine
resources required cleanup. No unrelated local build artifacts were removed.

## Final Acceptance

| Gate | Accepted result |
| --- | --- |
| GNU and musl runtime crates, all features/targets | 2,478 passed and five ignored per target, across 48 harnesses each; KFD has 898 tests. |
| GNU all-feature host / musl default host | 258 passed, four ignored / 141 passed. |
| GNU runtime and host doctests | 108 passed. |
| Musl runtime / default host doctests | 91 / 16 passed. |
| Generated macro fixtures | Seven passed. |
| Focused construction / initialization | Six / four passed; these are the ten new functions, not additional unique tests. |
| Existing transition / preparation / bind selections | Pass; exact overlapping counts and filters are retained in the summary and auxiliary record. |
| Lint, formatting and policy | Both Clippy configurations pass with warnings denied; formatting, whitespace, dependency policy/tests, local CI gate and 32 standalone lockfiles pass. |
| Runner/checker tests and production dependency audit | 151 Python tests pass; production metadata closure audit passes. |
| Proof inventory | 686 negative files inventoried; no Verus solver rerun. |

The accepted frozen run is `r91-final`. The [summary](test-summary.json),
[changed-source hashes](source-files.sha256),
[retained-file hashes](retained-files.sha256),
[gate record](raw/r91-final-source-gate.json),
[auxiliary record](raw/r91-auxiliary-results.json) and
[complete source inventory](raw/r91-final-source-inputs.json) retain exact
commands, results and identities. Raw logs and helpers preserve their executed
contents, including Unicode and whitespace; final source/documentation whitespace
checks exclude the `raw/` archive. Mutation receipts record source before and
after each execution; retained minimal patches reproduce those exact differences.

## Scope And Oracles

Six construction tests cover all three actual ring-token profiles, foreign and
closed-session prechecks, and ten admitted error/panic modes per profile.
They exercise R88 fake-native transitions, not a duplicate memory engine.
Exact retained precursor/successor identity, raw native record, original panic
payload and existing completion-Host/device charges are checked without cleanup.
Borrowed preflight matters: R88 does not take custody of inputs rejected before
admission, so the actual rejected token must remain in the caller's stage.

Resource-prefix tests preserve the exact present roster through missing fields,
size/VM/geometry mismatch and queue-ID exhaustion. All borrowed validation precedes
owner extraction. Success transfers once; occupied destinations reject without
loss. Each native mapping/publication coordinate is independently substituted
and rejected. Existing geometry planning supplies sizes; the fixture's enlarged
VM aperture accommodates the actual CWSR extent without validating its contents.

Four engine tests retain the original backend/foundation across opener, take and
authentication errors/panics; repeated attempts run no callback. Admission checks
retain the caller's exact authority on precommit rejection, and transfer only to
the engine on success or the existing revision-exhaustion terminal path. These
authorities are synthetic model fixtures, not Linux allocation evidence.

## Intermediate Attempts

`r91-first-construction` failed compilation because the factored module needed
an explicit path. The first formatting attempt hit the same path issue; its
output is in the execution transcript, not separately archived. The corrected
`r91-second-construction` passed eight selected tests before later fixture
strengthening.

`r91-kfd-local` passed 897 tests and failed one fixture assertion: changing a
synthetic publication ID does not invalidate model admission, which intentionally
creates fresh publications. The model-invalid case now changes a nonexistent
mapping ID; separate native-validator tests cover all publication substitutions.
The corrected initialization selection passes four tests. Strict KFD Clippy
preflight also passes. These attempts are retained separately and do not replace
the final frozen acceptance run.

## Mutation Oracles

Each isolated mutation compiles, exits Cargo with 101 and fails exactly one test.
All mutated files were restored to their exact original hashes before the final
gate suite; no mutation remains.

| Mutation | Intended rejection |
| --- | --- |
| Remove the borrowed CPU-token preflight | Foreign-token rejection loses the actual token instead of retaining it in the ring stage. |
| Store the returned foundation only after authentication | Authentication failure leaves the initializer without its returned foundation. |
| Extract context-save before borrowed resource validation | Rejection no longer preserves the original resource-prefix roster. |

These compiled adapter regressions are not new formal proofs. Model, proof,
resource-accounting, completion, manifest and lockfile inputs remain unchanged
from R73 `fb1e27e66cee27bb11f0b7c08f2d5994c5d168da`; its historical solver result
does not prove this new adapter.

## Open Boundaries

Full constructor retention must preserve the original session/account anchors,
USERPTR terminal threshold, explicit process-gate poisoning, runtime-admission
ordering and unpublished CWSR payload-cleanup contract. No early publication is
introduced. Returned callback values still require outer rooting; arbitrary
values consumed internally and never returned are not protected by this packet.
The primary constructor takes its foundation once and has no closing model retake.

There are no new ring/control/EOP/context-save backing charges. Terminal
retention is not aggregate budget closure. Native generated adoption, ISSUE,
runtime completion/API/graph/drain, teardown/unmap/release custody, new executable
refinement and Linux qualification remain open. No performance gain, full A1/A2
completion or HIP/HSA parity is claimed.
