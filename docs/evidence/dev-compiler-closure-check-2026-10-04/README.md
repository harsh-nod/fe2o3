# Immutable Compiler Closure Validation

Source: `e10b93ca7001f5ca4fb52348d0f104db3ce58106`.
Parent: `95aa35708e68fbd25fdbc76c9aa00231f3ce349e`.
The ordinary two-GPU admission path now has an independent compiler-evidence
checker suitable for reuse by the external proof custodian. This checkpoint does
not implement that service, transfer original proof ownership or authorize a GPU
launch. A3 remains open.

## Implementation

`check_worker_v3_compiler_closure_v1` accepts exact canonical V2 envelope bytes,
finalized HSACO bytes and one kernel identity. It canonical-decodes the complete
envelope with existing budgets, preserves receipt-to-compiler-subject binding,
rejects an incorrect artifact length before expensive replay, and uses the same
immutable validation core as recovered descriptor and roster admission.

That core independently reconstructs the finalizer from the outer handoff,
ordered external providers and transcript. It matches finalized and reconstructed
raw identities, compiler ABI source and exports, target and code-object version.
Exact descriptor/physical-kernel selection and host-lineage hashing are shared
with ordinary admission. Existing public error variants and admitted check order
are unchanged.

The returned checked closure borrows both exact byte strings and owns its decoded
wire and reconstructed evidence. It performs no filesystem access, publication
recovery or lock acquisition. It cannot produce the runtime's original recovery
evidence view, a publication token, an owned proof, or an executable.

Single-kernel audit requests now expose the original canonical V2 envelope borrow
while their existing current token pins the exact artifact. Live readiness,
publication/file occurrence, currentness and retained application-descriptor
checks remain with the original admission. The new public byte checker cannot
establish those properties for transferred copies.

`CheckedWorkerV3CompilerClosureV1::check_conditional_fill_refinement_v1` joins an
original executed refinement to that complete checked closure without a generated
marker or caller-supplied contract digest. It shares compiler-input, target-lineage,
machine and closed-contract checks with application admission. Deterministic
challenge hashing now takes checked descriptor coordinates; its encoding remains
identical to the previous marker-based form. The existing pending path still
checks the marker declaration before using the one-use compiler service.

Both paths produce the same inert subject for the same evidence. This is matching
data, not a session nonce or remote owner. The compiler-current-record service
continues generating its separate fresh challenge. The caller must retain the
original proof and authenticate application occurrence, session, compiler
deployment and publication currentness independently.

## Qualification

| Check | Result |
| --- | --- |
| Host library | 221 passed, 3 ignored |
| Host doctests | 13 positive and 38 compile-fail passed |
| Selected vertical regression | 51 passed, 5 ignored, 16 filtered |
| V2 envelope regression | 31 passed, 3 ignored |
| Host and vertical strict Clippy | Passed with `-D warnings` |
| Frozen-source native/protected selection | 3 tests passed, including all seven pending cases; 294.89 seconds |
| Changed-file rustfmt and whitespace | Passed |

Four new vertical tests cover copied evidence after publication removal, truncated
and extended or corrupted binaries, unknown selection, V1 rejection, a foreign
compiler receipt with a correctly resealed V2 checksum, and distinct closure
identities for identical HSACO from different finalizers. Audit fixtures compare
independently reconstructed evidence with the admitted request while its original
token is held; pointer checks establish exact borrowed inputs and a separate
finalizer reconstruction. They independently recompute the old challenge encoding.

Protected tests execute fresh native Worker compilation/replay/finalization,
authenticated machine analysis and protected Verus refinement. The independent
subject is compared with the existing expected encoding and the successful
pending subject. Marker, contract, payload, stale-publication and service-failure
controls remain, including endpoint-unused and AlreadyConsumed assertions. A
changed non-executable `.comment` byte also rejects through the public closure
checker. Original proof/source/analysis owner-address checks remain intact.

The compiler-current service uses test keys, not a genuinely deployed issuer and
separate anchor. These are CPU/subprocess checks, not an ordinary no-fork
application, cross-process proof custodian, native invocation or two-GPU campaign.
No new formal theorem or HIP/HSA parity or performance claim follows.

Checks use four Cargo jobs, disabled HIP discovery/incremental/debug information,
test optimization 1, and enabled debug assertions and overflow checks. Doctests
use four threads; vertical/V2/protected tests use one. The selected vertical run
excludes `strict_v3_` and `cargo_supervisor_and_static_host_consumer`; a full
workspace/deployed-system pass is not claimed. Final source hashes are checked
before and after qualification.

Development evidence preserves one corrected test-fixture failure: two default
synthetic fixtures had identical deterministic compiler attempts, so their receipt
swap was valid. The negative now creates a distinct attempt and asserts unequal
subjects before substitution. Production validation was not weakened.

## Evidence

[Evidence archive](evidence.tar.gz) SHA-256:
`3b42eb00bf512371e998b3441bfce8d18cf6a92a21227bc9f5ccef026aa548ad`.
It contains the signed source commit, source patch/hash manifest, command scripts,
initial and final logs, fresh proof/analyzer/service captures, independently derived
subjects and cleanup checks. Its file manifest and archive comparison passed.
Source patch SHA-256:
`fa54b19ff389a6cc00a264af8ae6a1fff95b84c11b7868fca06cfc9ab1287953`.

The final native selection includes the genuine host audit, seven-case pending
campaign, and underlying finalizer/authenticated-model fixture. An earlier
seven-case campaign also passed before the additional public `.comment` rejection
assertion; the final run repeats it on the frozen source. `proof.key` files contain
public verifying keys, not signing keys. Binaries, packages and private runtime
trees are excluded.

The native Worker remains SHA-256
`fb020a09969938d7fa849e106a146e81668d9d0b0d7dd40ded0e2145e6836715`.
Protected runtime source/installation verification passed with manifest identity
`ffef09bd240c90e72cbff31a82bc5173c796ba7ab9af239245e7ad892c25641c`.
Runtime inputs were hash-checked. Both private namespaces exited, their test
TMPDIRs were empty, and the owned root library overlay was removed. No host
protected runtime installation or MI300X resources were left behind. Pinned build
input caches are retained for subsequent qualification.

## Remaining Work

Authenticate the original application before descriptor ACK over a dedicated
proof endpoint, preserve FD195 for compiler-currentness, deploy a separately
measured keyless proof custodian, and retain its original proof through both
devices' settlement. A root-side duplicate of the ACK pipe writer must close
before acknowledgment, or Cargo's EOF-based startup wait cannot complete.
Application exit alone does not establish GPU settlement; unknown failure paths
still require retained or quarantined custody.

Genuine deployed compiler/anchor qualification, private native invocation
admission and two-GPU fill/XGMI/guarded readback remain required. Reuse the existing
qualified multi-device routing and XGMI paths; broader performance campaigns
remain deferred.
