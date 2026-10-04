# Native Anchor Helper Checkpoint

Date: 2026-09-26. Continuation of the
[startup checkpoint](conditional-native-anchor-startup-20260926.md) for
[issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
M0-M7 and 47/47 production-to-safe-GPU-launch completion remain open. This
checkpoint implements dedicated native provisioning helpers; it does not establish
successful protected startup, compiler semantic equivalence or GPU execution.

Base: `6d35f2e6a115ddc7280d9d41a650be81ee4ef462`.
Integrated code: `db7c870c62afb45c0ac431ba9de4cd5cfaa43f6c`.
Test-only synchronization follow-up: `eb87caef246802d1ba5bb529de89616c269e3b5a`.

## Implementation

- `4382d7976`: native lifecycle File transfer and candidate validation on the
  original ledger, without exposing a borrowed descriptor or upgrading V1 custody.
  Exact inode/parent/metadata checks and shared-lock acquisition do not prove
  arbitrary candidate open-file-description equality. Controlled exports remain
  trusted and drop only closes them.
- `db7c870c6`: dedicated V2/V3 helper entrypoints and binaries. Actual policy,
  supervisor, deployment and provisioning contexts compose with measured images,
  native lifecycle/key custody, durable open-or-initialize, exact staged transfer
  validation, readiness send and terminal native-daemon exec. No raw key export,
  legacy-owner fallback or second persistence engine is added.
- The same commit shares mechanical invocation inspection and helper descriptor,
  bootstrap and terminal exec code; the legacy helper keeps its existing table.
  Static helper packaging accepts explicit families, defaulting to V1. The root
  coordinator still launches V1 and is not migrated by this checkpoint.

See the [helper contract](../compiler-execution-native-anchor-state.md#native-provisioning-helper)
for exact slots, accounting, single-use unsafe entry, late persistent effects and
the distinction between readiness delivery and successful protected launch.

A native worker implemented lifecycle transfer in a private worktree; the primary
integrated helper startup, tests and shared mechanics. Worker review found missing
full-image overlap accounting when taking the inherited daemon descriptor. Intake
now prepays that overlap before duplication, with exact-limit and one-byte-short
regression cases. Final source review reported no remaining concrete finding.
The worker ran no builds, tests or network operations.

## Validation

Pinned `nightly-2026-04-03`, frozen/offline dependencies, one bounded Cargo job at
a time, HIP disabled, no incremental compilation and sources frozen during builds.
Evidence is in
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`.

| Check | Result |
| --- | --- |
| Focused R1, before overlap correction | Lifecycle 25 passed; helper 7 passed, 2 failed; profile 17 passed; other tests filtered |
| Final four-crate full R2 | Lifecycle 31 passed, 1 existing ignored subprocess role; helper 10 passed, 3 failed; service 72 passed, 15 failed; profile 51 passed; no filtering; exit 101 |
| Four-thread helper suite after EOF diagnostic improvement | 10 passed, 3 failed; no filtering or ignores; exit 101 |
| Four-thread R2 | 9 passed, 4 failed; revealed exec-observation race |
| Four-thread R3 / final R4 | Each 10 passed, 3 socket failures; no filtering or ignores |
| Final repeated concurrent exec tests | Both families passed 30 consecutive iterations; other tests filtered |
| Four-crate doctests | 10 positive and 33 compile-fail passed |
| Fourteen-package all-target check | Passed with existing warnings |
| Static V3 helper and rebuilt V3 daemon | Both release builds passed |
| Exact ELF checks and production sealed-static parser | Both passed; two explicitly invoked ignored integration tests passed |
| Empty-environment, missing-contract smoke | Both exited 1 silently |
| Changed Rust formatting, script syntax and whitespace | Passed |
| Final code hygiene delta | Passed after test-file classification correction |
| DCO through test follow-up | 112 signed-off commits; no inherited exceptions |

The full run totals **164 passes, 18 failures and one existing ignored role**.
Other runs overlap it; their counts are not additional independent coverage. The
integration check covers supervisor, issuer, client, compiler coordinator, closure
capability, anchor coordinator/service/provisioner, host, cargo-fe2o3, backend,
lifecycle, protected executable and protected profile. Cargo target kinds are not
GPU architectures. The EOF diagnostic change affects tests only and was exercised
in the subsequent four-thread helper run.

Each native helper family exercises 20 refusal/accounting scenarios, including
missing slots, wrong actual contexts, credential/image/template rejection,
before/after-state failure, unwind, work/storage refusal, exact/short image-intake
scratch, failed readiness, post-readiness refusal and malformed-state rejection.
Assertions cover closed source/private aliases, restored storage and retained
work/peak/denial history. Six late-failure cases per family restart against the
same root/key identity and reopen byte-identical genesis instead of resetting it.

Two subprocess exec tests passed, including with four test threads. They execute
a measured sealed minimal static test image, inspect `/proc/PID/exe`, assert the
exact inherited table `3/4/5/202/220/221/222`, and observe that an independent
exclusive lifecycle lock remains unavailable until child termination. These tests
use private rootless profile, running-image, key-reissue and lifecycle admission
hooks. Readiness is not actually delivered in those exec cases. They do not exec
the production protected daemon or establish root-parent provenance.

Repeated concurrent runs exposed two synchronization assumptions: the new
`/proc/PID/exe` may appear before CLOEXEC cleanup finishes, and descriptor
inspection can transiently fail with EACCES during exec. The final test polls
both exact image and descriptor table under its original deadline, retries only
NotFound/PermissionDenied descriptor observations, and still fails persistent
leaks, access failures or child exit. Thirty final iterations passed both
families. These separate procfs observations are not an atomic production
admission proof. The first repeat stopped at iteration 9 with EACCES; a second
repeat was mistakenly started before the rebuild completed and exercised the old
binary, failing at iteration 3. Neither is credited as validation of the final
correction. Their logs are preserved. The final repeat started after Cargo exited.

The test-only included file was renamed to `cases_tests.rs` to match the existing
hygiene classifier, which had reported its explicit test panic as production
code. No classifier or production panic policy was relaxed.

Two new unignored bootstrap socket tests fail `SO_DOMAIN` inspection with
`EPERM`; their cleanup and accounting assertions pass. The legacy helper test
initially reported readiness EOF; improved child-stderr reporting confirms the
same `inspect helper bootstrap domain` permission failure. The 15 service failures
match the preceding checkpoint's socket operations, endpoint-category assertions
and poisoned-mutex cascades. The suite is not green. No production check was
weakened and no failing socket test was skipped. Genuine protected template
reissue, readiness and recovery still need validation in the required environment.

## Static Images

Both source ELFs are preserved from the integrated code:

| File | Bytes | Secure Entry |
| --- | ---: | --- |
| `conditional-native-anchor-helper-v3-db7c870c6.elf` | 6890712 | `0x693e60` |
| `conditional-native-anchor-helper-daemon-v3-db7c870c6.elf` | 6856824 | `0x6921a0` |

Both are ELF64 EXEC, with entry equal to `fe2o3_secure_start_v1`, a non-executable
stack, no dynamic-loader dependency and no undefined symbols. The production
sealed-static image parser accepts both exact images. The daemon was rebuilt
because shared invocation inspection and lifecycle code changed; the preceding
checkpoint's daemon hash is not evidence for the new source.

Builds used `cargo rustc --release --target x86_64-unknown-linux-musl` with
`+crt-static`, static relocation, `-static`, `-no-pie` and the secure entry symbol.
Image/parser/smoke stages used the existing host cache explicitly. The complete
packaging script with its separate profile-test cache was not executed. V2 was
host-checked but not separately built as a static release image.

These source ELFs are not admitted sealed runtime capabilities. Missing-contract
smoke proves refusal, not which individual prerequisite caused it. No successful
protected deployment or GPU launch is claimed.

## Evidence Digests

SHA-256 of files in the evidence directory:

```text
39f8a13b42831c7a59dcba525a779f9f42f1037fe7126fd6116b3185f83ab0c3  conditional-native-anchor-helper-all-targets.log
1a410bf22e573418ebfd2133aae46187f4a2cf272a8f7d52d76ddf63481e36f0  conditional-native-anchor-helper-daemon-static-build.log
3ed070521a8f9291ad47f54f2818238bc05d6f19853544e016a28489852eb0c4  conditional-native-anchor-helper-dco.log
fe82b48d3f601e15cb1a9989504ccb81f44d285f96e970f8bdd786d8d8604162  conditional-native-anchor-helper-dco-r2.log
6590c2814d775e9a129cbd306231e8d4bff3153cf745418fc02f3cde6cbdea80  conditional-native-anchor-helper-doctests.log
c7dd62def95851f9610c33207160ad56649735e15b789a14f14ba68bd4c3f22d  conditional-native-anchor-helper-exec-repeat.log
9b096a1b6e7245a386c428c8c5c3d747b2003e52a7990d92d77c1302a5827ce0  conditional-native-anchor-helper-exec-repeat-r2.log
246bc21b1a37abcd8948c8e694c8f9980c4dcc29b6ba4b0bde6dcd1d22318408  conditional-native-anchor-helper-exec-repeat-r3.log
b54eeb26c200b03ca32dacf146979ffd32ea4b73703760b5a94f92ab75285f59  conditional-native-anchor-helper-focused-r1.log
c9ded749d674c7a0700ea511ef4c7657eb6ff3e200e6539bf1d18fbfc666099a  conditional-native-anchor-helper-four-thread.log
a6d03b6d6edd267e5a586b814779974906a271687ca05cb30e7a02a1a75841df  conditional-native-anchor-helper-four-thread-r2.log
c86ca85775b40503d5d7128ac52ca32a1acf43b8bccc6ab00b2c9c900b4e4d9e  conditional-native-anchor-helper-four-thread-r3.log
3945f34c12c33dea3b128c4265b50b3972e4c63baae4427e85e2432c88676de4  conditional-native-anchor-helper-four-thread-r4.log
a78cc7f8f81141e28e9dab8678276fddec796ce26828e80c2aff05144fd339d5  conditional-native-anchor-helper-full-r1.log
a2c036725cdf455f5f431adfb56e0153ba065a0ada2e9d7f85e09e3366146dbf  conditional-native-anchor-helper-full-r2.log
e468e71a2caddddd881adf699ebceb129326c232420f69c9357e29b3e451e91f  conditional-native-anchor-helper-hygiene-r2.log
4db781a53c399b7a1c21b473847ed71bc3e9a98a72bac15fa7ca81495a083573  conditional-native-anchor-helper-static-build.log
eee6f379688d47905809de7f76f3a4e9f7a53cda11fc848db4b7357a16b0d967  conditional-native-anchor-helper-static-elf-check.log
6b7857715fcdac56b04feb698f5bc23fac9bc29ab72ab3d72db7c85b19be1e64  conditional-native-anchor-helper-static-profile.log
b63744fdb0282c6731f8fa9f3a1dba258f48acc70a2db2d9721d89dfc3fe12a3  conditional-native-anchor-helper-static-smoke.log
9a1c0002f02f69832ba9adf6b7f6ebaa5c5054ca5604e8070e071213d0ec1bad  conditional-native-anchor-helper-static-daemon.readelf.txt
a7d91633d93111ebdf7e348b101835f1f0a8c2d7db4b878913e9066c7b4d6acb  conditional-native-anchor-helper-static-helper.readelf.txt
fbd274992a0ff714c544841e1879ab9cdb634fbe930838e6909e578b802e5f4e  conditional-native-anchor-helper-daemon-v3-db7c870c6.elf
f902c6ddfe63092f15f3f3bdd0f9fcaf2057771463d9985b75cecabeaf9aee9d  conditional-native-anchor-helper-v3-db7c870c6.elf
```

## Remaining Gates

1. Integrate native root coordinator preparation and launch with actual policy,
   supervisor, deployment, provisioning, image, key and lifecycle owners. Meter
   each wait/release/reap attempt on the original ledger and validate exact
   readiness descriptors, exec EOF, live pidfd and protected endpoint identity.
   Execute genuine protected reissue, startup, recovery and cleanup.
2. Complete native compiler admission/preparation, postchecks, invocation finish,
   revalidation, V5 publication and SubjectV3 transport through the single
   production pipeline. Backend receipt admission remains V1 and conditional
   finalization still refuses; helper startup does not close those gaps.
3. Complete source/machine/numerical proofs, generic non-AMD validation and all 47
   target-matched GPU runs. Numerical differences need explicit compiler-proved
   bounds, not unchecked tolerances.

The next coordinator work starts with shared, funded finite cleanup custody below
the supervisor and coordinators, reusing `process_cleanup` and
`process_reaper_native` rather than adding another reaper. Reserve cleanup capacity
and emergency-transfer work before clone. Keep the artifact spawn lease through
confirmed exec or terminal disposal, including errors and unwinds. Pending or
quarantined children remain charged; ECHILD is not proof of successful reaping.
Then implement bounded native spawn, native preparation, one shared metered
readiness scheduler, managed anchor transfer/lifetime and compiler-coordinator
wiring in that dependency order. A forked copy of a Budget cannot account child
attempts against the original parent ledger.

Fresh SSH probes failed DNS for mi350, mi350-2 and mi300x. No remote job or scratch
directory was created. Source worktrees and reports were preserved. Publication
to both remote main branches and the issue remains unconfirmed; no tutorial-site
update or new end-to-end kernel completion is credited by this checkpoint.
