# Native Root Spawn Checkpoint

Date: 2026-09-26. Continuation of the
[preparation checkpoint](conditional-native-root-preparation-20260926.md) for
[issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
M0-M7 and 47/47 production-to-safe-GPU-launch completion remain open.
This implements trusted low-level native spawn mechanics, not their integration
into the root coordinator or successful protected startup.

Base: `9a1920e604d1fcef7a01be3d814189c5b1369232`.
Tested code: `34a01815b8b430066e48260e30299527f67a61c8`.
The following documentation changes do not change executable behavior.

## Implementation

- `b5ced0a78`: checked child-work accounting for every supported descriptor count
  and capability ceiling. A private native worker implemented this bounded slice;
  the primary reviewed/integrated it and ran all builds and tests.
- `34a01815b`: unsafe native staging/spawn bridge, full source/duplicate storage
  accounting, prepaid parent/child work and finite child custody. The shared
  syscall implementation supplies atomic pidfd creation for both V1 and native
  owners. No V1 child owner is converted or wrapped into native authority.

The worker also reviewed the integrated source without running Cargo, SSH or
network commands. Its allocation finding was fixed before final validation:
descriptor staging now uses fallible `try_reserve_exact`, returning ENOMEM on
reservation failure. Actual allocator exhaustion was not injected.

The bridge is unsafe because the trusted coordinator must derive conservative
charges from actual source owners and validate the exact final staged Files
under native image/context/key/lifecycle admission. A caller-supplied charge is
not proof. Full borrowed source reservations stay live while full duplicate
storage is charged. Returned storage must be reserved before retaining the owner.
Scopes preserve original work/denial history and restore entry storage on exit.

Child setup is prepaid on the original parent ledger, not a forked account.
Before clone, the parent reserves shared cleanup capacity and an artifact-spawn
lease. The atomic pidfd, slot and lease enter one guard before any fallible parent
check. Cancellation/Drop take one prepaid finite step, then defer unresolved
custody to the same global pool. No native blocking wait, fresh budget or raw-PID
kill fallback is introduced. Missing pidfds and lost wait ownership quarantine
capacity and leases. Only verified exec or exact consuming terminal disposal
releases the artifact lease; exec alone does not release child custody.
Deferred or quarantined spawn leases can indefinitely delay artifact-lock
descriptor release. Cleanup must not wait for those descriptors to close while
retaining its own lease. Finite cleanup funding does not guarantee eventual
reaping or lock release.

Child work is `(166 + n + 3 * (c + 1)) * 1088 + 256`, where descriptor count
`n` is 1..32 and capability ceiling `c` is 0..63. The maximum is 424576;
parent work is 104728 and cleanup reservation costs 96, giving a maximum spawn
quota of 529400. Staging costs 139672. Child observation operations each prepay
4360. These are logical bounds, excluding the executed program and coordinator
readiness, not instruction counts, generated stack, RSS or syscall/mutex latency.
The V1 root child retains its existing synchronous Drop behavior.

## Validation

Pinned `nightly-2026-04-03`, frozen/offline dependencies, HIP disabled, one bounded
Cargo job at a time, no incremental compilation and no source edits during builds.
Final unit tests used four test threads and a short private TMPDIR.
Evidence directory:
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`.

| Check | Result |
| --- | --- |
| Initial spawn R1 | Compile failed: test called a private budget accessor; corrected |
| Spawn R2 | 68 passed; no failed/ignored/filtered tests |
| Spawn R3, with atomic-clone probe | 69 passed; no failed/ignored/filtered tests |
| Three-crate R4 | 148 passed, 1 coordinator socket failure |
| Final four-crate R5 | 337 passed, 37 failed, 42 existing ignored; no filtering; exit 101 |
| Child-lifetime subset repeated 20 times | Each run: 6 passed, 63 filtered; four threads; exit 0 |
| Four-crate doctests | 8 positive and 169 compile-fail passed; exit 0 |
| Fifteen-package all-target check | Passed with warnings; exit 0 |
| Changed Rust formatting and whitespace | Passed |
| Code hygiene delta through tested code | Passed |
| DCO through tested code | 121 signed-off commits, no inherited exceptions |

R5 consists of supervisor 189/36/42 (pass/fail/ignored), coordinator 28/1/0,
profile 51/0/0 and spawn 69/0/0. Earlier runs overlap this coverage. The repeated
subset contains a helper-role entrypoint, not six independent deployment cases;
20 repetitions are not 120 distinct tests. No failing test was ignored and no
production admission check was relaxed.

Staging tests cover descriptor identity/CLOEXEC, full image overlap, exact and
short work/storage, destination count/uniqueness, overflow, original denial
history, unwind and nonroot refusal before clone/reservation. They use inert
Files, not fully admitted deployment inputs. Child-work tests cover all 2048
supported input pairs against an independent operation transcript.

Synthetic descriptor/lease-free records test quarantine and exact pool capacity.
Real child tests run in isolated, deadline-bounded subprocesses with the actual
global native cleanup controller. Command-child tests check liveness, pidfd
duplication, exec confirmation, cancellation/Drop and exact consuming reaping.
A separate probe exercises the same atomic clone/guard-adoption function with
three children: explicit cancellation, a failed post-adoption CLOEXEC check, and
unwind. The gate remains closed and the image is inert. These probes do not prove
a successful root credential transition, helper execution or protected admission.

The remaining full-suite failures reach socket domain/bind/send/peer-credential
restrictions (EPERM) or ACL fixture installation (EINVAL). The coordinator
readiness test fails its peer-credential query. The supervisor parent-death
helper fails earlier at socket admission. A clean-environment rerun remains
necessary; the aggregate suite is not green.

The all-target check includes supervisor, issuer, client, compiler coordinator,
closure capability, anchor coordinator/service/provisioner, host, cargo-fe2o3,
backend, lifecycle, protected executable, profile and spawn. Cargo target kinds
are not GPU architectures. Historical static ELF evidence was not rebuilt at
this checkpoint and does not establish current protected-runtime execution.

## Evidence Digests

SHA-256 of selected logs in the evidence directory:

```text
7822781dc83adf4b70e29dcf5a0de7904fcb502c3f0080d8020c3937c32b3bfb  conditional-native-root-spawn-full-r5.log
e645fd69458a32ab5061b6aca3878324fbcafe4127374a5727edcea13928e838  conditional-native-root-spawn-repeat.log
7db0a27ff03a5a43771aadc9e142241ddc1148b41a8ac4e7c220007b97a46de8  conditional-native-root-spawn-doctests.log
31c6b4ae28c2e48ecd37b92c8d331679e519523669ea4d61d20a4de10c518a20  conditional-native-root-spawn-all-targets.log
```

## Remaining Gates

1. Integrate the primitive into `PreparedExternalAnchorOccurrenceV2/V3`, deriving
   full transfer charges and validating final staged Files under actual native
   owners. Preparation still exposes no native launch method.
2. Share the root readiness engine: independently observe profile/namespaces,
   release the gate, require exact ready endpoint, exec EOF and live pidfd, then
   perform native endpoint admission. Bound attempts/deadlines and preserve full
   managed lifetime/cleanup custody on failure, unwind and normal termination.
3. Integrate compiler admission/preparation, postchecks, finish/revalidation,
   V5 publication and SubjectV3 transport through the single production path.
   Backend receipt admission remains V1; conditional finalization still refuses.
4. Validate genuine protected startup/recovery, source/machine/numerical proof,
   generic non-AMD behavior and all 47 tutorial kernels end to end. Passing local
   API and custody tests does not close these milestones.

Fresh SSH attempts to all three configured GPU aliases failed DNS in this
session. No remote job or scratch directory was created. Source worktrees and
reports were preserved. Publication to both remotes and the issue is a separate
operation, not evidence of validation.
