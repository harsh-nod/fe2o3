# Context Generated Input Leases V1

Development above `ace978213dd4e5e1ff10cb00d875bf24581aae83`, extending
[ordinary typed input custody](runtime-context-kernel-read-leases-v1.md) and
[generated writer integration](runtime-context-version-journal-generated-v1.md).
Accepted checkpoints remain Native R125 CPU/test, Admission R118B C1-C3 and
Resources R116/V3. V6, A1/A2, #182 and native/formal/performance acceptance remain open.

## Issue Ownership

The opt-in Context journal acquires a whole-allocation read lease for each original
ReadOnly generated shell member. ReadWrite and WriteOnly members retain exclusive
writer custody instead. Shell logical IDs are unique under the existing KFD plan
validator; pointer-fixup participation does not change membership. All-read-only
attempts acquire readers without allocating a writer slot.

Source records and model requests are prepared before the submission ID, writer
Begin, reader Begin or native preparation. A busy source or insufficient reader
capacity rejects the whole preflight without acquiring a prefix. The reader arena
shares the configured writer capacity, not writer occupancy. Its domain binds the
genuine submission consumer plus original stream, unpublished hold and shell key.
The independent root exists before model acquisition and before a backend handle;
ordinary release cannot discharge it even when no writer or submission record exists.

The retained generated attempt, installed submission record and private completion
receipt bind the exact first-reference/count marker. Validation checks the entire
source/request/reference roster, exact domain, allocation/backend/credit records,
full extents, ordered fresh incarnations and live model leases against the original
authenticated shell roster. Read leases grant neither initialized-byte authority
nor permission to execute or reuse a kernel.

## Protected Settlement

Before native readback or retirement, the owner proves each ReadOnly member has
its exact own reader and no additional reader; writable members must have none.
An aggregate count alone is insufficient. Issued Stop defers on foreign readers.
Protected completion preserves its existing conservative behavior: a contention
error reaching the outer issue-failure wrapper terminalizes and quarantines; it
is not a retryable completion path.

Own readers remain live through original readback, read-only validation, native
DATA retirement, native submission release and closing source-currentness checks.
The exact completion receipt then permits reader release, followed by writer
Success, shell retirement and status publication. A quiescent issued Stop releases
readers before writer Unknown and whole-shell disposal. NoEffect is rejected before
any reader mutation. Generic poll/wait/drain/cancel/event/release cannot settle these
roots. Pending work and pre-settlement errors retain custody, including attempts
without returned backend handles.

Final shell disposal still requires no reader root or marker and aggregate zero
readers on every member. Pre-ISSUE adoption cleanup retains its existing zero-reader
rule. Once exact quiescence releases readers, a subsequent writer/shell/credit fault
does not resurrect them: remaining ownership is retained terminally, but the read
leases have conclusively ended.

## Cost And Evidence

For r ReadOnly members among n bounded shell members, preparation sorts in
O(r log r), retains O(r) metadata and reserves storage before Begin. Exact shell
matching uses O(n + r log r) work plus the existing writer checks. No new solver
proof, Rust/native correspondence or performance bound is claimed.

The [qualification archive](evidence/dev-v6-generated-read-leases-2026-09-17/README.md)
records CPU qualification separately from native acceptance. Fourteen new tests
cover domains and extents, reader capacity independent of writers, busy inputs,
pre-handle ordinary release, generic observers, no-handle failures, exact neighboring
completion/Stop, marker/root/reference/domain corruption, foreign readers, stale
settlement receipts, post-release disposal panic, default and graph profiles, and
NoEffect rejection. Callbacks assert delivered status outside panic containment.

Fixtures use real Context shell identities with explicitly assumed native
completion/disposal. Foreign readers and batch substitution are private model
premises, not public races or native fault injections. These tests do not execute
protected GPU kernels or prove native retirement call ordering. That ordering is
source-reviewed; production Worker V3/machine refinement, native concurrency/fault
campaigns, initialized-input authority, ordered writers, cross-run reuse, aggregate
residency, formal reader composition and matched HIP/HSA measurements remain open.
