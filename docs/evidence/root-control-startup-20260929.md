# Root Control Startup: September 29, 2026

Tested source: `61180b3a9` (before this documentation-only update).
All issue #272 M0-M7 milestones remain incomplete. This checkpoint adds no
protected production compiler execution, proof execution, safe GPU launch, or
tutorial kernel qualification. The compiler fixture never resumes after its
confirmed first exec stop.

## Correction

The previous MI350 startup attempt failed after readiness with
`root issuer custody changed or exited`. Temporary root-side NOWAIT inspection
confirmed issuer exit code 1 after challenge send. A diagnostic issuer build
narrowed the refusal to the root endpoint's address-shape check. A separate
socketpair reproducer confirmed Linux's first-send PASSCRED autobinding.

The production checks incorrectly required both socket names to remain unnamed.
Both now share a bounded predicate accepting an unnamed address or the exact
Linux abstract autobind form. That name is not authentication. Original socket
custody, flags/type, PASSCRED, creator and packet credentials, original compiler
association, measured issuer, and fresh challenge remain required. No timeout,
work ceiling, UID check, descriptor rule, or cleanup requirement was relaxed.
All temporary diagnostics were removed before the successful native run.

The same checkpoint removes a duplicated issuer-image validation, retains its
existing connection/trace validation and final liveness check, and updates the
exact resource quote. Packet tests accept only the requested MSG_CMSG_CLOEXEC
echo in addition to their existing exact payload checks; production packet
validation is unchanged. Replay completion has a private prepare/commit token
so failure of an enclosing accounting scope cannot install a reply.

## Executed Validation

Pinned toolchain: nightly-2026-04-03. Local Cargo runs were locked, offline,
single-job, serial-test, and GPU-disabled. Each guard checked every Git-visible
source input and tool identity before and after execution. The 9,340-file source
snapshot was unchanged:
`4889857aecf7660612dfda02ca8b4cdc92f2b8371749027d1d6e14f5354622f0`.

| Suite | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| Broker library | 385 | 0 | 20 |
| Coordinator library | 250 | 0 | 4 |
| Issuer library | 31 | 0 | 2 |
| Protocol library | 110 | 0 | 0 |
| Protected-spawn library | 242 | 0 | 7 |

The broker run includes all 12 replay tests and two new address regressions:
negative address/family shapes and a real three-leg credentialed socketpair
roundtrip exercising both automatic binds. The original failing runs and the
initial unnamed-address conversion regression remain in the local evidence;
they are not counted as passing.

Fresh static issuer/helper/daemon build guards passed. All three images passed
the static ET_EXEC, secure-entrypoint, no-interpreter/dynamic-dependency and
non-executable-stack checks. Scope-local whitespace and hygiene-delta checks pass.

## Native Matrix

The actual ten-case ignored coordinator matrix passed on `mi350` in an isolated
root container using image
`sha256:fd5370f370708f6a02cec6d44818a4295609e5bc68aa42455e53f141168a9d5f`.
The container had a read-only root and input binds, no network or GPU, one CPU,
2 GiB memory, 32 PIDs, bounded private tmpfs paths, and a 600-second outer limit.
Only the required explicit Linux capabilities were added. No host service or
protected installation was provisioned or changed.

Exact successful cases: `ready-cancel`, `ready-unwind`, `ready-image-mismatch`,
`ready-issuer-exit`, `ready-compiler-cancel`, `same-uid`, `corrupt-state`,
`zero-timeout`, `short-work`, and `short-storage`. The driver independently
enumerated the tests, checked all ten completion markers in order, verified
input hashes, and inspected terminal container state. The container, private
remote scratch and private SSH control directory were removed with no cleanup
errors. The dedicated privileged packet-fault suite is not implied by this run.

| Input | SHA-256 |
| --- | --- |
| Coordinator tests | `4a6e2cb3d5f9589034254421c586982abcdd81f780a1cc5f37315d110304b17f` |
| Static compiler fixture | `5cd9ee2d9194b8302fc39d9ed940bd5a2da3166a130596a8e957e5acb2934af5` |
| Static conditional issuer | `5792cef7c8f701854ca961676503557ec3d19e96815a652163e02fd88969dd43` |
| Static anchor helper | `a00ce7020aa68e1bc5cd856dcf9c0f2273ea49b85b407a2e4bd6e8561848d276` |
| Static anchor daemon | `6ac69f5be4a3de434de0ef2a782644d499c884917e71f9cbdcf8d686209121a0` |

## Evidence And Limits

Local evidence directory:
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921`.

| Report | SHA-256 |
| --- | --- |
| `root-autobind-broker-r6.log` | `fe1088f9d53c05be3f9043ac299cbf673c824f46e59fd74793c6b2256d1eff35` |
| `root-autobind-launch-r6.log` | `b7fc938fa3e672a1e551f5ee42b18b048d6cf401ae1d20966fe923aae069493d` |
| `root-handshake-autobind-r6/outcome.json` | `f7e666d2b8c5a3e08275d7f9a5f1af1d300243a6ed49007b8cc979b5bedfd5fb` |
| `root-handshake-autobind-r6/status.json` | `90e43fa2178f2f9c858e914a993987fd9787bf20b8079f4eb23dc9dd148d7b62` |

The successful test uses real static issuer and anchor services but a mechanical
held compiler fixture. It does not acquire a publication occurrence. The
independent production attempt, cleanup-slot retention of actual publication
locks, authenticated Observe/Validate/Retire, durable retirement/restart,
protected proof execution, generated safe-host activation, and complete
positive/negative simulator and target-matched GPU matrices remain required.
The site was not deployed by this checkpoint.
