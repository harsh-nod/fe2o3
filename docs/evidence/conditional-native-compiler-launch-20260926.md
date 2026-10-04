# Consuming Native Compiler Supervisor Launch

Date: 2026-09-26. Continuation of the
[retained-custody checkpoint](conditional-native-retained-custody-20260926.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
**No whole M0-M7 milestone or 47/47 production-to-safe-GPU-launch completion is
claimed.** Native inherited root composition and protected deployment validation
remain outstanding.

Base: `c035d542e666cfa25be2c74496609070f785081b`.
Integrated code: `befff4d464dd8a3486d9e700a09019dcc1cc96d5`.

## Implementation

`PreparedCompilerExecutionSupervisorV2/V3::launch` now consumes genuine complete
preparation through the existing retained spawn path. Its trust, three measured
images, listener/root, live anchor, namespaces and both lifecycle leases enter
the charged cleanup slot before atomic clone. There is no V1 authority upgrade,
public provider interface, second reaper or alternate compiler pipeline.

The compiler first revalidates preparation and checks that a controlled alias
of the already installed cleanup guard matches its actual root-bound lifecycle.
It does not install or replace that guard: the anchor must have installed it
before the first child. Both original accounts pay for the alias; it closes
before its request reservation retires. Worker commit `0069752da` supplies this
join and its component tests.

The supervisor ABI remains the fixed eleven roles at FDs 3 through 12 and 220.
Before spawn, the coordinator validates the exact final staged executable,
launcher, issuer, listener/root, policy, signing key, anchor endpoint/pidfd,
supervisor lifecycle, deployment and private bootstrap object. Full transferred
image charges overlap their retained originals and the staged copies.

After spawn the coordinator requires the profile-ready token, original
namespaces, actual child profile and retained preparation checks before releasing
the gate. The shared finite readiness scheduler receives exactly 88 bytes with
no descriptor rights. The owning coordinator decodes native V2 or V3 Ready
against the actual deployment and atomic-clone PID. Bootstrap EOF, repeated
context/profile checks, live pidfd and the deadline precede exec confirmation.
Confirmation releases the artifact spawn lease, not preparation custody.

`RootManagedCompilerExecutionServiceV2/V3` retain the typed child and nominal
Ready. Their metered continuity operation rechecks actual resources, namespaces,
profile, Ready context and child liveness. Complete request quotas include
retained-owner mutex access, all finite readiness attempts, nested validators
and returned growth. Persistent cleanup separately funds the entire preparation
payload. Quotas bound logical work/storage, not syscall latency, generated stack
or RSS.

Every post-clone refusal or unwind drops into the same funded cleanup path.
Pending/quarantined supervisors retain preparation and the live anchor. Exact
supervisor termination permits preparation destruction outside pool locks;
anchor cancellation can then defer into that pool. Even a `Reaped` supervisor
result is not a guarantee that anchor cleanup completed. The joined persistent
guard remains until every slot is terminal and the pool shuts down.

The installed root entrypoint and inherited source composition still use V1.
The new native API has not been exercised through a successful protected launch.
Existing native supervisor binaries do not by themselves complete root-side
production composition, provisioner migration or GPU execution.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled and one bounded
Cargo command at a time. Compiled inputs stayed frozen during each run. The
guard worker edited source only; the primary integrated and executed tests.
The worker's independent launch review found no concrete correctness or quota
undercharge finding, but ran no builds or runtime probes. A subsequent read-only
review also found no concrete defect in the timestamp fixture or EOF-test
isolation fixes described below.

| Check | Result |
| --- | --- |
| Four-crate GNU unit suite, R3, no filters | 465 reported passes, 52 failures, 42 existing ignored; exit 101 |
| Four-crate doctests, R2 | 29 positive and 272 compile-fail passed; exit 0 |
| Fifteen-package all-target check, R2 | Passed with existing warnings; exit 0 |
| Coordinator musl release unit suite, R2, no filters | 67 reported passes, 7 failures, 0 ignored; exit 101 |
| Supervisor musl release unit suite, R1, no filters | 216 reported passes, 42 failures, 42 existing ignored; exit 101 |
| Static coordinator gate, R2 | Static ELF inspection and two fail-closed activation smoke cases passed; exit 0 |
| Changed Rust formatting and diff whitespace checks | Passed |

GNU totals are compiler coordinator 67/7/0, supervisor 216/42/42, external anchor
coordinator 63/1/0 and shared spawn 119/2/0 (reported pass/fail/ignored). Nested
subprocess output is not counted twice. The GNU failures are the same 51 socket
EPERM and one ACL fixture EINVAL. Musl retains the seven coordinator socket
EPERM failures and the supervisor's 41 socket EPERM plus one ACL EINVAL.
The suites are not green. No pre-existing test
was disabled or assertion relaxed. All-targets is not GPU architecture coverage.

One new root-dependent lease test prints an explicit SKIP under uid 1000 and
returns early; libtest reports it as a pass. Its genuine-lease exact/short,
substitution and unwind cases were **not executed** and receive no credit.
Three new rootless guard tests execute independently. Ten new family adapter
tests cover checked layouts, exact returned Ready storage, original-account
boundaries/history, PID mismatch and legacy/other-family framing mutations.
These are not protected-startup tests. First coordinator R1 failed compilation
on a Rustix PID API mismatch; R2 corrected it and completed with the same seven
socket failures, before the full four-crate run above.

The first musl release run exposed a timestamp assumption in an existing image
test: rapid chmod calls need not yield distinct observable ctime values. After
restoring mode, the fixture now explicitly changes and checks mtime before
requiring snapshot-drift rejection. No validator predicate changed, and no sleep,
retry-to-pass or skip hides the failure. The corrected full musl R2 passes this
test and retains only the seven known socket failures. Snapshot comparison
checks observed metadata; it does not prove that metadata never changed between
observations.

GNU R2 then exposed a concurrent-fork race in the V3 program-refusal EOF test
(464 reported passes, 53 failures). CLOEXEC pipe writers can remain open in
another test's child until exec. The existing isolated EOF-test helper is now
shared with this test: probes are constructed only after re-exec in a single-test
process, with exact-name selection, a completion marker written after assertions,
a bounded wait and kill/wait on unwind. All four refusal modes and eighteen
staging-failure positions still run; EOF assertions are neither retried nor
weakened. GNU R3 and the full musl supervisor suite pass all three isolated tests.

The static script builds the existing **V1 root coordinator**, not a native root
entrypoint. It checks ELF64 ET_EXEC, no dynamic-loader sections or undefined
symbols, a non-executable stack, and exact refusals for absent activation and
forbidden arguments. It does not run protected image admission or a successful
deployment. The final executable SHA-256 is
`2b0422cc44cda2cd0de37fe47d340d48277a6fe9501500df53bb01ff6054b493`.

Logs remain in
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
5b4def43864bc7e344eca98fbbb6ba61bf3af1deffb3ceb95e5c2e48f492b4d8  conditional-native-compiler-launch-all-four-r3.log
75df7af39f2f8aeb3d6c089ddba1e023c7d69b2e39b01631547c222db20c10de  conditional-native-compiler-launch-docs-r2.log
33a50ed4091d60a6f5f500209729ca8f33932c204cc8bcaa465c305d83fb033d  conditional-native-compiler-launch-all-targets-r2.log
31853735b5c9cc8ef0453b9c03ba7b329d6b7bc71b6847d5a7f7c0931a0e020c  conditional-native-compiler-launch-musl-r2.log
bedd70b9728d7fb6ab0133f86d7ff0b7d5226eb14472681d093da5d5fb4ae17d  conditional-native-compiler-launch-musl-supervisor-r1.log
bd46a0a94b063a9061a55271798fdf788226c9cc9fdc15cf3ab2e62ad79c244b  conditional-native-compiler-launch-static-r2.log
3a964d720e3502ce43ea5054f06ac2fdb6330ab7c563c19f9b90562c2759e178  conditional-native-compiler-launch-all-four-r2.log
```

## Remaining Gates

1. Exercise combined launch/continuity quota boundaries against genuine prepared
   owners, final staged substitutions, and post-spawn refusal/unwind through
   deferred cleanup. Add coordinator-level malformed Ready/EOF schedules proving
   that confirmation stays unreachable. Component tests and source review do
   not replace these complete-path checks.
2. Migrate inherited root admission and the installed entrypoint to genuine native
   families. Admit the fixed fourteen source descriptors, original root leases,
   source provenance, canonical records and zeroized seeds under full original
   quotas. Install the root guard before anchor launch, then use this consuming
   supervisor path with the same persistent cleanup account.
3. Validate real protected startup, terminal framing, recovery and complete
   deployment custody on the intended host. A static refusal smoke test does not
   authenticate a deployment or trusted-parent provenance.
4. Complete compiler admission/postchecks/finish/revalidation, V5 publication,
   SubjectV3 transport, backend receipts and conditional finalization. Validate
   semantic/machine/numerical proof, generic non-AMD behavior and all 47 tutorial
   kernels on gfx942/gfx950 through that production path.

SSH attempts to mi350, mi350-2 and mi300x failed DNS. No remote job or scratch was
created. Both main pushes of the preceding checkpoint also failed GitHub DNS;
current publication and issue updates require separate confirmation.
