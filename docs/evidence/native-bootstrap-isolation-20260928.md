# Native Bootstrap and Isolation Checkpoint

Date: 2026-09-28 (Pacific). Continues the
[namespace checkpoint](native-user-namespace-20260928.md).

**M0-M7 and the 47/47 production-to-safe-GPU-launch gate remain incomplete.**
No tutorial kernel changes classification in this checkpoint. The new Linux
diagnostic establishes five paired memory-access observations, not a production
proof-isolation capability, a semantic receipt, or GPU execution.

## Implemented

- The retained compiler runtime can transfer only its uniquely approved
  `ProofExecutorHelper` file. Complete approval/inventory/origin revalidation
  surrounds duplication and final validation. The original owner remains live;
  the transfer carries a separate full file-plus-image charge on the original
  account. This API is not yet consumed by an actual proof-helper launch.
- Native V2/V3 supervisor and anchor readiness compare actual message
  `SCM_CREDENTIALS` against the retained child's PID and admitted UID/GID.
  `SO_PASSCRED` is enabled before launch and checked at receive. Socket-creator
  credentials alone are not treated as the current writer's identity.
- The bounded ancillary parser accepts credentials plus exactly the expected
  descriptor shape. All disclosed descriptors, including rejected `SCM_PIDFD`,
  enter ownership before fallible validation. Truncation, unexpected rights,
  missing/wrong credentials and malformed records refuse and close transfers.
- The supervisor's CLOEXEC exec-status channel is independent of bootstrap.
  Exec status is consumed before readiness so startup failure stages cannot be
  masked by bootstrap EOF. The anchor retains readiness followed by bootstrap
  EOF because its helper performs a second exec into the final daemon. Legacy
  transport callers keep their existing API.
- A non-Send/non-Sync creator scope owns the original cleanup pool inside the
  unsafe dedicated native coordinator entry. Return or unwind before successful
  empty shutdown exits that dedicated process with status 125. Shared-process
  launch behavior is unchanged. Actual external whole-cgroup cleanup custody
  remains a deployment obligation; this scope does not admit the service unit
  or prove eventual descendant termination.

Request/cleanup quotas cover the new channels, credential observation, bounded
control parsing and descriptor disposal. Comparison values, transferred files,
and the creator scope grant no independent compiler or launch authority.

## Local Validation

Code commit: `822655fdd57fae5d9cb33a5d35e02307be279df5`.
All final guarded runs used the same 9,208-file snapshot:

```text
57d1af952d17dc8bc1c4fb43686f4bf3680904290279a968352735ac120ff7a6
```

Pinned `nightly-2026-04-03`, locked/offline dependencies, one build job, serial
tests, fixed resource limits, and before/after source/tool hashes were retained.
Existing warnings remain; these are not warning-free or workspace-wide results.

| Check | Result | Evidence Label |
| --- | --- | --- |
| Closure capability / compiler coordinator / anchor / spawn | 253 / 184 / 78 / 169 passed; 2 root diagnostics ignored | `bootstrap-all-lib-rfour` |
| Same four crates' doctests | 137 / 78 / 66 / 33 passed | `bootstrap-doc-rone` |
| Static musl spawn tests, executed locally | 169 passed; 2 root diagnostics ignored | `bootstrap-spawn-musl-rone` |
| Unsafe source inventory | 5 passed; maintenance command ignored | `bootstrap-unsafe-policy-rtwo` |
| Build authority, closure capability, verifier, cargo driver, codegen | All-target checks passed | `bootstrap-integration-check-rone` |

The GNU unit and documentation counts are 684 and 314 respectively. Musl repeats
the spawn tests under that ABI; it is not 169 additional unique test definitions.
Nested test subprocess summaries are not counted twice. Scoped formatting,
whitespace and delta hygiene against `0a400115a` passed without a waiver.

Tests include an actual subprocess writer distinct from the socket creator,
wrong/missing credentials, exact/truncated records, excess descriptor disposal,
independent channels, creator-scope unwind/busy/exhausted shutdown, original
account history, and eleven private inventory transfer tests. Synthetic inventory
fixtures do not fabricate a public approved runtime or prove FS_IMMUTABLE admission.

