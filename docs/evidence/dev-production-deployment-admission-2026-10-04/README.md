# Production Deployment Admission

Parent: `8ebc94cb0f373a7027ff23d52eb20e1335665a70`.
This implements application-side independent deployment pinning for the conditional
multi-GPU admission path. It does not qualify genuine deployed compiler execution,
grant conditional launch authority, or provide a GPU or performance result.

## Implementation

`ProductionCompilerExecutionDeploymentV1::open` admits only the fixed root-owned
client, supervisor and anchor records under `/etc/fe2o3/compiler-execution`.
Caller-created sealed capabilities cannot acquire this provenance. The move-only
owner retains the original directory and file descriptors, their exact snapshots,
sealed canonical copies, current-thread user/mount namespace handles, procfs and
application credentials. It validates the client/supervisor UID/GID and anchor
credentials, exact issuer policy, supervisor identity and separately pinned anchor
key through the existing canonical protocol relations.

Original trusted-tree checks are reused, including ownership, read-only file mode,
single link, canonical length, descriptor flags and forbidden ACL/capability
attributes. Revalidation opens the ambient root and walks every path again before
and after reading exact files. Byte-identical replacements are not original
provenance. Unrelated directory timestamps do not invalidate unchanged records.
Namespace handles are for the current thread, not merely the process leader.
Full-identity UID/GID maps are read through constrained procfs `openat2` resolution;
remapping and map-file overmounts are rejected. Real/effective/saved/filesystem IDs
must agree, UID must be nonzero and distinct from both service UIDs, and effective,
permitted and inheritable capabilities must be empty. Supplementary GPU groups
remain permitted.

The concrete host auditor has an additional fixed-production factory. It consumes
FD195 before opening the deployment, uses the independently admitted policy instead
of the carriage policy, and consumes the whole session exactly once. The original
deployment owner moves with the successful response into the existing compiler
evidence lane and pending artifact. Both automatic fresh and caller-owned challenges
share this implementation. A post-exchange provenance check runs even when transport
or signature verification fails, before propagating that failure. Local subject or
policy rejection consumes the session without sending a request.

Delayed compiler-evidence binding revalidates retained deployment. The separate
fallible `revalidate_production_deployment()` accessor rejects absent provenance;
ordinary audits, canonical-byte imports and synthetic evidence cannot obtain it.
Existing publication-only currentness APIs and all verification/load/launch
authority flags remain unchanged. Revalidation does not perform another service
exchange and is not a continuous issuer-liveness lease. No signing protocol,
allocation path, routing abstraction or public caller-supplied trust hook is added.

## Qualification

Twenty-one isolated cases pass using the actual fixed production paths, root-owned
configuration and an unprivileged application inside disposable private namespaces:

- Deployment positive; mutable, wrong-owner, symlinked, hard-linked, truncated and
  canonically mismatched files; root caller; actual remapped user namespace; and a
  bind-mounted fake full-identity UID map.
- Concrete FD195 host positive with fresh and caller-owned challenges, valid
  alternate-policy carriage, pre-exchange replacement, disconnected service,
  replacement during successful and failed exchanges, delayed binding, retained
  evidence after auditor drop, wrong signer, and factory rejection/endpoint closure.

Every started host session is checked for `AlreadyConsumed`. Replacements use
identical file bytes. Wrong-signer evidence remains canonically valid and self-signed
under its substituted embedded key, then fails the independent pin. Both qualification
executables retain identical SHA-256 hashes before and after the complete campaign.

The service responses and public configuration deliberately use test keys. This
qualifies fixed-path trust admission, strict client verification and original owner
retention, not protected issuer execution or genuine compiler receipt acquisition.
The existing genuine conditional-fill Verus/native campaign from the parent checkpoint
is not repeated here. No new formal theorem for filesystem, process or host adapters
is claimed. The security scope trusts the local root, kernel and compatible procfs;
it does not resist malicious root, whole-host rollback or changes after a completed
point-in-time check. Live worker-thread namespace change and chroot transitions
remain additional qualification cases, not results of this campaign.

The initial remapped-namespace harness failed before admission because `newuidmap`
was absent; a direct mapping then exposed an unnecessary `setgroups` operation.
Using direct single-ID mapping without changing already mapped IDs fixed the harness.
An intermediate build caught a test calling a nonexistent authority accessor; the
assertion now uses the existing verification-authority API. A dependency update also
required regenerating the lockfile offline. No production guard was relaxed.
The broader host test-target Clippy check also found an existing `clone()` on a
Copy observation in a generated-argument test. Removing that redundant call is
the only unrelated one-line test cleanup; runtime behavior is unchanged.

