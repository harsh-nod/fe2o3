# Inert Conditional Fill Subject

Source: `3b169f88fc3d03f193bff5f01da3985796e14042`.
Parent: `687b1ad753d18345b80a2401420ba584709e689a`.
This is matching data for the future retained-proof custodian, not that custodian,
original occurrence authentication, currentness or load/launch authority.

## Implementation

`InertWorkerV3ConditionalFillSubjectV1` is constructed privately after the existing
program, machine and host-request association checks. Pending admission retains it
only after finalizer revalidation, current-publication checks, the one-use compiler
service audit and exact compiler-evidence binding all succeed. Validation order,
error variants and original evidence custody are unchanged.

The 221-byte encoding is a versioned domain followed by host lineage, deterministic
host request challenge, original refinement boundary, obligation identity, signed
receipt identity and verifying key. Each content identity encodes SHA-256 followed
by its little-endian u64 length. Encoding uses a fixed stack buffer; the immutable
public value caches its content identity and supports cloning, not promotion.
The original compiler/analyzer/refinement owners and publication token remain in
the non-Clone pending artifact.

These roots commit the canonical compiler/publication/finalizer/request association
and original proof inputs/source/recipes without duplicating their schemas. Equal
canonical evidence can have equal subjects. This does not identify a unique object
or process occurrence, authenticate protected compiler deployment, or transfer proof
custody. The host challenge is deterministic, not a fresh session nonce. In
particular, committing a marker's declared contract does not independently derive
or authenticate it.

## Qualification

| Check | Result |
| --- | --- |
| Host library | 214 passed, 3 ignored |
| Host doctests | 13 positive and 36 compile-fail passed |
| Fresh protected pending campaign | All six cases passed; 206.67 seconds |
| Selected vertical regression | 46 passed, 5 ignored, 16 filtered |
| V2 envelope regression | 31 passed, 3 ignored |
| Host and vertical strict Clippy | Passed with `-D warnings` |
| Changed-file rustfmt and whitespace | Passed |

Two new unit tests pin the domain/layout/endian encoding and independently mutate
all eight retained identity components. Five new doctests establish inert traits
and reject raw construction, mutation, executable conversion and proof-owner
reconstruction. The genuine Good fixture independently assembles the complete
encoding, checks its digest and original owner addresses, and captures `subject.bin`.
Its subject stays unchanged during publication replacement/restoration while
currentness still rejects/accepts appropriately. Marker, Payload, StaleSuccess,
StaleFailure and ServiceFailure retain their original rejection, endpoint-use and
one-use audit assertions. No additional proof execution was added to these cases.

The campaign executes fresh native Worker compilation/replay/finalization,
authenticated machine analysis and protected Verus refinement. It does not import
archived proof bytes. The compiler-current-record service is still test-key-backed,
and the fixture marker supplies test request identity, not generated ABI authority.
This is not a deployed production issuer or ordinary no-fork application campaign.
No new formal theorem, native invocation, GPU result or HIP/HSA parity is claimed.

The native Worker was reused after SHA-256 verification:
`fb020a09969938d7fa849e106a146e81668d9d0b0d7dd40ded0e2145e6836715`.
The protected runtime was provisioned in an isolated root namespace, followed by
unprivileged execution. Source and installed runtime manifest verification passed
with identity `ffef09bd240c90e72cbff31a82bc5173c796ba7ab9af239245e7ad892c25641c`.
The pinned rustup/libc/zlib downloads were hash-checked; an ignored input cache was
retained for subsequent qualification. No host `/opt` installation remains. The
owned root library overlay was removed and the test temporary directory was empty.
No MI300X resources were used.

CPU checks disable HIP discovery, use four Cargo build jobs, disable incremental
compilation/debug information and retain test optimization level 1, overflow checks
and debug assertions. Host library tests use default harness threading; doctests
use four threads, and the protected/vertical/V2 tests use one. The selected vertical
regression excludes `strict_v3_` and `cargo_supervisor_and_static_host_consumer`;
this is not a full workspace or protected deployment pass.

[Evidence archive](evidence.tar.gz) SHA-256:
`854fa81ef73a0a899c2b683ef5f336a21c08f46f86fc0ff56ae79d2223be0799`.
It contains commands, source patch/hash/signature records, build/test logs, fresh
proof/analyzer/service captures, the positive subject, runtime-input hashes and
cleanup checks. `proof.key` contains public verifying keys, not signing keys.
Worker binaries, downloaded packages and the private runtime tree are excluded.
Source patch SHA-256:
`1f01b085bfe928d87de61f7d203e603ab4b6a9171f4abb85d92b3b234791defb`.

## Next Multi-GPU Gate

Implement independent closed-fill contract derivation before accepting a subject
in a measured custodian. The fixed service cannot monomorphize an arbitrary
application marker or trust its unsafe `K::PROFILE` declaration. Validate the
original descriptor and semantic/KIR shape, reconstruct the restricted u32
disjoint-slice ABI and launch/resources with existing contract APIs, and compare
against a genuinely generated marker. The audit-only fixture is not that positive.

Then add independently measured proof custody outside the permanent no-fork
application profile, bound to the original application occurrence and a fresh
authenticated session. Keep FD195 exclusive to compiler-currentness auditing and
avoid reacquiring the application's held publication lock. Retain original proof
custody through both devices' settlement; canonical equality alone is insufficient.
Genuine deployed issuer/anchor qualification, the private join with native invocation
premises, and admitted two-GPU fill/copy/guarded readback remain required. A3 is open.