Earlier failed iterations remain recorded: a test incorrectly compared borrowed
ledger addresses across an owned-account move; the legacy anchor caller needed
its original wrapper; and a new channel test expected a scalar instead of the
pinned rustix receive tuple. Each was corrected before the final runs. Review
also corrected the supervisor exec-status/readiness ordering described above.

## Actual MI350-2 Diagnostic

A separately reviewed static C diagnostic ran once on `mi350-2`. For each endpoint,
a fresh target creates its writable marker and memfd only after fork and final
credential installation. The actual ancestor then drops to the same host UID/GID
9661, with zero groups/capabilities. Both sides are dumpable, with no-new-privileges
set. Yama scope 1 and the existing LSM profile remain unchanged.

Each control must acquire access, change bytes, and obtain an independent live
target acknowledgement. The isolated case uses a fresh root-owned user namespace,
with actual namespace-parent, owner, maps, pidfd and cgroup custody checked before
release. PID/time namespaces remain unchanged. A blocked control never counts.

| Endpoint | Same-Namespace Control | Isolated Case |
| --- | --- | --- |
| `ptrace` | SEIZE, interrupt, peek/poke and detach; changed bytes acknowledged | EPERM; bytes unchanged |
| `process_vm_writev` | 8-byte write acknowledged | EPERM; bytes unchanged |
| `/proc/PID/mem` | 8-byte write acknowledged | EACCES; bytes unchanged |
| `/proc/PID/fd/N` | Same backing object, 8-byte write acknowledged | EACCES; bytes unchanged |
| `pidfd_getfd` | Same backing object, 8-byte write acknowledged | EPERM; bytes unchanged |

Result: **5/5 paired diagnostics, 10/10 cases**, SSH exit 0, no quarantine.
Every target exits normally after its acknowledgement; each actor and the compiler
are reaped. All eleven fresh cgroups and the root-private scratch directory were
removed. A separate SSH check at `2026-09-29T01:13:22.050Z` confirmed absence of
all 25 paths: this run's 12 objects plus 13 previously cleaned paths. Shared
runtime installations, source worktrees and host security settings were unchanged.

These are standalone Linux mechanism tests, not executions of the production
namespace owner or a full threat-model qualification. Distinct mapped-peer
coverage, complete backing exclusion, deployed custodian admission, and the
helper/proof-child integration remain unqualified. In particular, these results
do not authorize enabling dumpability in the production proof-child path.

## Evidence Identities

The retained local archive `paired-memory-rone-evidence.tar.gz` is 35,929 bytes
with ten members: frozen diagnostic/driver/invoker/support sources, request,
status, controller JSONL, SSH stderr, independent absence report and its verifier.
The temporary executable itself was removed, not installed or retained remotely.

| Artifact | SHA-256 |
| --- | --- |
| Diagnostic source | `0d2a0cd26fb54cc0c7f961685019df620c59166718b91a904046c8136789b6e2` |
| Root driver | `e0d4cc4a2b6278655cf53c1f0792c57f6da571607889b33eb5fa0da862f89187` |
| Invoker | `ce2068fba6e347d5c4ec38cb2ff343686d959e8d7be1d043da71898d6876d7ab` |
| Static diagnostic, 1,067,360 bytes | `8555762aeda902cb5e1ac42a91745e6d526758b08b75d4a42823241e7a277c78` |
| Controller JSONL | `227e408d1c823268bab3ced4af3c66781723f01b00c8d49aec3a07057ea2b833` |
| Independent absence report | `95920996f93ddf8f7dbc6ce6b710004d48531d1a30f57c39f564048caf7a1fd9` |
| Local archive | `339852ad8885fea881c3892c862726b0b5f47d789500d0948d765e4df306f7e9` |

## Remaining Production Work

The next integrated boundary is the real approved helper/compiler attempt:
secure helper entry and genuine deployment-origin association, retained helper
backing, exact dynamic-rustc invocation staging, one traced-child/wait owner,
whole-lifetime writer exclusion, authenticated attempt commit, and a fresh
process-affine proof-runtime lease opened inside the helper after exec.

The native runtime-enforcement refusals remain in place and executor replies
remain inert. Actual proof execution, the owned compiler continuation, safe GPU
launch, the full 47-kernel positive/negative matrix, and release documentation
are still required. No milestone is closed by this infrastructure checkpoint.
