# Application Bootstrap Qualification

Source: `6b89ecad8bc61383fdcbe63fd46a55a4bc3aa4db`.

This completes the application-side registration transport prerequisite on the
[multi-GPU critical path](../../runtime-multi-gpu-critical-path.md). Production
Cargo/supervisor/host activation remains pending. This is not an ordinary admitted
two-GPU application, remote proof custody, or GPU execution authority.

## Implementation

The consuming `register_pre_ack` API performs Hello/Challenge/Accept/Ready over the
original inherited proof endpoint. It binds the exact four-input capsule, app and
Cargo identities, original endpoint coordinates/object, both fresh session nonces,
root SCM credentials and the received original root pidfd. Ready requires the same
sender and transcript with no descriptors. One absolute deadline covers all phases;
failure closes the endpoint rather than returning a retryable partial owner.

The resulting move-only owner retains the endpoint and poll-only root-process
observation. It has no descriptor export, cloning or conversion to proof/execution
authority. Root PID/start time/object/probe-source continuity is rechecked before
and after protocol operations. Successful checks are point-in-time observations,
not a guarantee against subsequent root exit.

The existing strict PIDFD_GET_INFO/procfs and start-time parsers now live once in
`fe2o3-process-identity`, shared by broker and client. Broker error classifications,
original syscall sources, waitid supplement and EINTR retries are preserved.
Application probes fail closed on interrupted poll and never call pidfd_open,
waitid, signaling, shutdown or socket creation. Cargo's allowlist is unchanged;
its regression test now explicitly covers these required and forbidden calls.

## Qualification

| Check | Result |
| --- | --- |
| Process identity unit tests | 23 passed |
| Runtime protocol unit tests | 42 passed |
| Broker unit tests | 200 passed; 17 default ignored |
| Client unit, binary and integration tests | 55 passed; 2 default ignored |
| Supervisor unit and integration tests | 61 passed; 4 default ignored |
| Cargo binaries | 444 passed; 5 default ignored |
| Protocol, broker, client, supervisor and process identity doc tests | 100 passed |
| Isolated root campaigns | 5 passed |
| Six-package all-target Clippy | Passed with `--no-deps -- -D warnings` |
| Scoped formatting, frozen hashes and signed-source audit | Passed |

Total: 930 passed, excluding nested helper reruns. The 28 default-ignored tests
include helpers and the five separately enabled root campaigns; not every ignored
deployment test was enabled. Ten parser tests moved from broker into process
identity, rather than being removed.

The new root/UID1000 transport campaign covers 22 cases: both direct and separate
live root senders; wrong root pidfd, ordinary/missing/extra descriptors; changed
inputs, nonce, coordinate and object; malformed/oversized/wrong-phase packets;
wrong Ready transcript, rights and sender; duplicate Challenge, EOF and timeout;
and valid queued Challenge/Ready from an already-reaped original root.

For queued-death controls, the app is stopped before publication, the sender exits
and is reaped, and only then does the controller resume the app. A retained peer
alias excludes EOF as the explanation. Both require Process(AlreadyDead). The
Ready case establishes rejection of queued bytes after root death, not isolation
of the post-dequeue revalidation branch.

Process-owner tests cover exact identity, complete metadata/probe/start-time
mutations, descriptor-table substitution, absent CLOEXEC, consuming failure cleanup,
and exit detection without reaping. Separate subprocess tests positively verify
that forbidden process/socket operations return EPERM before admission succeeds.

## Limits and Next Steps

Root campaigns use private PID/IPC/UTS/network namespaces, a read-only host
filesystem and private tmpfs. Application helpers install a focused denylist after
libtest startup. This proves independence from the denied syscalls, not operation
under Cargo's full inherited pre-exec application allowlist or a measured deployment.
Input capsules use a synthetic ELF identity and synthetic non-proof slots; they
qualify transport, not original executable/descriptor observation or host ACK.
The four existing root registry/observation campaigns also pass separately.

Next seal compiler registration and its gate into one application owner, carry it
through issuer launch/readiness, and require gate completion before dedicated
application readiness reaches Cargo. Activate the exact four-right Cargo transfer,
atomic supervisor dispatch and host pre-ACK handshake together. Preserve the original
startup deadline and recheck publication currentness after Ready, before ACK.
Qualify this complete path using the actual static no-fork application profile.

Then implement the independently authenticated keyless retained-proof custodian
and consuming native invocation join, followed by genuine fill and guarded
bidirectional XGMI on two selected MI300X devices. No new formal machine proof,
hardware result, HIP/HSA parity or performance improvement is claimed here.

## Evidence

`evidence.tar.gz` includes reproducible commands/logs, source patches, 5,094 frozen
source/config hashes, 20 test-binary hashes, signed-source audit and an internal
manifest. Archive extraction and every manifest entry were verified. Temporary
evidence staging is removed; the normal target cache is retained. No MI300X
process, allocation or temporary file was created.

Source patch SHA256:
`f0aabf6a66dfe8091e8618cee21b24ca8fe5262b3ddad99638f7803c24185d44`.

Archive SHA256:
`3bfec196b9172ea657d40502e89baffc8e25a173a1ac3d81c465d83608732c1b`.
