# Fixed Dispatch Preparation Custody V1

R89 implements CONTROL-2 above R86 whole-roster data retention, R87 pending
allocation custody and R88 session transitions. It retains the complete
preparation, including a successful dispatch awaiting caller settlement.
The [local record](evidence/local-r89-dispatch-preparation-2026-09-11/README.md)
separates source acceptance from native execution and executable refinement.

## One Production Sequencer

The private `FixedDispatchPreparationCustodyV1` owns original packets and data,
generation, the existing planner's output, authenticated program identities,
converted data and its exact initialization premises, completed code prefix,
current code/kernarg typestate, resolved packet prefix and completed dispatch.
Executable envelopes are borrowed synchronously; no artifact borrow enters
terminal storage. The production and CPU-fixture memory adapters forward only
existing primitives. Neither supplies a second planner, allocator or loader.

Generation enters the owner before the first fallible stage. Planning and
output-capacity reservation precede control allocation. R86 conversion checks
the whole original roster before moving any member. The returned roster enters
the owner immediately; unpublished initialized content is never reconstructed
using post-dispatch recovery rules.

Each returned control token enters its owner slot before materialization or
later validation. Materialization borrows that slot. Before a consuming R88
seal/map/retain transition, an exact `InSession` identity marker replaces the
slot; R88 retains the actual token on admitted failure. The marker is an
observation, not a reconstructed usable token. Before allocation returns a token,
R87 pending custody or R88 allocation-output custody retains returned native
state. Successful code authorities remain rooted during address resolution and
then move into a preallocated prefix.

Commit prechecks cardinalities, capacities and required owners, then moves them
without callbacks or allocation into a completed dispatch inside the same
preparation owner. Original initialized-content descriptors, writable ranges,
snapshot storage, roles and generation provenance remain exact. Persistent role
assignment happens while the completed dispatch remains rooted. One-shot
extraction requires complete, nonfailed preparation; error/panic forbids retry
or extraction even if completed output exists.

After control preparation starts, failure quarantines the session without
native cleanup, refund or another currentness callback. The original preparation
panic is resumed. Existing backing records and charges remain retained. Terminal
preparation custody grants no publication, disposal, recovery or retry authority.

## Callers And Compatibility

Both persistent bind cardinalities root preparation before their catch boundary
and model loan. Only `()` or an error passes through construction/retake; the
actual completed dispatch stays in preparation. On failure the queue retains
the actual owner in terminal `Preparation` custody. The address-free stage
projection reports `Preparing`, including completed-but-unaccepted construction.

Ordinary fresh, recycled and detached wrappers share this sequencer while
preserving their signatures and generation admission. Pristine abort keeps its
distinct continuation rules. The ordinary wrapper's owner is still local:
durable outer constructor custody remains NATIVE-2. Subsequent persistent
validation/attachment is also outside R89's completed-preparation boundary.

The existing planner, native call order, code image and kernarg bytes are
unchanged. R89 adds retained metadata proportional to admitted program, packet
and data rosters plus fixed current-stage storage. It adds no encoded-image
copy; bounded metadata is not free or a performance result. Existing infallible
planner allocations and process-aborting allocation failure remain outside the
unwind guarantee.

## Validation And Remaining Work

CPU tests use real loader/planner/sequencer code and R86/R88 adapters over
fake-native records and the queue foundation. They check exact original data
bytes/identities/charges, control ownership, program images and resolved
addresses, patched kernarg bytes, roles and generation. They inject every code
ordinal's allocation/materialization/seal/map failure, native/currentness panic,
partial mapping, natural model-aperture rejection and later packet/commit failure.
Stage injection and source-location guards are identified separately from
native faults; a source-location check is not an outer settlement test.

- CONTROL-3 must cover the full operation/retake outcome cross-product,
  preserve the first panic on simultaneous failures, root later validation and
  attachment, and make terminal queue state dominate retry classification.
- NATIVE-2 primary/auxiliary/replacement construction, DATA-ADOPT, ISSUE and
  runtime completion remain open. R89 installs no generated adoption hooks.
- Kernarg/executable/control budgets, aggregate terminal metadata accounting,
  unreturned backend mappings and teardown/unmap/release custody remain open.
- No new Verus theorem, whole-adapter refinement, protected compiler
  acceptance, Linux qualification, performance gain or HIP/HSA parity follows
  from these CPU tests.