The ordinary capability/host test campaigns run as UID 1000, with HIP discovery
disabled. Root rejection is exercised separately inside the private namespace.
The shared MI300X host is not used by this checkpoint.

## Regression Results

| Check | Result |
| --- | --- |
| Fixed-path deployment and concrete host audit | 21 isolated cases passed |
| Capability library | 61 passed, 2 fixture/namespace ignores |
| Capability ownership doctests | 14 passed |
| Host library | 212 passed, 3 ignores |
| Host with verifier test support | 213 passed, 3 ignores |
| Host ownership/API doctests | 12 positive and 32 compile-fail passed |
| Selected vertical integration | 46 passed, 5 default ignores, 15 filtered |
| V2 envelope regression | 31 passed, 3 default ignores |
| Capability all-target and host library/test Clippy | Passed with `-D warnings` |
| Targeted rustfmt and whitespace | Passed |

The vertical selection excludes `strict_v3_` and
`cargo_supervisor_and_static_host_consumer`. These are scoped regressions, not a
full workspace, live protected compiler deployment, or hardware campaign. Ignored
production-path tests are exercised explicitly in the isolated campaign above.

## Evidence

[`evidence.tar.gz`](evidence.tar.gz) contains the code patch, qualification scripts,
public test configuration, build/test logs, intermediate failures, final binary
hashes and permission-probe source/results. It contains no signing seeds or
compiled executables. The final fixed-path campaign is `qualified-4`; all 21
cases passed and its before/after executable hashes agree.

- Archive SHA-256: `e666662f75a14e373cbc5abc9d5238fbb190068b1e4e2200c2602e56c3fc37f6`
- Code patch SHA-256: `61a99f8c51c96ba2abd3cb811ed8d988f34e7e57a7c3812ba94e23fe81f6732c`

## Remaining Multi-GPU Work

First implement bounded cross-UID process observation. The existing
`ValidatedRemoteRustcProcessObservationV1::observe` reads the compiler's procfs
working directory and environment and duplicates its descriptors with
`pidfd_getfd`. The distinct-UID signing issuer's hardened profile empties all
capability sets; there is no delegated observer path. Simply adding a systemd
capability would conflict with that enforced profile. Keep the signing issuer
unprivileged and give it only authenticated, occurrence-bound observations from
a narrowly scoped root-coordinator-owned authority.

A local Linux permission witness used only two owned processes in a disposable
private namespace: a UID 1000 target and a UID 61001 observer with empty capability
sets. Root with `CAP_SYS_PTRACE` successfully duplicated the target descriptor;
the unprivileged observer received `EPERM` from `pidfd_getfd` and `EACCES` opening
the target's procfs working directory. Both processes were reaped. This is an OS
permission witness, not an actual deployed compiler acquisition attempt. No host
ptrace settings were changed. The first probe failed a harness assertion because
the namespace launcher preserved capabilities across the UID change; explicitly
clearing the observer's capabilities corrected the harness. Both logs are retained.

Then run genuine compiler receipt acquisition and fresh application verification through
the root-managed deployed issuer and separate anchor, retaining the deployment
coordinator in the root process. This must replace the test-key response fixture;
existing test-signed carriage is not production evidence. The current issuer already
implements checks for actual compiler occurrence, measured service continuity,
ledger custody and the external anchor, but their full deployed acquisition path
is not qualified. No new signing scheme is needed.
The selected-rustc path already publishes conditional singleton evidence and calls
actual compiler-execution acquisition before producing the receipt sidecar. Cargo's
Worker preflight uses the structural V3 handoff and shared finalizer engine; it
does not impose an unconditional proof-input gate here. Rerun that real compilation
path rather than importing the previous conditional handoff. The existing deployed
client-check executable only tests receipt-absent recovery/cancellation and cannot
qualify this acquisition.

Then bind exact prepared full64 geometry, patched DATA/kernarg ranges and selected
device in the existing KFD preparation transaction before private invocation
authority. Retain one admitted artifact across both GPU invocations. Finish the
admitted fill -> completion -> staging -> settled H2D -> PUBLIC XGMI -> guarded
readback campaign on two selected free GPUs in both directions. A3 and HIP/HSA
parity remain open.
