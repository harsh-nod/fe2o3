# Distributed Receipt Origin Ingress V1

Status: CPU-tested component on signed base `cebd2b3667fe827ef6cafc2b83805111c8eaf2ed`.
No formal-proof, hardware or two-host acceptance is claimed.
This is a narrow A4 production API, not completion of A4 or HIP/HSA parity.

## Boundary

`DistributedReceiptIngressV1` holds one immutable provisioned peer key, one
OS-generated challenge and one existing `ModelDistributedPublicationRecordV1`.
Its expected binding contains all 15 existing operation coordinates, including
participant incarnation, coordinator/membership epochs and attempt. There is no
registry, session replacement, reconnect, rekey, epoch issuance or network I/O.
There is no mutable access to the underlying record and no cloning the ingress.

The caller is responsible for provisioning the correct key-to-participant
association and expected coordinates. Parsing a non-weak Ed25519 key does not
establish membership, key ownership, revocation status or certificate policy.
This key type and signing domain are distinct from external-anchor/issuer keys.

`open` reads a 32-byte challenge through the existing safe
`rustix::rand::getrandom` pattern used by the profiler. Partial reads are filled;
errors, zero or invalid counts fail closed. At most four complete all-zero
samples are attempted. Linux/Android use the OS API; other targets fail closed.
Kernel entropy initialization may block, and no wall-clock bound is claimed.
There is no public deterministic challenge setter or entropy callback. Test
injection exists only in the private `cfg(test)` module and branch.

Challenge freshness is probabilistic under the OS entropy assumption. A new
ingress rejects an old session's signature, but a peer can sign the same claim
for a new challenge. This is not durable replay protection, restart-safe
deduplication, incarnation allocation or authorization to retry an operation.

## Acceptance

The signature preimage is the exact fixed-size concatenation:

1. `FE2O3/DISTRIBUTED-RECEIPT-ORIGIN/V1\0`.
2. The pinned 32-byte peer public key.
3. The 32-byte session challenge.
4. The expected 431-byte canonical operation description.
5. The supplied 488-byte receipt description.

`signing_preimage` exposes these public bytes, not a signer or private-key API.
`accept_signed_receipt` checks exact length, calls Ed25519 `verify_strict`, then
uses the existing canonical receipt decoder and `record_untrusted_receipt`.
No record changes occur on rejection. There are no new trusted callbacks,
native queue/backend calls, allocators, completion/release/retry operations or
authority conversions. Module state and preimage buffers are fixed-size; the
module does not allocate on acceptance. No end-to-end allocation measurement or
performance claim has been made.

The result is an `OriginAuthenticatedDistributedReceiptV1`: an authenticated
peer-origin claim whose receipt getter deliberately retains the untrusted model
type. A signature does not make a claim of publication or completion truthful.
The wrapper may be copied as descriptive evidence; it is not a single-use
operational capability and has no execution-authority implementation.

Existing sequence semantics are unchanged: first sequence is 1, first Completed
is invalid, only Published may advance to Completed or FailedMayStillExecute,
exact duplicates are idempotent and conflicting/gapped/stale claims reject.
Connection loss is sticky and rejects new claims while permitting an exact
duplicate. Timeout and observer-drop notifications do not synthesize outcomes.
Ordinary Rust drop has no native resource or state transition to perform.

## Dependencies And Compatibility

The runtime adds the same pinned `ed25519-dalek =2.2.0`, default-features=false,
fast/zeroize configuration as `fe2o3-external-anchor-protocol`. It promotes its
existing rustix workspace dependency from dev-only to production with `rand`,
already enabled in production by `fe2o3-profiler-protocol`. Cargo.lock changes
only the runtime's existing dependency list; no package version is added or
updated. No existing feature defaults, constructors or execution paths change.
The new entropy call has an explicit target gate and does not assert that the
rest of the Linux/KFD runtime is portable to other targets.

The ingress consumes the model's public receipt API from signed cebd.
It does not modify the separate W19 receipt codec candidate, its 13 proof inputs,
or any frozen qualification records. This is a runtime consumer of descriptive
claims, not a Worker, Context or KFD execution-authority path. Origin
authentication must not be presented as qualification of the W19 codec or the
whole runtime.

## CPU Validation

Seventeen source tests cover strict signature/key/context rejection, every receipt and
signature byte, all 15 expected-coordinate substitutions, valid signed malformed
wire fields, all six outcome tags, sequence/duplicate/loss rules, entropy
partial/error/zero handling and separate-session replay behavior. Compile-fail
examples cover forged wrappers, deterministic challenges, cloning/resetting the
ingress and conversion to Worker V3 execution authority.

One stable-name test leaves entropy injection absent and exercises public
`open` through the actual OS adapter, then authenticates one receipt. It checks
the nonzero challenge and exact empty session state, not uniqueness or RNG
correctness. Its complementary target branch expects `EntropyUnavailable`.
The original 16-test source draft and patch remain preserved separately.

The 2026-10-01 MI300X-host CPU campaign completed all 28 planned stages:

- 17 focused tests passed in each of debug and release profiles.
- Full runtime suite: 1,918 passed, 32 pre-existing ignored tests, none filtered.
- Full model suite: 1,130 passed, 19 pre-existing ignored tests, none filtered.
- Six doctests passed, including five compile-fail examples.
- Strict runtime/model Clippy checks passed; three fresh test executables and
  their dependency records were retained.

The CPU command's SSH connection ended with exit 255 (`Broken pipe`). Its
original transport result and collection remain rejected. A separate root
review adopted only the complete recovered CPU result after replaying all
28 actual stage logs, four dependency Git commands, seven transport receipts,
source/tool identities, executable hashes and saved process-closure records.
The review also checked every member of both durable archives and separate
outer/direct-inner sensitive-data scans. This is not an inference from partial
streamed output or a relabeling of the failed SSH command.

The source inventory has SHA-256
`9fa2e453d8b1144bb229937310db5bd94ecc3c9b2d04825bde2dad476198b567`;
the retained complete CPU result has SHA-256
`d7cf24f05d8e03d0f6cbceed6778b808ff0c18e38919f5c411dd45b546c89443`.
The separate root CPU-adoption record has SHA-256
`59a2721e9fd57e637422136b1ac316f655f59267c44c991441cfccc8e5b64361`.
Raw custody is retained in
`/mnt/c/fe2o3-a4-origin-remote-cpu-20261001-attempt-1`; these local artifacts
are not a self-contained public replay bundle. Scan coverage is limited to
the documented known-value and JSON-key checks, not a universal secrets audit.
This campaign predates integration with other in-flight runtime changes and
does not qualify the combined integration tree.

No theorem or cryptographic primitive implementation is added here; the pinned
Ed25519 and OS entropy implementations remain dependencies.

Later packets must bind actual native publication/completion receipt issuance,
durable epoch/attempt and deduplication policy, and authenticated two-host
transport/fault tests. This ingress alone closes none of those gates.
