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

## Remaining Issuer Work

This admission must be consumed by one durable state machine before signing is
enabled. That state machine must own nonce generation, sequence and rollback
state, challenge-before-release durability, request/subject comparison,
receipt-before-release durability, exact receipt replay after crashes, and
idempotent acknowledgment. A separate supervised-occurrence protocol must let
the service independently reconstruct the exact compiler-execution subject.

Only after those pieces exist can a bounded `SOCK_SEQPACKET` service loop call
the existing canonical protocol signer. Worker V3 must then carry and verify
the receipt under its own current rollback ledger. Until that complete chain
lands, `CompilerExecutionProvenance` remains open.

## Qualification

The package tests cover the fixed runtime measurement, exact key admission,
wrong key and policy, missing seals, wrong permissions, wrong length,
`FD_CLOEXEC` removal, irreversible process hardening in a subprocess, and
move-only compile failures. Existing service tests continue to cover pidfd,
credential, socket, descriptor, and durable-directory hostility. These tests
qualify admission behavior only; they are not a production issuer or runtime
authority result.
