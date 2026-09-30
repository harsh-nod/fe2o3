# Distributed Publication Description Contract V1

This is a production-neutral A0 draft for issue #182. It is not an A4 session,
transport, receipt authentication or distributed execution implementation. No
runtime, Worker, Context or KFD path consumes these model values. The shared
declaration implementation passed nine focused CPU tests on a fresh 1,123-test
model executable, with no warnings. Full positive discovery and the two later
unfiltered positive runs each verified 23 functions with zero errors. This is
component development evidence, not a completed signed mutation campaign or a
full-model test result.

## Ownership

`fe2o3-runtime-model::distributed_publication_contract` owns fixed-size untrusted
descriptions and one constant-space consistency record. Device generations,
allocations, mappings, queues, packet publication and native completion remain
owned by existing direct-KFD primitives. Compiler plan semantics and generated
launch admission remain with their existing owners. This module implements none
of them and creates no operational capability.

The operation binding names a runtime instance, participant and incarnation,
coordinator and epoch, exact membership description and epoch, distributed run,
logical operation and attempt, artifact, execution plan, placement plan, target
description and runtime model. These are caller-supplied names. Nonzero checking
is structural only: it establishes neither uniqueness nor admission nor origin.
Independent participant/incarnation and coordinator coordinates are all compared;
no epoch is inferred from an address, process counter, connection or clock.

The existing `IdentityDigestV1`, `RuntimeArtifactIdV1` and `RuntimeModelIdV1` are
reused. Distributed wrappers do not redefine device identities or compiler IR.
An execution-plan descriptor refers to an external plan; the module does not
compute, authenticate or reinterpret that plan's identity.

## Wire Boundary

Both encodings have distinct literal domain prefixes, schema 1, zero reserved
fields and exact constant lengths. Digests are 32 bytes; numeric coordinates and
receipt sequences are unsigned 64-bit little-endian values. A receipt embeds
one complete operation description, then its sequence, outcome tag and three
zero reserved bytes. No variable fields, strings, native handles, GPU addresses,
file descriptors or authority tokens exist in either format.

Encoding returns a stack array and decoding borrows an exact-length slice. The
module allocates no heap storage and performs no I/O. Decoding checks both nested
headers, lengths, tags, reserved bytes and nonzero coordinates. Its output remains
explicitly untrusted. Canonical bytes are descriptive identity preimages, not a
cryptographic digest, signature, authenticated envelope or proof result.

## Record Rules

One record retains its immutable operation binding, at most one receipt and an
independent sticky connection-interruption bit. There is no map, operation
registry, message queue, transcript or retained native resource. Caller-created
collections of records require their own aggregate budgets and admission.

- The first receipt sequence is exactly 1. A first `Completed` claim rejects:
  this conservative profile requires the preceding `Published` description.
- Only `Published` can advance, at sequence 2, to `Completed` or
  `FailedMayStillExecute`. Every other outcome closes that exact attempt.
- `DefinitelyNotPublished` means a peer-reported final classification for this
  attempt, not a pre-publication snapshot that the same attempt can later leave.
  This module neither authenticates that finality nor issues retry permission.
- An exact repeat of the last receipt is idempotent, even after interruption.
  Conflicting duplicate, stale sequence, sequence gap, binding substitution or
  illegal transition rejects without any record mutation.
- Connection loss preserves the existing receipt and permanently closes new
  receipt admission in this profile. With no receipt, publication remains unknown;
  with a Published receipt, publication remains reported but completion unknown.
  It does not assert participant loss or manufacture an indeterminate peer receipt.
- Timeout and observer Drop leave every field unchanged. Neither event implies
  nonpublication, cancellation, completion, quiescence or permission to retry.

The record exposes descriptive observations only. There is no automatic retry,
attempt reissue, release decision or conversion to a runtime completion state.
Even `DefinitelyNotPublished` and `Completed` are only peer claims. An attacker
can construct them; recording a structurally consistent claim does not make it
true. Reconnect, gap reconciliation, late stronger evidence, epoch replacement,
durable deduplication and coordinator failover are explicitly unsupported.

## Test And Proof Boundary

The nine focused CPU tests cover canonical byte vectors; every truncated prefix;
extension/header/tag/reserved-field refusals; successful-decoding canonicality
under single-byte mutations; all 15 coordinate substitutions; zero and maximal
coordinates; the complete reachable state/sequence/outcome/interruption matrix;
and bounded traces retaining terminal and unknown descriptions.

`classifier_body.rs` contains the unchanged executable classifier, commit and
observation-event bodies used by this model. `declarations.rs` now shares the
complete record, receipt, binding and all 15 coordinate declarations with the
draft `distributed_publication_contract_v1.rs` proof. The record remains neither
Copy nor Clone. This source extraction changes no constructor or wire code.

The explicit proof representation maps each typed descriptive identity to its
full 32-byte digest payload, not a scalar hash or precomputed equality Boolean.
The checker binds the real `identity.rs` types and the eight model wrappers;
Rust's derived structural equality and its Verus `PartialEqSpecImpl` bridge are
an explicit source-calibrated library/compiler boundary. No cryptographic
collision resistance, authentication or digest-preimage theorem follows.

The measured positive proof states the exact raw-state decision and
record/observation frames without constructor-validity or reachability premises. Results
include full binding comparison, refusal atomicity, duplicate idempotence, and
sticky interruption with no fabricated receipt. `SequenceExhausted` is defensive
but unreachable even on arbitrary well-typed raw states: reaching checked_add
requires the incoming u64 sequence to exceed the previous sequence. No positive
exhaustion witness is claimed. The independent first-receipt and terminal rules
still bound public reachable sequences to 1 or 2.

The measured count comprises 19 generated Clone implementations, one complete
binding-equality lemma and the three actual classifier/record/observation methods.
It is not 23 independent lifecycle properties. Strict all-feature/all-target
model Clippy, warning-free no-default checking, scoped formatting and whitespace
checks passed against the same native bytes. The declaration extraction had a
fresh CPU rebuild; no earlier pre-extraction executable was reused to qualify it.

Canonical codec and constructor refinement remain separate obligations; neither
is inherited from the classifier theorem or from the CPU canonicality tests.

Sixteen source-constructed negative mutations cover weakened binding
or sequence checks, conflicting duplicates, initial Completed claims, terminal
reopening, interruption bypass, lost record updates, mutation before refusal,
and timeout/loss/drop distinctions. Three representative actual-body mutations
were observed under their exact classifier/record/observation selectors. Each
reported zero verified functions and one postcondition error, with the reviewed
root-module selection note and no compiler warning or VIR error. These captures
remain observation-only; their packets are not retroactively qualified by
pinning the note. The final signed three-positive/16-negative campaign, relocated
closure check and complete retained model executable run remain pending.

Authentication/key management, session-to-participant binding, complete artifact
and plan preimages, epoch issuance, durable journals, truthful publication and
completion observations, local authority admission, native resource retention,
clock behavior, leases, transport, process failure and two-host execution remain
outside this module. There is deliberately no `authenticated` flag, trusted
adapter, new backend trait or shared-memory fiction. A future production adapter
must establish each relevant boundary separately before these descriptions can
affect an execution decision. This draft closes no A0/A4 milestone or HIP/HSA
parity claim.
