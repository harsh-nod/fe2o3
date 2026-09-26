# Native Durable Anchor State

## Status

`DurableExternalAnchorV2/V3` in `fe2o3-external-anchor-service` integrate native
sealed key custody with the same private durable-state engine used by V1. They
support initialization, strict reopen, atomic open-or-initialize, and signed
challenge exchange. They do not export keys or descriptors and do not upgrade
V1 owners. The state-file format and observation wire remain unchanged.

These are direct service operations, not activated protected startup. The peer
loop, helper and daemon entrypoints still consume V1 owners. Trusted deployment
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
