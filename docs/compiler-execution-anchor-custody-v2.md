# Native External-Anchor Custody V2

## Scope

`ProtectedExternalAnchorServiceAdmissionV2` freshly consumes an anchor endpoint
and service pidfd. It is move-only, exposes no `AsFd`, and accepts no admitted
V1 owner as an upgrade. Identity getters return inert observations.

The private state currently reuses the V1 mechanical representation. Shared
`continuity.rs` selects an explicit inspection mode; this representation reuse
does not invoke legacy admission or grant legacy authority to the native owner.
Native methods always select bounded native I/O. Legacy methods retain their
existing allocating procfs readers, interrupted-poll retry loop, and duplicate
FD minimum zero. Native duplicates use minimum FD three and CLOEXEC.

## Admission And Continuity

Admission requires an unnamed, connected, nonblocking read-write Unix
`SOCK_SEQPACKET` endpoint, CLOEXEC descriptors, and exact peer UID/GID agreement
with the supplied service identity. The pidfd must identify that same live peer
process, with retained descriptor identity, target PID and process start time.
Revalidation repeats identity, credentials and liveness checks; transfer checks
require the exact retained socket and pidfd objects, not merely matching credentials.

Production always requires the anchor service UID to differ from the issuer's
effective UID. Continuity also rejects a changed issuer UID. Native same-UID
fixtures exist only under `cfg(test)`; the legacy `test-support` bypass cannot
activate them or weaken production native admission.

The shared schedule performs these target/start-time probe pairs:

| Operation | Target Probes | Start-Time Probes | Liveness Checks |
|---|---:|---:|---:|
| admit | 7 | 7 | 3 |
| validate_continuity | 4 | 4 | 2 |
| validate_transfer | 19 | 19 | 9 |
| try_clone_for_transfer | 27 | 27 | 13 |

Each successful liveness check performs two zero-timeout polls around one
non-reaping waitid probe. The exact pidfd ioctl is preferred; only its existing
ENOTTY/EINVAL fallback permits a procfs target proof. Quotas prepay every possible
fallback, even when the ioctl succeeds and no fdinfo record is read.

## Bounded Native I/O

Each procfs record uses one fixed 4097-byte buffer and at most 4097 read attempts.
Positive short reads continue; EOF completes the read. Filling all 4097 bytes
rejects an oversized record: the admitted record limit is 4096 bytes.
EINTR and other read errors fail immediately, without retry. Native poll likewise
makes one libc call without retry; shared continuity classifies its observation.

Paths use fixed decimal buffers, not String, Vec or formatting. Native reads
preserve procfs magic checks, numeric-self inode consistency and existing path
semantics. Shared parsers preserve fdinfo field validation and binary stat
command handling. For fdinfo, UTF-8 validation precedes oversize rejection.
Errors retain bounded labels and errno data; caller-requested formatting is
outside the inspection quota.

## Resources And Ownership

Every substantive operation uses the caller's typed resource budget through
`Budget::with_prepaid_scope`; no fresh or unlimited child ledger is created.
Let `S` denote the native storage-receipt type and define:

```text
D = size_of::<(ProtectedExternalAnchorServiceAdmissionV2, S)>()
P = 2 * size_of::<(OwnedFd, S)>()
IO_STORAGE = 8*D + 4*P + 2*4097 + 8192
```

Tuple sizes include padding. These charges describe logical userspace custody,
not kernel socket queues or descriptor-backed kernel allocation.

| Operation | Required Input Floor | Returned Additional Charge | Work |
|---|---:|---:|---:|
| admit | P | D - P | 61094120 |
| validate_continuity | D | none | 35135624 |
| validate_transfer | D + P | none | 164928104 |
| try_clone_for_transfer | D | P | 234150760 |

For the corresponding probe count `n`, total work is exactly
`8 + 512*1024 + 2*n*((4097 + 64)*1024 + 16*4097)`.
This prepays bounded read attempts, 64 additional weighted calls per record,
512 outer weighted calls, and byte processing including buffer initialization.

Entry work eight precedes the input-floor check. Remaining work is charged before
scratch reservation or inspection. Scratch requires actual entry storage plus
`IO_STORAGE`, not merely the minimum input floor plus scratch. Layout guards cover
owners, result/error envelopes, syscall structures and bounded leaf scratch.
Success, refusal and unwind restore entry storage without refunding accepted
work or clearing peak/denial history; unrelated prepaid owners remain charged.

Preserve consumed input charge P and immediately reserve admission's D-P delta
before retaining or using the result. Clone returns a separate unreserved P
charge while the original D remains live. Consuming refusal closes its inputs;
retire their charge after return. On eventual drop or transfer, retire the full
owner charge, not its construction delta. Scratch is a logical allowance, not a
bound on generated stack, RSS, instructions, syscall latency or kernel memory.

## Remaining Boundary

Custody is a point-in-time transport observation, not authority, protected proof,
signing-key custody, exclusive endpoint ownership, an independently deployed service,
monotonic persistence or GPU execution credit. Native supervisor binding and
the consuming launch path remain to be integrated with the native program,
policy, signing key, credentials, root and anchor inputs. This owner alone
does not activate serving, a protected launcher, or downstream native protocols.
