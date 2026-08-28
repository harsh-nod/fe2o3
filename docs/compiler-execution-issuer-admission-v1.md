# Compiler Execution Issuer Admission V1

## Status

This checkpoint implements the non-signing admission boundary for the
protected compiler-execution issuer tracked by
[issue #218](https://github.com/harsh-nod/fe2o3/issues/218). The implementation
is in `fe2o3-broker-authority-service`, which owns the single protected
authority-service process used by broker and compiler-attestation functions.

`ProtectedCompilerExecutionIssuerAdmissionV1` is move-only and deliberately
has no public or crate-private signing method at this checkpoint. Its public
authority marker is `admission-only`. It cannot construct a challenge, sign a
receipt, authenticate a compiler occurrence, publish an artifact, load an
object, or authorize a GPU launch.

## Process Boundary

`ProtectedIssuerProcessV1::harden` performs and immediately revalidates three
irreversible Linux process transitions before key admission:

1. set both `RLIMIT_CORE` limits to zero;
2. set `PR_SET_DUMPABLE` to zero; and
3. set `PR_SET_NO_NEW_PRIVS` to one.

Every continuity check re-reads all three kernel states. A transition or query
failure rejects admission. This prevents ordinary same-UID `/proc/<pid>/mem`
inspection and core-dump disclosure; it is not a hardware enclave, kernel
attestation, or defense against a privileged host administrator.

The shared `ProtectedServiceAdmissionV1` simultaneously retains and
revalidates the service-owned mode-`0700` private directory, connected unnamed
Unix `SOCK_SEQPACKET` peer, exact `SO_PEERCRED`, client pidfd, process start
time, descriptor identities, UID separation, and `FD_CLOEXEC` state. The
issuer does not introduce another IPC path.

## Executable And Runtime Closure

Admission opens `/proc/self/exe`, retains that exact file description, and
requires a bounded executable regular file. It reads the complete image,
checks the object metadata before and after the read, and applies the
loader-independent x86-64 static executable/static-PIE validator. `PT_INTERP`,
runtime dependencies, unsupported dynamic tags or relocations, malformed
segments, and writable/executable mappings reject.

The caller policy executable measurement is raw SHA-256 plus exact byte
length. Admission also retains the domain-separated sealed-static ELF
identity. Every later continuity check revalidates `FD_CLOEXEC`, object
metadata, complete bytes, static profile, raw measurement, and sealed-static
identity through the retained file description.

The production image enters through `fe2o3_secure_start_v1`, a freestanding
x86-64 shim that performs no allocation or runtime call. It restores
nondumpability after the kernel's `exec` transition, reasserts `no_new_privs`
and a zero `RLIMIT_CORE`, verifies the two process controls, and then jumps to
the static runtime entry while preserving its initial stack state. The static
build gate requires the ELF entry address to equal that symbol. Supervisor-side
authorization of the same sealed image remains required before key transfer;
self-measurement after entry cannot authorize a substituted executable. The
dedicated supervisor now implements the preceding authority-free program
stage: trusted provisioning supplies the launcher measurement, the sealed
caller policy supplies the issuer measurement, and both exact images are
copied to distinct read-only anonymous executable memfds with complete content,
exec, and seal seals. The program value cannot accept a root or key; that
authority-binding transition is now complete, as is authenticated receipt of
the direct-parent rustc handoff. The supervisor also materializes the exact
ten-source table, distinct standard-stream and readiness pipes, cloned
authority descriptors, and a sealed canonical static pre-exec manifest bound
to its PID and process start time. The actual `clone3` issuer launch, complete
child-profile observation, readiness admission, cancellation, and reaping
remain pending.

A sealed-static issuer has no user-space DSO inventory. Its runtime policy
therefore uses SHA-256 and length of the fixed canonical
`SEALED_STATIC_ISSUER_RUNTIME_CLOSURE_V1` record. Any other runtime measurement
rejects.

## Signing-Key Custody

The supervisor supplies one descriptor, never an argument, environment value,
path, or protocol byte string. Admission requires:

- `FD_CLOEXEC`;
- an anonymous regular file with zero links;
- ownership by the protected service UID and exact mode `0400`;
- exact length 32 bytes;
- immutable `WRITE`, `GROW`, `SHRINK`, and `SEAL` memfd seals; and
- an Ed25519 public key exactly equal to the caller-pinned policy key.

The seed scratch buffer is zeroized immediately. The retained descriptor,
metadata, seals, bytes, in-memory key, and policy public key are compared again
at every continuity boundary. Neither the key bytes nor a raw key descriptor
is exposed by the API.

## Durable Issuer State

This admission is now consumed by one
[signed crash-safe state machine](compiler-execution-issuer-durable-v2.md). It
owns OS nonce generation, sequence and rollback state,
challenge-before-release durability, exact request/subject comparison,
receipt-before-release durability, crash replay, singleton exclusion, and
publication-bound idempotent acknowledgment. Journal V2 retains the complete
Worker publication ACK in every later state and rejects raw receipt-digest
acknowledgments. The signing entry points construct a fresh
`ProtectedCompilerExecutionOccurrenceV1` from this admission's own retained
service session. They accept no occurrence parameter, so descriptor-only,
foreign-admission, and caller-constructed subjects cannot reach signing.

The authority service now has an authority-free primitive that independently
binds an admitted live rustc pid/start identity to its exact procfs state,
sealed V3 invocation, measured rustc/backend images, and retained artifact
directory. The authority service now joins that observation to the exact
current production-slot V3 publication, reconstructs the subject under the
publication lock, and retains both custody values through issuer use. The
currentness guard keeps that lock through request comparison, signing, and
durable receipt commit. The issuer accepts a publication acknowledgment only
through a move-only committed-publication capability that callers cannot
construct from wire bytes. The
[protected Worker ledger](compiler-execution-worker-ledger-v1.md) now creates
that capability only after independently verifying, committing, and reacquiring
the exact request and sidecar. The
[bounded service](compiler-execution-service-v1.md) now exposes that composition
over the already admitted `SOCK_SEQPACKET` connection and makes the direct
transition methods private. The descriptor-only musl-static issuer entrypoint
now admits the exact policy, launch manifest, client pidfd, peer, root, key, and
readiness pipe. It emits readiness only after complete admission and durable
recovery. The remaining issuer deployment work is the production distinct-UID
supervisor launch and inspection policy that provisions those descriptors.
Until that complete chain lands, `CompilerExecutionProvenance` remains open.

## Qualification

The package tests cover the fixed runtime measurement, exact key admission,
wrong key and policy, missing seals, wrong permissions, wrong length,
`FD_CLOEXEC` removal, irreversible process hardening in a subprocess, and
move-only compile failures. Existing service tests continue to cover pidfd,
credential, socket, descriptor, and durable-directory hostility. These tests
qualify admission behavior only; they are not a production issuer or runtime
authority result.
