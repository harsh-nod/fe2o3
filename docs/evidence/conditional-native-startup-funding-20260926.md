# Native Startup And Cleanup Funding

Date: 2026-09-26. Continuation of the
[activation checkpoint](conditional-native-startup-bounds-20260926.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
**No whole M0-M7 milestone or 47/47 completion is claimed. The installed runner
and provisioning still use V1.**

Base: `ef2f0c11ed31f6627d176e53865393cc013baaf9`.
Integrated implementation: `08716ae0160771f6d332f53db39162bb70b07ee8`.
Native anchor worker source: `2a816d5a1438bc9c51f545c3a163d1ace166677d`.

## Composition

`InheritedCompilerExecutionDeploymentV2/V3::startup_quota` now computes separate
request and persistent-cleanup work/storage limits before any admitted owner
exists. Both monitoring and cleanup turn counts must be positive; variable sums
and products are checked. No account is constructed or renewed by the query.

The request plan covers bounded activation, signal installation, inherited
admission, guard installation, anchor launch, compiler preparation and launch,
readiness publication, finite continuity monitoring and signal restoration.
Each monitoring turn allows one signal wait, one continuity check and one full
pool scan. Each cleanup turn allows one wait, one scan and one shutdown attempt.
Cancellation and the first shutdown attempt are already prepaid by launch and
cleanup admission respectively.

The independent cleanup account funds its fixed pool, one guard installation,
two guard clones, the complete retained compiler-preparation payload, every
scheduled full-pool scan, and additional shutdown attempts. Pending/quarantined
custody remains charged; exhaustion does not authorize releasing the guard or
renewing the account. Funding finite turns cannot guarantee eventual reaping.

Maximum preparation, revalidation, launch, continuity, transfer and retained
storage queries share the actual component calculations. They use fixed native
family image ceilings and conservative owner-size bounds, never synthetic
capabilities, processes or digests. `Image::file_storage_for_length` shares the
actual source-charge calculation; zero and arithmetic overflow refuse.
`Cleanup::pump_work` shares the actual finite scan bounds; `shutdown_work`
exposes the existing additional-attempt charge.

These are root-coordinator logical envelopes, not generated-stack, allocator,
RSS, elapsed-time or executed-program limits. Child services and compiler/proof
execution have their own accounts and admission obligations. The installed
entrypoint does not yet consume the new plan; no successful native root boot
within these limits has been demonstrated.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded Cargo
at a time, compiled inputs frozen during each run. One native worker implemented
anchor bounds privately and reviewed the composed funding. The primary integrated
the work, implemented compiler/root/shared accounting and ran validation. No
Qwen worker was used.

| Check | Result |
| --- | --- |
| Four-crate GNU unit suite, no filters | 337 reported passes, 11 failures, 0 ignored; exit 101 |
| Four-crate doctests | 19 positive and 155 compile-fail passed; exit 0 |
| Fifteen-package all-target check | Passed with existing warnings; exit 0 |
| Full coordinator musl release suite | 125 reported passes, 8 failures, 0 ignored; exit 101 |
| Changed Rust formatting and whitespace | Passed |

GNU totals are coordinator 125/8, anchor coordinator 77/1, shared spawn 120/2,
static executable 15/0. Nested subprocess output is not counted twice. Failures
remain ten socket EPERM and one ACL setup EINVAL. Musl retains the coordinator's
seven EPERM and one EINVAL. No ignore/filter was added to hide these failures.
The older root-dependent guard test skips its body under uid 1000 while libtest
reports a pass; it receives no privileged validation credit.

New tests cover maximum queries against actual component fixtures, independent
closed-form bounds, overlap and overflow, exact monitoring/draining increments,
and the persistent payload floor. Existing exact component budgets still pass
after factoring the calculations. These are not complete successful root
admission/launch tests. The source-only independent review found no concrete
accounting defect and explicitly retained that validation limitation.

Logs in `/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
64364304f867858b40e43cdd0c158c6cc349277bc8d08600f68ef89b2ce72e28  conditional-native-startup-funding-four-r1.log
d96660a65731807fc544b65557798c2491cda7dddee4743e16b5d9e728f4983a  conditional-native-startup-funding-docs-r1.log
b40046e68b446183a04a4ac473bcccd44ffa567a052ce02a5047e99a74c0817d  conditional-native-startup-funding-all-targets-r1.log
e77fdd51bd0de607cb2594abcca4e645791bc2df9c1b03abb231dea9d56b603a  conditional-native-startup-funding-musl-r1.log
```

## Next Integration

1. Consume the plan in the fixed V3 root runner. Preserve both original accounts
   across admission, launch, monitoring, cancellation and cleanup; restore signals
   only after terminal cleanup. Retain custody on refusal, exhaustion or unwind.
2. Migrate provisioning records, client-profile-v3, system-manager paths, bundle
   inventory and deployment validation together. Package the native V3 supervisor,
   helper and daemon with `fe2o3-compiler-execution-issuer-conditional`; existing
   issuer `--native` packaging is V2. Migrate V1 client-check/Cargo consumers too.
3. Run genuine protected root boot, exact/short-budget, staged-substitution,
   readiness/EOF failure and deferred-cleanup tests. Then complete downstream
   compiler/proof/publication/host integration and the full 47-kernel GPU matrix.

Fresh SSH attempts to all three GPU aliases failed DNS; no remote job or scratch
was created. The prior origin push was rejected as non-fast-forward, and current
fetches still fail DNS. Newer remote commits must be fetched and integrated without
force before identical main publication can be confirmed. The earlier issue
comment's push-status correction also awaits API connectivity.
