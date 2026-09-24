# Native Compiler Execution Receipt Transport V2

## Status

This additive `fe2o3-artifact-transaction` API durably associates opaque receipt
bytes with the complete [native execution subject V2](compiler-execution-subject-v2.md).
It authenticates neither compiler execution nor the receipt body. No compiler,
publication, load or launch authority follows from decoding or recovery.
Protected issuance, Cargo, Worker and runtime consumers are not switched to it.
This library increment completes no #272 milestone or tutorial GPU qualification.

V1 retains its raw sidecar, public types, identity transcript and authorization
rules. Shared private filesystem mechanics serve both versions; V2 has distinct
types, filename, framing and identity domain, with no legacy fallback.

## Lifecycle

`publish_compiler_execution_receipt_transport_v2` requires an exact non-DIRECT
Building attempt and ready V4 handoff. Under the output lock it validates the
payload, reconstructs the complete subject, and compares every canonical byte.
It writes a private temporary, syncs, renames without replacement, syncs the
directory, then validates committed bytes and pinned ready-record/payload custody.
Content is streamed through the shared transaction hash before and after commit;
file timestamps alone cannot detect every in-place change.
An exact retry is idempotent; a different subject or body cannot replace it.
Publication after consumption is rejected.

`recover_compiler_execution_receipt_transport_with_currentness_v2` requires the
matching raw V4 token and lease and retains their lock. It reconstructs the
subject from that immutable owner, reads the sidecar and checks currentness
again. Call it before mapping the token into a concrete verifier-owned value;
arbitrary user-supplied `AsRef` callbacks are not admitted by this API.

`recover_compiler_execution_receipt_transport_v2` accepts a ready or consumed
occurrence. Ready recovery reconstructs from the actual payload. Consumption
deletes that payload, so the envelope retains all 690 subject bytes for restart
comparison against the caller's expected typed subject. The consumed record
must also match producer, attempt, slot, outer binding, length and transaction.
This comparison does not authenticate an untrusted expected subject or defend
against a directory owner replacing both the content and expected evidence.
Higher layers must independently authenticate their policy and attestation.

Recovery accepts Building without a backend receipt, or BackendClaimed/Completed
with a backend receipt. It does not reopen publication or token acquisition.
Stale attempts, failed phases, orphan sidecars, unknown names and simultaneous
ready/consumed records fail closed.

## Wire Contract

The private filename is `compiler-execution-receipt-v2`. All integers are little
endian. For opaque body length `B`, `1 <= B <= 65536`, total length is `754 + B`.

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | Magic `F2O3CRT2` |
| 8 | 2 | Version `2` |
| 10 | 2 | Zero flags |
| 12 | 8 | Total length |
| 20 | 4 | Zero reserved word |
| 24 | 690 | Complete canonical SubjectV2 |
| 714 | 8 | Opaque body length |
| 722 | B | Opaque receipt body |
| 722 + B | 32 | Transport identity |

The identity is SHA-256 of
`fe2o3.compiler-execution-receipt-transport.identity.v2\0`, `u64_le(722 + B)` and
the exact prefix. Recovery compares the complete stored subject to the typed
canonical expected subject; no second subject allocation or decode is needed.
The opaque body is deliberately not interpreted by this lower-level crate.

## Resources And Failures

All native calls use the existing work/storage ledger and V4 storage ceiling.
Keep complete borrowed input owners prepaid, including enclosing/spare capacity.
Publication checks a minimum subject-plus-body floor; locked recovery checks
the additive subject, lease and token floor. These checks cannot account for
unrelated objects on the caller's behalf.

Recovered content owns one move-only wire buffer; `exact_bytes()` borrows only
the body range without copying. Recovery returns `(owner, storage)` with admitted
but unreserved additional storage, including actual wire capacity and headers.
Reserve it before retaining or using the owner. Scope cleanup restores entry
storage without resetting work, peak storage, denial history or ledger identity.

Native reads reserve actual buffer capacity before filling. All variable-sized
readback buffers and remaining validation work are admitted before the sidecar
rename. Resource refusal cannot first occur after that commit. A later I/O or
custody failure may leave a committed sidecar; recovery must inspect it instead
of assuming publication never happened. Filesystem directory/registry metadata
still has separate bounded protocol accounting, not allocator or RSS guarantees.

Tests cover independent golden hashes, every-byte mutation/truncation, full
subject substitutions before and after payload deletion, private-file and
record-inode checks, phase gates, commit faults, and exact/one-short work and
storage boundaries. These are local content/custody tests, not protected proof
execution or hardware evidence. See [production convergence](production-pipeline-convergence-v1.md).
