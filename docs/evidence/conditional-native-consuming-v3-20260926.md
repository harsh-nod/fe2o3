# Native V3 Consuming Fixture and Session Checkpoint

Date: 2026-09-26. Continuation of
[native launch custody](conditional-native-launch-20260926.md), for issue #272.
M0-M7 and the 47/47 production-to-safe-GPU-launch matrix remain incomplete.
This checkpoint grants no protected-proof, numerical-refinement or GPU credit.

## Implementation

Base: `2ffc388a75a2cfed6f8aad7d01735857e25fa585`.
Session implementation: `829183c401f24c55ec0cf893965ee447a6139241`.

- The isolated consuming coordinator now has genuine V3 supervisor and submitter
  roles, measured program admission, handoff, prepared launch, readiness and
  terminal custody. Shared process mechanics do not convert admitted V2 owners.
- The synthetic static issuer selects V2 or V3 at compile time. The builder
  preserves default V2 behavior and emits separately named V3 artifacts. Rootless
  argument/output-directory contract tests run in PR preflight and local CI.
- Negative consuming cases now require public-peer EOF without any publication,
  after cleanup completes. Silence alone is insufficient. Drop-before-readiness
  checks cleanup, not successful child input admission. Cross-family consuming
  substitution remains untested; exact readiness-join tests are separate evidence.
- Native V2/V3 `run_session` composes existing handoff, preparation, launch,
  readiness, publication and terminal wait on the original borrowed ledger.
  Returned growth is reserved under an owner-before-funding guard. Delta refusal
  preserves its exact handoff/preparation stage. Accepted work and first denial
  history survive retirement. Public launch's pre-adoption behavior is unchanged.
- Session limits retain distinct finite native waits and cap handoff at two
  minutes. They do not reinterpret V1's 24-hour serving policy. Cleanup remains
  independently funded and controller-owned; the exited result retains the
  original request-account borrow until Drop.

Two native workers contributed fixture implementation and read-only session
review. The primary integrated changes and serialized every Cargo invocation.
The session reviewer found no concrete defect. Rootless helper tests do not
establish an integrated protected `run_session` execution or stage short-circuit
behavior. Existing `Funded` tests check field destruction order; the new advance
probes count retirements and check account restoration, not that order directly.

## Validation

Pinned `nightly-2026-04-03`, offline, one Cargo job, no incremental compilation.
Local logs are retained under
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`.

| Check | Observed result |
| --- | --- |
| Focused supervisor/launch/consuming/session tests, R2 | 81 passed, 19 ignored, 175 filtered out |
| Supervisor doctests | 2 compile-only examples and 109 compile-fail examples passed |
| Six-package all-target check | Passed for supervisor, issuer, client, host, cargo-fe2o3 and rustc-codegen-fe2o3 |
| Rootless fixture-builder preflight | Passed |
| Local CI dispatch probe | Exactly one fixture-builder contract step selected |
| Host V3 synthetic issuer example | Built; not static ELF validation or child execution |
| Simultaneous V2/V3 fixture cfg | Expected exit 101 with explicit family-selection compile error |
| Separate handoff tests | 5 passed, 2 EPERM failures, 3 ignored; not a full-suite pass |
| Static V3 fixture build | Two bounded attempts ended before any validated artifact |

The two handoff failures are the existing V2/V3 socket-shape tests at
`handoff_native_io_tests.rs:77`; their environmental refusals were not skipped or
relaxed. All-target means Cargo target kinds, not GPU architecture coverage.
Doctests and the integrated check preceded the final test-only strengthening;
the R2 focused suite exercised that strengthening at the recorded source state.

The second static attempt returned timeout status 124 at the 900-second per-build
bound. Both attempts used fresh private output directories, the same warm cache,
one job, pinned musl, and unchanged CPU/address-space/file limits. No issuer was
executed. Logs were retained and empty scratch directories removed. One completed
276,266,960-byte backend test executable was removed from our private debug cache
for disk headroom; no source, reports, active release artifacts or remote files
were removed.

## Evidence Digests

SHA-256 for logs in the directory above:

```text
d695a0c81428063b6bd63f29a6f0e3eb0abf7f00c7cd6d305fdc5308f4efb5af  conditional-native-session-tests-r2.log
d0d5e51bdb3aa84a342ce6a9c8ce776e786356dcc52c3169dc84d3578811183b  conditional-native-session-doctests-r1.log
a4985a64b6c168fada9e42bc9e0419c047567a827bb495f81802b586ef9c815e  conditional-native-session-integrated-check-r1.log
5871bcc71d53a81cb35ec7ba188a4f7a3b7070eab8320b0d3b6832e885672f14  conditional-native-consuming-v3-builder-tests-r2.log
ab44a562029a91a0907ad2b7781c7d240e306f8f0610331e23d5734fcfe69c01  conditional-native-consuming-v3-issuer-build-r1.log
cbbb5fc374337ef8ed4f5f6511851d4edb52b6b6c7a40bc9ca95440fd6dbc651  conditional-native-fixture-family-refusal-r1.log
59e773763a5d405e8f568ab42c98e2612a0c5c03fa9d41401c536c4bb75c8642  conditional-native-consuming-v3-handoff-tests-r1.log
f5b96f0b4fd163f5f7cbd054646a9fa36b0cf12b413be95c037eb55288294f42  conditional-native-consuming-v3-static-build-r1.log
72ae0578c67494b431620c5982fceed0a0885ba0956cacad3561d7f7e53e6e43  conditional-native-consuming-v3-static-build-r2.log
```

## Remaining Gates

1. Finish static fixture validation and execute V3 consuming and integrated
   session success/refusal cases in the isolated protected profile.
2. Integrate native listener, provisioning and recovery, retaining the persistent
   cleanup account and exact independently pinned runtime/policy identities.
3. Connect production backend admission and conditional preparation, postchecks,
   invocation completion, revalidation, V5 publication, SubjectV3 acquisition and
   transport through the normal entry point. Normal admission remains V1 and
   conditional publication still refuses.
4. Complete required machine/numerical refinement and the protected GPU matrix
   across gfx942/gfx950, with generic non-AMD coverage and explicit proved error
   bounds where allowed. No local helper success substitutes for this evidence.

Fetches from both configured remotes and SSH probes failed DNS resolution during
this checkpoint. No remote job or remote scratch directory was created. The
latest remote main state could not be established; synchronization must remain a
normal fast-forward push after successful reconciliation, never a forced update.
