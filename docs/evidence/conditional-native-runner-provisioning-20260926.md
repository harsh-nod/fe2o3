# Native V3 Runner And Public Record Graph

Date: 2026-09-26. Continuation of the
[funding checkpoint](conditional-native-startup-funding-20260926.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
**No whole M0-M7 milestone or 47/47 completion is claimed. Installed main,
same-host provisioning and client consumers still use V1.**

Base: `3aa54df204da177f788423d4a15c3643a8f84608`.
Integrated implementation: `80eabfd1294dd79f1280dc2f28713d06a52b8400`.
Native provisioning worker source: `bfee42de2bf9c2e3b9b2d7ea5bfd341d4edeaf47`.

## Fixed Runner

The unsafe dedicated-process `run_inherited_compiler_execution_coordinator_v3`
constructs one original request account and one original persistent cleanup
account from the V3 startup quota. There is no runtime family selector or fallback.
The maximum schedule is 86,400 monitoring attempts and 20 cleanup turns, each
with at most one one-second signal wait. Interrupted waits consume turns too;
the bound is not a wall-clock lease. Monitoring exhaustion is a refusal even
after successful cleanup.

Bounded activation clears the validated environment, blocks termination signals,
admits the fixed V3 descriptor graph, launches anchor-first, reserves returned
growth and publishes readiness once. Ordinary failures after pool admission
cancel foreground service custody before shutdown/draining. Cleanup control is
prepaid before side effects. A denied request-funded wait cannot stop independently
funded pool scans; the failed wait is not retried and the request is not renewed.

Signal restoration is attempted only after successful empty-pool shutdown. A
busy/quarantined/exhausted pool keeps charged custody. Unwind drops managed child
custody before the cleanup controller, and signal-owner Drop does not restore the
mask. The caller must terminate the dedicated process on every return, including
refusal or caught unwind. Whole-service-cgroup termination is an external
service-manager obligation; the runner does not prove eventual reaping.

The startup storage query now includes the actual native runner frame, in addition
to nested admission/launch frames. Request reservations remain until owner Drop
and outer-scope retirement; retained cleanup payload uses its independent account.
These are logical resource bounds, not allocator/RSS/time or child-execution limits.

## Public Records

`CompilerExecutionProvisioningBundleV3::new` constructs the matching issuer
policy, supervisor deployment, anchor deployment, anchor provisioning and client
profile using native protocol constructors on the original ledger. The client
profile owns the single policy. The inputs are borrowed and fully prepaid; the
returned charge is the complete unreserved bundle, not just its metadata.

All executable roles require distinct measurements. Existing native constructors
enforce supervisor, launcher, helper and daemon ceilings. Primary integration also
enforces the issuer ceiling used by root admission, which general policy framing
alone does not enforce. Tests cover that exact ceiling and oversize refusal.

The API accepts public keys only, has typed fixed-shape errors, and restores entry
storage while preserving accepted work, peaks and first-denial history. It is a
pure public record constructor: no authenticated image observation, signing seed,
filesystem installation, publication, launch or compiler/GPU authority is created.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded Cargo
command at a time with compiled inputs frozen. A native worker implemented the
record graph in a private worktree while the primary implemented the runner.
Primary integration added module wiring and issuer-ceiling enforcement. The
worker then independently reviewed runner accounting and ordering. No Qwen was
used; the available native-agent concurrency limit prevented a second reviewer.

| Check | Result |
| --- | --- |
| Full coordinator GNU unit suite | 150 reported passes, 8 failures, 0 ignored, 0 filtered; exit 101 |
| Coordinator doctests | 8 positive and 69 compile-fail passed; exit 0 |
| Fifteen-package all-target check | Passed with existing warnings; exit 0 |
| Full coordinator musl release suite | 150 reported passes, 8 failures, 0 ignored, 0 filtered; exit 101 |
| Changed Rust formatting and whitespace | Passed |
| Independent source review | No concrete accounting or ordering defect found |

All 25 new tests pass on GNU and musl: 15 runner-control tests and 10 record-graph
tests. The eight existing failures on both targets remain seven socket EPERM
failures and one ACL-fixture EINVAL. No ignore/filter hides these in either full
suite. Nested subprocess output is not counted twice. Existing root-dependent
tests that skip their bodies under uid 1000 receive no privileged validation credit.

Runner tests inject effects into a private orchestration interface. They check
ordering, finite limits, exact/short control budgets, independent cleanup funding,
denial preservation and unwind. They do not run genuine root admission/launch at
the complete startup quota, and fake-runtime Drop is not a retained-pool unwind
test. The independent review explicitly retained these coverage gaps. All-target
compilation is not GPU architecture coverage. No protected boot, protected proof
execution or 47-kernel GPU run is claimed.

Logs in `/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
57d5c353167d559edc13530b2683e2a07b937ea64adfb4a2160f5c1471cd8046  conditional-native-runner-provisioning-full-r1.log
16783080920bd439156000a3c596fe9b489300fa8976b5418744ec61c4b3e3cb  conditional-native-runner-provisioning-docs-r1.log
a4ba3dccad05bc7e0830a14e36d8d7b3dd843cb53ae7ed31eb835fadf1d45a86  conditional-native-runner-provisioning-all-targets-r1.log
d68bfd8350ee965592fecd87fc997a0e1e9c0629c836b11f2611235ce9482629  conditional-native-runner-provisioning-musl-r1.log
```

## Remaining Integration

1. Migrate the bounded same-host installer to authenticate measured native V3
   images and durably publish the V3 graph and client-profile-v3 under its existing
   lifecycle exclusion. The pure bundle does not implement this I/O boundary.
2. Change system-manager OpenFile paths, bundle inventory/build gates and deployment
   validation together. Select the V3 supervisor/helper/daemon and the
   `fe2o3-compiler-execution-issuer-conditional` image, not the V2 `--native` issuer.
3. Migrate client-check and Cargo client-profile/handoff/receipt consumers with
   the installed coordinator main. Do not switch main alone or add version fallback.
4. Validate actual root boot, exact/short full-path budgets, staged substitution,
   readiness/EOF failure, post-spawn unwind, pending and quarantined cleanup.
   Then finish downstream compiler/proof/publication/host integration and GPU runs.

Fresh SSH attempts to all three GPU aliases failed DNS and created no remote job
or scratch. GitHub fetch also failed DNS. The earlier origin main push was rejected
as non-fast-forward, so newer remote commits still need a fresh fetch and integration
without force. No main publication is confirmed. Correcting the earlier issue
comment's inaccurate push-status sentence remains pending API connectivity.
