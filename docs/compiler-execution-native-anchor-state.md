# Native Durable Anchor State

## Status

`DurableExternalAnchorV2/V3` in `fe2o3-external-anchor-service` integrate native
sealed key custody with the same private durable-state engine used by V1. They
support initialization, strict reopen, atomic open-or-initialize, and signed
challenge exchange. They do not export keys or descriptors and do not upgrade
V1 owners. The state-file format and observation wire remain unchanged.

`serve_connected_peer_v2/v3` now drive those native owners through the same
peer I/O and scheduling implementation as V1, using the original resource ledger.
These are direct service operations, not activated protected startup. The helper
and inherited daemon entrypoints still consume V1 owners. Trusted deployment
provenance, executable measurement, lifecycle custody and process admission must
be established separately. Neither these APIs nor their tests complete M0-M7,
prove kernel semantics, or qualify any additional end-to-end GPU kernel.

## Invariants

The core owns the exact root descriptor and exclusive advisory lock, admitted
public key, cached sequence/head, and poison status. It never owns a signing key.
Each native wrapper owns its same-family key capability and borrows the actual
deployment on every operation. Complete deployment binding and exact nonroot
service UID/GID are checked before root mutation and again before response return.

An `Advance` from the exact prior position writes a fixed next-state image,
syncs it, atomically renames it, and syncs the directory before updating cached
state and signing. Exact retries at the proposed position do not rewrite state.
`Recover` observes only the exact prior or proposed position without advancing.

The core sets poison before the first persistence hook. Any persistence error
or unwind leaves it poisoned; reopening is mandatory, even if a rename occurred.
Malformed existing state is never reset by open-or-initialize. Only exact absence
permits genesis. The shared V1 path uses this same engine and poison behavior.

State reads/writes now make single attempts: short transfers and EINTR reject
rather than retry. Existing-state open uses NONBLOCK so a FIFO reaches metadata
rejection without blocking in open. Regular-file, ownership, mode, link-count,
exact length, key identity, checksum and framing checks remain in force. These
changes apply to V1 too; successful wire and state semantics are unchanged.

| Failure Point | State / Required Action |
| --- | --- |
| Input accounting, wrong context/key or credentials before persistence | No advance; consuming constructor errors close their inputs |
| Persistence error or unwind | Live core is poisoned; reopen and recover |
| Signing/key/resource failure after a completed persistence operation | No response returned; commit remains, so retry or reopen/recover |
| Invalid existing state at reopen | Reject; never silently reinitialize |

A constructor's late revalidation failure can follow genesis creation. Treat it
as a failed operation with possible persistent effects, not a rolled-back create.
Likewise, resource refusal is not a transaction rollback promise. Failure-injection
tests exercise the real I/O ordering; they are not power-loss tests or independent
verification of filesystem durability guarantees.

## Accounting

All operations use the caller's original `Budget::with_prepaid_scope`, including
nested key checks/signing. No new work ledger is introduced. Let `S` be
`NativeExternalAnchorStorageV2` (shared by both families), `K` the key's full
retained charge and `D` the borrowed deployment charge:

```text
R = size_of::<(OwnedFd, S)>()
A = size_of::<(DurableExternalAnchorVn, S)>() + KeyCapabilityVn::FILE_STORAGE
```

`A` conservatively includes the whole wrapper plus the sealed-image File charge.
Constructors consume prepaid `R + K`, borrow `D`, and return growth `A - R - K`.
Reserve that growth before retaining the owner; retire full `A` after drop.
On consuming failure, inputs are closed but their old reservations remain for
caller cleanup. Exchange borrows `A + D + 184` for an exact-sized challenge and
returns the full unreserved `size_of::<([u8; 288], S)>()` response charge. Wrong
lengths are not scanned and need no wire floor.

| Operation | Successful Work | Additional Peak |
| --- | ---: | --- |
| initialize / open / open_or_initialize | 244760 | `STATE_STORAGE + Key::IO_STORAGE` |
| exchange | 378656 | `STATE_STORAGE + Key::IO_STORAGE` |

Outer state work is 42504; construction additionally prepays 65536 for public-key
validation. Nested key operations charge the same ledger as they occur. Full
quotas are not reserved as one atomic transaction, so late budget refusal can
follow a durable commit. Entry work eight precedes input-floor checks. Every
scope restores entry storage on return/error/unwind without refunding work or
clearing historical peaks or first denials. `STATE_STORAGE` is the exported,
size-derived allowance for core, challenge, response, I/O and control staging.
Quotas are logical accounting, not syscall-time, generated-stack or RSS bounds.

See the [validation checkpoint](evidence/conditional-native-durable-anchor-20260926.md)
for source attribution, test failures, evidence hashes and remaining integration.

## Native Peer Loop

Each public native serving function borrows the anchor and actual same-family
deployment, consumes one prepaid peer descriptor, and uses the original caller
ledger. It revalidates key/deployment custody and exact service credentials before
transport validation. Public callers cannot supply a signer, persistence engine
or transport implementation, or convert a V1 owner into native authority.

The shared schedule is receive, durable exchange, send, retire response, increment
the checked exchange counter. V1 preserves its existing unmetered I/O behavior.
Native serving charges before every validation, poll, receive and send attempt,
including retries. The packet and response formats remain 184 and 288 bytes;
ancillary data and noncanonical packet lengths still reject. A response is
reserved immediately after exchange and remains charged through sending. Errors
and unwinds restore entry storage without refunding work or erasing history.

The input floor is `A + D + NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2`. The fixed
`NATIVE_EXTERNAL_ANCHOR_PEER_FRAME_STORAGE_V2` covers live packet/control staging;
peak additionally includes nested anchor/key scratch or the retained response.
The consumed descriptor closes on return/error/unwind. Its old reservation stays
for caller cleanup. Success returns the **full**, unreserved
`NATIVE_EXTERNAL_ANCHOR_PEER_REPORT_STORAGE_V2`, independent of that descriptor's
charge. Reserve the report charge before retention and retire it after drop.
The report is an inert V1 exchange counter, not an admitted V1 owner.

On Linux, logical work is:

| Stage | Charge |
| --- | ---: |
| Outer entry/frame | 31496 |
| Initial native key revalidation | 68360 |
| Endpoint validation | 13064 |
| Each poll attempt | 1032 |
| Each receive attempt | 6920 |
| Each durable exchange | 378656 |
| Each send attempt | 10248 |

With one successful poll per I/O, no retries, and EOF observed by a receive,
`n` exchanges cost `120872 + n * 397888`. Quotas bound logical attempts, **not**
idle poll time, syscall latency, filesystem durability or total service lifetime.
The existing 30-second response-publication timeout is unchanged.

Failure after persistence may leave a commit with no delivered response. Reopen
and recover/retry, never roll back or recreate state. Failure after send may mean
the peer already received the response. Peer shape and native custody are not
protected peer-identity, lifecycle or process-profile admission; startup callers
must establish those separately. No protected startup or compiler/GPU authority
is granted by this API or its tests. See the
[peer validation checkpoint](evidence/conditional-native-anchor-peer-20260926.md).
