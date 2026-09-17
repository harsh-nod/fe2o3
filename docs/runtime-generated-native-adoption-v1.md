# N5 Native DATA Adoption

Status: implemented development integration, not accepted N5 or R126 qualification.
R125 remains the last accepted Native CPU/test checkpoint. A1/A2, #182 and
HIP/HSA parity remain open.

The subsequent [private I2 integration](runtime-generated-issue-v1.md) extends
the adopted state with publication, physical completion and Stop-only disposal.
The adoption packet and its original evidence below remain distinct; current
qualification is recorded in the linked I2 receipt.

## Ownership

The generated preparation driver now installs its private nonpublishing DATA
hooks. Public preparation and reservation remain nonexecuting; activation is
still private until COMPLETE supplies output/readback delivery. The original
N5 packet described here did not implement ISSUE; the later I2 adapter is linked
above.

Context joins its original preparation binding, stream hold, exact source roster,
allocation records and credits to the existing backend shell. The source handoff
revalidates the original HSACO/program/ABI and brackets native adoption with
Worker currentness. A closing failure retains the original carrier and native
owners and terminalizes both Context and backend.

Before native effects the backend roots an Entering record, pre-reserved DATA
roster and logical queue-lane lease. It retains the exact opaque native lane
when available. The original packet stays in its shell until the lower binding
call takes ownership; it is never reconstructed from metadata. Supported routes:

- Fresh VM and primary queue, with the memory session retained during initialization.
- Initial binding of a bootstrap primary queue.
- New auxiliary queue under the retained primary owner.
- Rebinding an existing lane after disposal of its old cached recipe.

Every returned initialization prefix remains backend-owned until the complete
roster transfers to the lower constructor. Constructor failure retains its
consumed owners under the existing lower custody rules. No publication or
completion reservation is created. New queues emit the existing profiler
creation event after their handle is rooted.

Same-stream ordinary submit, async copy and stream destruction reject while the
generated shell exists. The lane lease excludes other ordinary submissions;
persistent dispatch and generated adoption also exclude each other. Live native
generated phases participate in fail-closed backend Drop and shutdown checks.

## Stop And Failure

Successful adoption becomes Adopted. Stop latches Retiring before entering the
exact native lane's pristine abort. The returned DATA vector is installed in the
backend record immediately. Forward disposal records the completed prefix and
the ordinal handed to the lower consuming release; the untouched suffix remains
in the backend on any error or panic, including failure on the final item.

Only complete disposal, closing native-device currentness and exact stream/lane
identity permit Retired, lane-lease removal and Context record/credit disposal.
The async driver releases the stream hold last. Cleanup is payload-independent:
stale Worker authority must not prevent disposal of an already-owned resource.
An authentic empty-prefix Stop succeeds only with no shell or backend lane lease.

Entered failures are terminal, not recoverable rollbacks. Panic settlement
preserves the first panic even if poisoning panics. Metadata-only disposal
rejects Entering, Adopted and Retiring records. A retired record must have the
complete disposed DATA count and no remaining control or DATA owner.

Activation validates the original binding/source and installs the stream hold;
it does not require a free lane. A private immutable, payload-free readiness hook
checks the exact hold, empty native prefix, backend health and lane availability
on each advancement. Lane contention or persistent-compute exclusion leaves the
driver pending with its original carrier, ticket and hold. It neither repeats
source hashing nor registers shells, charges native allocations, transfers the
control packet or polls native work. Existing round-robin progress remains
responsible for the occupying operation; waiting has no bounded-liveness claim.

Readiness and adoption run in the same owner turn without scheduler interleaving.
The backend still rechecks capacity at native entry. Readiness errors and panics
are terminal, not retryable capacity outcomes; the phase is armed before calling
the hook. Stop and drain can retire an indefinitely waiting operation's authentic
empty prefix. Rollback of partially constructed native owners is not claimed.
The [readiness receipt](evidence/dev-n5-adoption-readiness-2026-09-17/README.md)
records the additional CPU-only regression campaign.

## Qualification Boundary

The [development evidence](evidence/dev-n5-native-adoption-2026-09-17/README.md)
separates CPU validation from unexecuted hardware probes. Runtime tests exercise
metadata exclusion, authenticated empty-prefix and shell-only Stop, disposal
counts, first/middle/last release failure, suffix retention, callback destruction,
terminal diagnostics and first-panic preservation. These are not native fault
injection or a proof of machine-code execution.

Two ignored native probes admit the exact repository vecadd fixture using its
real checked device, loader, ABI, original fixture bytes and fixed packet. They
exercise cold/bootstrap primary, AUX, rebound, pristine abort and complete
shutdown without calling submit. They deliberately bypass only generated
carrier registration, use no synthetic Worker authority, and cannot establish
production Worker authentication or end-to-end async qualification.

All eight MI300X GPUs were occupied when checked. Neither new native probe ran;
no remote scratch, workload or cleanup was created by this packet. Native route
success, native failure matrices, production carrier/async composition, formal
correspondence, aggregate accounting and matched performance remain open. N5
must not be accepted from compiled probes or CPU ownership tests alone.

The subsequent [I2 receipt](evidence/dev-i2-generated-issue-2026-09-17/README.md)
records both N5 native probes passing during a later idle-device window, along
with two new native publication/completion probes. Those bounded route-success
results do not supply the missing failure, protected-composition or formal gates.
