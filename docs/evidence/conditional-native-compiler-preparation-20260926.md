# Native Compiler Preparation Checkpoint

Date: 2026-09-26. Continuation of the
[transfer checkpoint](conditional-native-transfer-20260926.md) for
[issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
M0-M7 and 47/47 production-to-safe-GPU-launch completion remain open.

Base: `d40cee32ddd0cc560abf93a6ae68570bac115823`.
Integrated code: `c666a8cd620703eeb3cf3da3c84fd41e78cb2fe3`.
Subsequent documentation changes do not change executable behavior.

## Implementation

- `3f8d44742`: native worker's V2/V3 supervisor trust custody, integrated from
  its private worktree. Actual deployment, policy and key owners are revalidated
  and joined by complete policy identity, including the key generation.
- `6d5cf2f52`: worker's metered lifecycle-to-root binding, following its review
  finding that independently valid leases were insufficient for preparation.
- `c666a8cd6`: primary's shared V2/V3 compiler preparation, service-input join,
  tests, and correction to an existing transfer cleanup fixture.

`PreparedCompilerExecutionSupervisorV2/V3` consume genuine same-family trust,
native listener/root inputs, two native lifecycle leases and a genuine managed
anchor. Three fresh supervisor/launcher/issuer images are sealed against the
actual deployment and policy measurements for the dedicated service owner.
Preparation checks exact root credentials and pins the preparing PID and
namespaces. Revalidation repeats the context, input, image and process checks.
The root provisioning lease is the last retained field to close.

Both families share one closed implementation. No V1 admitted authority is
upgraded, no public provider or signing operation is added, and no fake managed
anchor is introduced for tests. The existing V1 source bundle is reused only as
three untrusted Files. **Preparation creates no process.** Native inherited
composition, supervisor startup and consuming launch remain separate unfinished
integration steps; the production entrypoint still uses V1.

Storage queries include every consumed full owner and all three complete source
images. Quotas include cumulative image growth, the local frame and nested native
checks on the original account. Callers reserve returned growth before retaining
the result and retire full retained charges only after Drop. Refusal restores
entry storage without refunding work or clearing peak/denial history. These are
logical quotas, not generated-stack, elapsed-time, allocator or RSS bounds.

`CompilerExecutionServiceLifecycleLeaseV2::revalidate_for_root` derives the actual
root's parent and checks its policy, pinned identity and canonical lock sibling
alongside retained lease continuity. Its work charge is 65544 and scratch is
`IO_STORAGE + FILE_STORAGE`. It requires the complete lease and borrowed-root
floor and closes its temporary parent on success, refusal and unwind.
`ProvisionedProtectedIssuerServiceInputsV2::validate_lifecycle` surrounds this
join with exact input continuity without exposing the root descriptor. The
coordinator uses it for both leases and accounts for both operations. These
checks are point-in-time observations, not immunity from future privileged moves.

## Validation

Pinned `nightly-2026-04-03`, frozen/offline dependencies, HIP disabled, one bounded
Cargo command at a time, source frozen during builds. The full unit suite used
four test threads and no filters. The native worker ran no Cargo, SSH or network
commands; the primary performed all execution and integration.

| Check | Result |
| --- | --- |
| Four-crate unit suite, R2 | 369 passed, 50 failed, 43 existing ignored; exit 101 |
| New trust and image/accounting tests | 28 passed, included in R2 |
| New lifecycle binding tests | 11 passed, included in R2 |
| New service-input join tests | 2 failed during socket fixture binding with EPERM |
| Four-crate doctests | 27 positive and 240 compile-fail passed; exit 0 |
| Fifteen-package all-target check | Passed with existing warnings; exit 0 |

R2 consists of compiler coordinator 52/7/0, lifecycle 42/0/1, supervisor
194/42/42 and anchor coordinator 81/1/0 (pass/fail/ignored). Nested subprocess
reports are not counted again. The suite is not green: remaining failures are
socket bind/domain/send/peer-credential EPERM or ACL fixture EINVAL. No test was
disabled and no production predicate was relaxed. Rust all-targets checks do
not establish GPU architecture coverage.

Tests exercise genuine trust capabilities, full policy substitution, sealed
image roles/owners/metadata, exact/short resource accounts, prior denial history,
descriptor closure and unwind. Root binding tests cover unrelated valid leases,
root moves, current parent and lock replacement, and retained lock behavior.
Socket-dependent composed join tests still need a capable environment. Typed
preparation examples compile; a successful protected native preparation or
deployment is not established by these local fixtures.

Earlier image test assumptions were corrected: executable seals reject removal
of execute permission, and restoring mode does not restore the pinned ctime.
The first four-crate run also exposed a cleanup-fixture race in an existing anchor
transfer test. Retaining `TempPath` links through the assertions prevents inode
reuse after descriptor closure; the exact zero-live-descriptor assertion remains
unchanged. R2 passed that test, and the worker found no lifetime hole in review.

Logs are retained under
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
4758e735ccc819e12a50805f89591478b548991f3fb99641b34533d13168a964  conditional-native-compiler-preparation-full-r2-20260926.log
9dfc238060fb2cda10f77f1f2c9cba9720417d0e12f4af563bb0b6c4e2421048  conditional-native-compiler-preparation-docs-r1-20260926.log
de91ced235e7a9537ec628326e8bcc84b0151bf843a5c842ce2e36dbb64db883  conditional-native-compiler-preparation-all-targets-20260926.log
```

## Remaining Gates

1. Implement native supervisor inherited startup over the existing eleven-role
   descriptor ABI, actual policy/deployment/profile/image/lease/key checks,
   native program and anchor admission, listener activation and finite readiness.
   Retain deployment, image, profile and lease through native dispatch, using
   separately funded cleanup. Wire the production binary and static build gate.
2. Compose native root inherited inputs and consuming supervisor launch through
   the shared spawn primitive, with final staged-object checks, gate release,
   exact readiness/EOF/liveness and managed continuity. Do not add a second
   credential, profile, parser or reaping implementation.
3. Validate genuine protected startup, supervisor transfer, recovery and composed
   post-clone/error/unwind cases, including child-storage refusal and failure
   after endpoint receipt. Lower-level packaging tests are not substitutes.
4. Complete compiler admission/postchecks/finish/revalidation, V5 publication,
   SubjectV3 transport, native backend receipt admission and conditional
   finalization in the single production compiler path.
5. Validate source/MIR/KIR/machine/numerical proof, generic non-AMD behavior and
   all 47 tutorial kernels on gfx942/gfx950. No new protected proof or GPU run
   is credited here.

Fresh SSH attempts to mi350, mi350-2 and mi300x failed DNS in this session. No
remote job or scratch was created. GitHub access also failed; publication must
be checked independently of this local implementation and validation checkpoint.
