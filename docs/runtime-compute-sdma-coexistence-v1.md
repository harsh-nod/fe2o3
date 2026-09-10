# Disjoint Compute/SDMA Coexistence: R66

R66 implements the native checker and runtime scheduling changes from OVL-1/2
in the [A1/A2 swarm plan](runtime-a1-a2-swarm-plan.md). It is a prerequisite for
compute/copy overlap, not evidence that the GPU executed both simultaneously.
It does not complete A1/A2, establish HIP/HSA parity, increase native queue depth
or supply compiler execution authority.

## Admitted Profile

The new profile composes the existing primary persistent compute lane with its
exact directional SDMA pair. Compute retains one or three full-extent DeviceLocal
bindings. Copies use the existing directional-persistent H2D/D2H single-request
or window paths and their existing range/currentness checks. Copy buffers may
retain padding; the existing full-extent compute restriction is unchanged.

All compute allocations must differ from every retained copy device allocation.
Disjoint byte subranges of the same allocation are insufficient. A generation
change cannot disguise shared backing. Ordinary/generic copy publication,
same-device D2D, striped SDMA, XGMI and auxiliary-compute coexistence remain outside
this profile. Copy/compute dependencies still take precedence over readiness.

The historical `GFX942_PERSISTENT_LOCAL_COMPUTE_ADAPTER_MANIFEST_V1` and its digest
are preserved byte-for-byte. They describe the earlier R52 exclusion profile.
`GFX942_COMPUTE_SDMA_COEXISTENCE_MANIFEST_V1` has a separate frozen identity and
claim boundary. Neither descriptive manifest grants executable authority.

## Native Checks

The lower KFD checker borrows actual retained custody; it does not issue a
transferable permit or cache a decision across a mutation. Before compute bind,
it checks the candidate inputs against every retained directional-copy endpoint.
Before directional-copy admission, it checks the candidate plus existing copies
against the persistent attachment and actual retained dispatch/control roster.
Both paths retain the existing operational-currentness and publication checks.

The extraction checks include:

- Exact primary queue, device/VM domain, directional pair and attachment
  generation, with unsupported or terminal owners rejected.
- Native allocation identity, persistent owner extent, pool generation and exact
  mapped-storage identity. Host endpoints must belong to the retained host
  allocation session and queue owner.
- Both complete 64-slot directional ledgers, including settled-but-retained
  copies. No records disappear merely because completion was observed.
- Reciprocal slot/window-anchor membership, exact slot generation and completion
  identity, valid packet count and request extent. Missing, overlapping or
  malformed records reject.
- Private single-copy provenance set only by the actual directional-persistent
  preparation path. Matching buffer shapes cannot relabel an ordinary request.
- Agreement between the attached compute entries, actual dispatch data
  authorities and retained control-storage roster.

The pure storage comparison validates domains and checks allocation IDs
independently of generation equality. Native identity extraction and the
meaning of those identities remain explicit adapter obligations. The checker
does not prove physical memory nonaliasing or inspect kernel machine effects.

## Runtime Integration

Three scheduling exclusions are narrowed: persistent compute publication,
directional copy publication and pending-compute observation. Private runtime
filters check exact retained operation owners, one/three-binding allocation
rosters, native-storage markers and directional copy class before allowing an
attempt. These are scheduling filters, not substitutes for the lower checker.

Failure/cancellation dependencies remain ahead of publication. A rejected
attempt does not imply device cancellation or replay permission. Existing
completion, retirement, allocation retention, quarantine and explicit shutdown
paths are unchanged. Legacy detached/quiescent test helpers are not supplied a
disjointness result as fabricated quiescence evidence.

The new scan is bounded and allocation-free. Its general model permits at most
16 compute identities and 258 copy device endpoints; the actual admitted compute
profile remains one or three bindings. Cross-roster comparisons are bounded by
4,128, with at most 120 additional compute-uniqueness comparisons. Native ledger
validation and domain checks have additional bounded cost. This is not a timing
measurement or a whole-progress-tick constant-time claim.

## Verification Boundary

The R66 Verus source proves its executable bounded scan returns exactly the
quantified domain/uniqueness/nonalias specification, including its loops. Scalar
and cyclic-slot properties are also covered. The Rust model uses the matching
algorithm; correspondence between the separately compiled Rust and Verus source
is reviewed, not automatically established by compiling one shared source.

R66 adds 16 verified obligations and eight deliberately failing mutations:
omitted endpoint, alias hidden by generation, foreign VM, duplicate compute
allocation, invalid window extent/generation/completion and excessive roster
size. The authenticated runner retains its existing source, toolchain, negative
quality, closure and transcript gates with the new exact roster and pins.

Native ledger extraction, private-token enforcement, physical identity meaning,
actual publication/completion observations, Linux/KFD/firmware/GPU behavior and
general compiler semantic-to-machine refinement are not proved by this scan.
Existing R25/R57 proofs do not become proofs of the new native composition.

CPU tests cover all bounded compute/copy alias placements, cyclic slots,
malformed native record/window extraction, both runtime publication orders,
one/three-binding compute, H2D/D2H, aliases, dependencies, delayed completion,
timeout, retry and terminal retention. Copy retirement while compute remains
active is tested separately. Scripted publication establishes test-model custody
behavior, not live native publication or physical execution overlap.

## Remaining Acceptance

Successful full native coexistence admission still needs live ring/control/
doorbell authority and an independently captured MI300X qualification. Native
CPU extraction fixtures deliberately lack that authority. Previous R26/R65
hardware evidence predates this change and cannot qualify it.

The next hardware campaign must exercise both publication orders with the exact
admitted compute fixture, disjoint copies, full independent output/padding
checks, retained native submission identities, explicit retirement and cleanup.
Only a separately checked compatible device timeline can establish physical
compute/copy overlap; host pending intervals cannot. Matched HIP/HSA performance
remains a distinct workload-specific campaign.

During this implementation's local validation, SSH to `mi300x` failed hostname
resolution. No remote staging, GPU work or shared-host cleanup was necessary.
Some existing local telemetry tests also require socket inspection unavailable
under the current sandbox. These environment failures must be reported in the
validation record, not skipped and relabeled as an unrestricted passing suite.

Native budgets, cross-run versions, generated typed async authority, active-work
drain qualification, high native depth and the later multi-device/distributed
milestones remain tracked separately in the swarm plan and #182.

The [local validation record](evidence/local-r66-coexistence-2026-09-10/README.md)
retains exact final test/proof results and the environment limitations. It does
not reuse earlier MI300X captures as qualification for this composition.
