# Native Root Preparation Checkpoint

Date: 2026-09-26. Continuation of the
[helper checkpoint](conditional-native-anchor-helper-20260926.md) for
[issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
M0-M7 and 47/47 production-to-safe-GPU-launch completion remain open.
This checkpoint implements native preparation and shares funded cleanup;
it does not implement native root launch or establish protected startup.

Base: `0cbffbc76e4ca7d41f213894faeeb0170c58bf5e`.
Tested code: `8cabc5ace34ce3f94d2ecbcc00ffe98166679d32`.
The following documentation changes do not change executable behavior.

## Implementation

- `5d1fc7de3`: moves the existing cleanup engine and its tests from the issuer
  supervisor into protected-service-spawn. Old issuer public names are aliases.
  There is one global pool and original persistent account, not a second reaper.
  A narrow unsafe cross-crate bridge states atomic pidfd, exclusive wait,
  pre-clone reservation, exec and terminal-reap obligations. Safe code cannot
  construct child custody or assert exec/reaping through this bridge.
- `bfc2dfb27`: rearms the root child's parent-death guard after credential
  transition and rechecks the exact parent before readiness. Existing failure
  stages are preserved. Linux clears PDEATHSIG when effective or filesystem
  UID/GID changes; arming only before the transition was insufficient.
  See [PR_SET_PDEATHSIG](https://man7.org/linux/man-pages/man2/PR_SET_PDEATHSIG.2const.html).
- `011f4f70f`: shares the bounded 64-attempt pre-exec gate reader and its tests
  with root spawn. Root capability-ceiling reads now use the existing fixed-buffer,
  finite profile observation instead of `read_to_string`. These are mechanical
  observations, not new admitted authorities or latency guarantees.
- `10dc028c6`: adds same-family native V2/V3 preparation/revalidation, actual
  policy/supervisor borrowing, native deployment/provisioning/key/lease ownership,
  freshly sealed helper/daemon images and pinned state-root custody. No native
  launch method or V1 owner conversion is supplied.
- `8cabc5ace`: updates the existing workspace dependency lock entry and corrects
  the image mutation test to respect F_SEAL_EXEC while testing mutable mode drift.

The native worker implemented preparation in a private worktree. The primary
reviewed/integrated it, implemented the cleanup/child-profile changes, and ran all
builds/tests. The worker ran no Cargo, SSH or network commands. Native preparation
uses one closed family implementation, with no public test provider interface.

See the [preparation contract](../compiler-execution-native-anchor-state.md#native-root-preparation)
for exact input/growth/retained accounting. Cleanup remains finite per turn;
pending and quarantined records keep capacity, descriptors and unverified spawn
leases. Only exact consuming terminal waits retire custody. Recovery preserves
limits and history. Native issuer launch consumes this shared pool; root-service
launch still uses its legacy blocking child owner.

## Validation

Pinned `nightly-2026-04-03`, frozen/offline dependencies, HIP disabled, one bounded
Cargo job at a time, no incremental compilation and no source edits during builds.
Evidence directory:
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`.

| Check | Result |
| --- | --- |
| Shared cleanup focused R1 | 44 shared cleanup tests passed; supervisor subset had a socket failure |
| Shared cleanup full R1 | Supervisor 192 passed, 36 failed, 42 existing ignored; spawn 47 passed |
| Parent-death follow-up | Spawn 49 passed |
| Bounded-reader full R2 | Supervisor 189 passed, 36 failed, 42 existing ignored; profile 51 passed; spawn 52 passed |
| Preparation full R1 | 26 passed, 3 failed, including two incorrect image-mutation fixtures |
| Corrected preparation full R2 | 28 passed, 1 failed; all 26 native tests passed |
| Final four-thread, four-crate full R3 | 320 passed, 37 failed, 42 existing ignored; no filtering; exit 101 |
| Four-crate doctests | 8 positive and 160 compile-fail passed; exit 0 |
| Fifteen-package all-target check | Passed with warnings; exit 0 |
| Changed Rust formatting and whitespace | Passed |
| Code hygiene delta through tested code | Passed |
| DCO through tested code | 118 signed-off commits, no inherited exceptions |

R3 consists of supervisor 189/36/42 (pass/fail/ignored), coordinator 28/1/0,
profile 51/0/0 and spawn 52/0/0. Earlier runs overlap this coverage. Three gate
tests moved from supervisor to spawn; they were not removed. No failing test was
ignored and no production admission check was relaxed.

The preparation fixtures exercise both nominal families, every policy and full
supervisor-context axis, provisioning/key mismatches, root identity/metadata/PID
drift, image source and retained metadata checks, exclusive-lock lifetime,
consumed-input closure, exact and short resource limits, arithmetic overflow,
original-ledger identity, historical denials and unwind. Private rootless hooks
omit exact-root admission only for these fixtures. The public nonroot entrypoint
rejects. These are not successful protected root deployments.

The parent-death regression runs actual prctl operations in an isolated bounded
subprocess. A private callback models the kernel's credential-transition clearing
of PDEATHSIG; the test verifies rearming, wrong-parent refusal and failed-transition
staging. It does not perform a root setuid transition or prove confinement.

The first preparation run attempted chmod 0500 on an execute-sealed image.
F_SEAL_EXEC correctly rejected it. The final test asserts this rejection and then
changes only the owner-write bit to 0755; native revalidation rejects that mutable
metadata drift. See [file seals](https://man7.org/linux/man-pages/man2/F_ADD_SEALS.2const.html).

Earlier supervisor runs included overlong UNIX-socket paths under the cache's
TMPDIR. R3 uses a short private `/tmp/fe272.*` directory, eliminating that failure
point; those tests then reach bind and fail EPERM. Remaining failures are socket
domain/bind/send/peer-credential restrictions and an ACL fixture installation
returning EINVAL. The coordinator readiness test fails its peer-credential query.
The supervisor parent-death test's helper fails earlier at socket admission.
No clean-environment baseline or successful root/GPU run is claimed.

The integration check includes supervisor, issuer, client, compiler coordinator,
closure capability, anchor coordinator/service/provisioner, host, cargo-fe2o3,
backend, lifecycle, protected executable, profile and spawn. Cargo target kinds
are not GPU architectures. Prior checkpoint static ELFs are retained as historical
evidence, not rebuilt-current-source or protected-runtime proof.

## Evidence Digests

SHA-256 of selected logs in the evidence directory:

```text
e1c7dd3638b689191e24859dbd0c43f011db07d8157ae952d9e7ed284d8fbcd3  conditional-native-root-cleanup-full-r2.log
a78644f22452b6c4342ca97e1aab2eebb1894618bff313b3c1d3848a8a80d6ef  conditional-native-root-prepare-full-r1.log
76bbf0357805a26370b92a31f4001e50d72260d6bbadc5fb0f6267dfe762f653  conditional-native-root-prepare-full-r2.log
8431aaad22f8879e9f357f0d97ed4705725931a9941a39033a3eb9fb7967d516  conditional-native-root-combined-full-r3.log
f12d092964475cd0ea2f029bba5e69795566806217de6c38e7b4c17d9af7d7d2  conditional-native-root-prepare-docs.log
3a2f1579367edbe2aa097e099f7d8e4429708af9f4eb14a9700539b822091b73  conditional-native-root-prepare-all-targets.log
```

## Remaining Gates

1. Implement native staged transfer validation and bounded root child custody.
   Reserve the funded cleanup slot and artifact-spawn lease before clone; adopt
   both with the atomic pidfd before fallible parent work. Keep leases through
   verified exec or exact terminal disposal, including errors/unwinds. Prepay the
   bounded post-clone child on the original parent ledger, not a forked copy.
2. Add native root launch/readiness/managed lifetime using actual contexts and
   the shared protocol engine. Require exact readiness endpoint, exec EOF, live
   pidfd, namespace/profile checks and protected endpoint admission. Pending or
   quarantined cleanup must not become successful termination or admission.
3. Integrate compiler admission/preparation, postchecks, finish/revalidation,
   V5 publication and SubjectV3 transport through the single production path.
   Backend receipt admission remains V1; conditional finalization still refuses.
4. Validate genuine protected startup/recovery, source/machine/numerical proof,
   generic non-AMD behavior and all 47 tutorial kernels end to end. Unit tests and
   compile-fail API contracts do not substitute for these milestones.

Fresh SSH attempts to all three configured GPU aliases failed DNS in this session.
No remote job or scratch directory was created. Local Cargo release-cache cleanup
removed 101 MiB; source worktrees and saved ELF/report evidence were preserved.
The short R3 temporary directory was empty and removed after testing. Publication
to both remotes and the issue is a separate operation, not evidence of validation.
