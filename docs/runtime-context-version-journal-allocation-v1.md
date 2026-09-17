# Context Journal Allocation Foundation

This V5 development increment joins the bounded journal model to actual Context
allocation lifecycles. It does not complete V5/V6, A1/A2, #182 or HIP/HSA parity.
Accepted checkpoints remain Native R125 CPU/test, Admission R118B C1-C3 and
Resources R116/V3. Qualification is recorded in
[the development receipt](evidence/dev-v5-allocation-journal-2026-09-17/README.md).

## Public Boundary

`RuntimeContextV1::open_with_version_journal_v1(backend, A, W)` is the only opt-in
entry. Both bounds must be in `1..=1_048_576`. Invalid bounds reject before
backend enumeration. Other returned construction errors retain the original
backend value, possibly after enumeration; they do not promise pristine backend
state. Successful construction binds a fresh Context generation and zero writer
watermark before the first local ID. There is no activation, reset, import or
caller-selected identity API. Default `open` behavior is unchanged.

The only journal observation is metadata usage. It grants no initializedness,
content version, native currentness, input lease, reuse or execution authority.
Writes, launches, copies and backend aliases remain unrestricted. Their effects
are not journaled yet, so no globally valid lineage can be read even after a
successful operation. Writer slots are preallocated but not issued here.

## Ownership

- Ordinary allocation checks journal capacity before consuming the original
  Context ID. It reserves map storage and roots the exact allocation/device/extent
  provisionally before backend entry. Metadata or credit rejection before entry
  leaves no provisional owner; consumed IDs remain consumed.
- An allocated backend handle is rooted in the Context map before credit and
  Live-phase commit. Zero or duplicate handles remain rooted in a sealed Context.
- Only allocation-specific `SettledNoOwner` or definite pre-effect `Rejected`
  refunds provisional enrollment. Generic Quiescent, Terminal and panic retain
  it, including when there is no public handle. Existing diagnostics and panic
  payloads are preserved. Dropping the private move-only ticket never refunds.
- Generated shell registration enrolls the complete original-ID roster, including
  read-only and unused members, before transferring control into the backend.
- Explicit release, cleanup and generated whole-roster retirement validate exact
  journal references before disposal. Only confirmed disposal returns slots.
  Credit disposal precedes journal retirement and removal of Context handles.
  Internal invariant failure seals and retains metadata rather than retrying an
  already-disposed backend object. Pending/Unknown model membership cannot retire.
- Cleanup counts unidentified journal records independently of allocation credits;
  it cannot return the backend while an ambiguous no-handle attempt remains.

## Model And Cost

Batch enrollment has atomic returned errors, requires canonical keys within each
batch, and permits previously unused lower keys in subsequent batches. It uses
caller-owned empty output storage temporarily for sorting, restoring it on error.
Cost is `O(A log(k + 1) + k log(k + 1))`; retirement is `O(k)`. Neither operation
allocates. Cleanup's retained count is O(1); explicit provisional-usage inspection
scans the bounded phase roster. No wall-clock performance claim is made.

These checks preserve the allocation/free partition from valid prestates; they
are not a detector of every unrelated corruption. Disposed-key history is not
stored by the model. Production supplies fresh nonwrapping original Context IDs,
so old references cannot revive after slot reuse. Caller-authored model values
remain inert and are not production disposal authority.

## Remaining Work

V6 must join genuine writer IDs and authenticated Success/NoEffect/Unknown
settlement before callbacks. Full mutation coverage includes failed disposal
that changes bytes, generated DATA adoption, all launch/copy families, completion
and cancellation paths, and backend aliases. Ordered writers, explicit Unknown
disposal/recovery, input leases and aggregate residency remain separate work.

New allocation transitions and this Rust adapter have executable tests and static
review, not mechanically proved correspondence. Existing V4-J1 proof bytes are
unchanged and prove issuance contents only. Storage-allocation failure and backend
enumeration unwind are not fault-injected here. Scripted shell tests are not
protected native GPU qualification. No GPU, formal solver or HIP/HSA benchmark
is supplied by this increment.
