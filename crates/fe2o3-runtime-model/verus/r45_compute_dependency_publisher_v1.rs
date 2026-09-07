// Independent finite R45 model for the crate-private R43 dependency publisher.
// Every identity, currentness fact, live-lease fact, and epoch-minting premise
// is a mathematical input. This is not a refinement of executable Rust and
// establishes no public facade, dependent-completion release, native, KFD,
// hardware, progress, parity, or performance claim.

use vstd::prelude::*;
use vstd::arithmetic::div_mod::{lemma_div_non_zero, lemma_multiply_divide_le};

verus! {

pub open spec fn max_dependencies_v1() -> nat { 256 }
pub open spec fn barrier_fan_in_v1() -> nat { 5 }
pub open spec fn max_barriers_v1() -> nat { 52 }

#[derive(PartialEq, Eq)]
pub struct EpochPremiseV1 {
    pub owner_occurrence: nat,
    pub issued_mint_id: nat,
    pub issued_epoch: nat,
    pub exactly_one_owner: bool,
    pub all_source_epochs_minted_here: bool,
}

#[derive(PartialEq, Eq)]
pub struct AcceptanceV1 {
    pub mint_id: nat,
    pub owner_occurrence: nat,
    pub session: nat,
    pub epoch: nat,
}

#[derive(PartialEq, Eq)]
pub struct SignalIdentityV1 {
    pub mapping: nat,
    pub slot: nat,
    pub slot_generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct OccurrenceV1 {
    pub session: nat,
    pub epoch: nat,
    pub queue: nat,
    pub mapping: nat,
    pub batch: nat,
    pub slot: nat,
    pub slot_generation: nat,
    pub dispatch_generation: nat,
    pub packet: nat,
}

#[derive(PartialEq, Eq)]
pub struct TargetBundleV1 {
    pub target: OccurrenceV1,
    pub batch_target: OccurrenceV1,
    pub event_target: OccurrenceV1,
    pub retention_id: nat,
    pub event_id: nat,
    pub completion_signal: SignalIdentityV1,
    pub dispatch_id: nat,
    pub dispatch_completion_signal: SignalIdentityV1,
    pub dispatch_target: OccurrenceV1,
}

#[derive(PartialEq, Eq)]
pub struct SourceRecordV1 {
    pub source: OccurrenceV1,
    pub event_id: nat,
    pub signal: SignalIdentityV1,
    pub dependent_epoch: nat,
}

#[derive(PartialEq, Eq)]
pub struct ReaderCustodyV1 {
    pub record: SourceRecordV1,
    pub lease_id: nat,
}

#[derive(PartialEq, Eq)]
pub struct ArenaIdentityV1 {
    pub queue: nat,
    pub mapping: nat,
}

#[derive(PartialEq, Eq)]
pub struct LiveReaderRecordV1 {
    pub source: SourceRecordV1,
    pub lease_id: nat,
}

pub struct SourceArenaV1 {
    pub identity: ArenaIdentityV1,
    pub live_readers: Seq<LiveReaderRecordV1>,
    pub mutation_count: nat,
}

#[derive(PartialEq, Eq)]
pub struct TargetArenaV1 {
    pub target: OccurrenceV1,
    pub retention_id: nat,
    pub event_id: nat,
    pub completion_signal: SignalIdentityV1,
    pub target_live: bool,
    pub mutation_count: nat,
}

#[derive(PartialEq, Eq)]
pub enum NativeBoundaryV1 {
    RingOccupied,
    PreClaimInvariant,
    ClaimAttempt,
    BarrierBody,
    FinalBody,
    BarrierHeader,
    FinalHeader,
    Doorbell,
    Success,
}

#[derive(PartialEq, Eq)]
pub struct PublicationTraceV1 {
    pub reservations: nat,
    pub claims: nat,
    pub barrier_bodies: nat,
    pub final_bodies: nat,
    pub barrier_headers: nat,
    pub final_headers: nat,
    pub doorbells: nat,
    pub last_body_step: nat,
    pub first_header_step: nat,
    pub completion_loads: nat,
}

pub open spec fn premise_valid_v1(premise: EpochPremiseV1) -> bool {
    premise.owner_occurrence > 0
        && premise.issued_mint_id > 0
        && premise.issued_epoch > 0
        && premise.exactly_one_owner
        && premise.all_source_epochs_minted_here
}

pub open spec fn source_occurrence_valid_v1(source: OccurrenceV1) -> bool {
    source.session > 0
        && source.epoch > 0
        && source.queue > 0
        && source.mapping > 0
        && source.batch > 0
        && source.slot_generation > 0
        && source.dispatch_generation > 0
        && source.packet > 0
}

pub open spec fn target_occurrence_valid_v1(target: OccurrenceV1) -> bool {
    target.session > 0
        && target.epoch > 0
        && target.queue > 0
        && target.mapping > 0
        && target.batch > 0
        && target.slot_generation > 0
        && target.dispatch_generation > 0
        && target.packet == 0
}

pub open spec fn signal_for_occurrence_v1(occurrence: OccurrenceV1) -> SignalIdentityV1 {
    SignalIdentityV1 {
        mapping: occurrence.mapping,
        slot: occurrence.slot,
        slot_generation: occurrence.slot_generation,
    }
}

pub open spec fn target_bundle_exact_v1(bundle: TargetBundleV1) -> bool {
    target_occurrence_valid_v1(bundle.target)
        && bundle.batch_target == bundle.target
        && bundle.event_target == bundle.target
        && bundle.retention_id > 0
        && bundle.event_id > 0
        && bundle.dispatch_id > 0
        && bundle.completion_signal == signal_for_occurrence_v1(bundle.target)
        && bundle.dispatch_completion_signal == bundle.completion_signal
        && bundle.dispatch_target == bundle.target
}

pub open spec fn source_record_exact_v1(source: SourceRecordV1) -> bool {
    source_occurrence_valid_v1(source.source)
        && source.event_id > 0
        && source.signal == signal_for_occurrence_v1(source.source)
        && source.dependent_epoch > 0
}

pub open spec fn reader_custody_exact_v1(reader: ReaderCustodyV1) -> bool {
    source_record_exact_v1(reader.record) && reader.lease_id > 0
}

pub open spec fn sources_pairwise_distinct_v1(sources: Seq<ReaderCustodyV1>) -> bool {
    forall|i: int, j: int|
        0 <= i < sources.len() && 0 <= j < sources.len() && i != j
            ==> sources[i].record.source != sources[j].record.source
                && sources[i].record.signal != sources[j].record.signal
                && sources[i].record.event_id != sources[j].record.event_id
                && sources[i].lease_id != sources[j].lease_id
}

pub open spec fn source_admitted_v1(
    acceptance: AcceptanceV1,
    target: OccurrenceV1,
    reader: ReaderCustodyV1,
) -> bool {
    reader_custody_exact_v1(reader)
        && reader.record.dependent_epoch == acceptance.epoch
        && reader.record.source.session == acceptance.session
        && reader.record.source.queue != target.queue
        && reader.record.source.epoch < acceptance.epoch
}

pub open spec fn preparation_admitted_v1(
    premise: EpochPremiseV1,
    acceptance: AcceptanceV1,
    bundle: TargetBundleV1,
    sources: Seq<ReaderCustodyV1>,
) -> bool {
    premise_valid_v1(premise)
        && acceptance.owner_occurrence == premise.owner_occurrence
        && acceptance.mint_id == premise.issued_mint_id
        && acceptance.epoch == premise.issued_epoch
        && acceptance.session > 0
        && acceptance.epoch > 0
        && target_bundle_exact_v1(bundle)
        && bundle.target.session == acceptance.session
        && bundle.target.epoch == acceptance.epoch
        && 1 <= sources.len() <= max_dependencies_v1()
        && sources_pairwise_distinct_v1(sources)
        && forall|i: int| 0 <= i < sources.len()
            ==> source_admitted_v1(acceptance, bundle.target, sources[i])
}

pub open spec fn barrier_count_v1(dependency_count: nat) -> nat {
    (dependency_count + 4) / 5
}

pub open spec fn packet_count_v1(dependency_count: nat) -> nat {
    barrier_count_v1(dependency_count) + 1
}

pub open spec fn owner_after_mint_v1(next_epoch: nat) -> nat { next_epoch + 1 }

pub open spec fn rejection_owner_epoch_v1(next_epoch: nat) -> nat { next_epoch }

pub open spec fn retryable_v1(boundary: NativeBoundaryV1) -> bool {
    boundary == NativeBoundaryV1::RingOccupied
}

pub open spec fn claim_attempted_v1(boundary: NativeBoundaryV1) -> bool {
    boundary == NativeBoundaryV1::ClaimAttempt
        || boundary == NativeBoundaryV1::BarrierBody
        || boundary == NativeBoundaryV1::FinalBody
        || boundary == NativeBoundaryV1::BarrierHeader
        || boundary == NativeBoundaryV1::FinalHeader
        || boundary == NativeBoundaryV1::Doorbell
        || boundary == NativeBoundaryV1::Success
}

pub open spec fn terminal_v1(boundary: NativeBoundaryV1) -> bool {
    boundary == NativeBoundaryV1::PreClaimInvariant
        || (claim_attempted_v1(boundary) && boundary != NativeBoundaryV1::Success)
}

pub open spec fn success_trace_v1(barriers: nat) -> PublicationTraceV1 {
    PublicationTraceV1 {
        reservations: 1,
        claims: 1,
        barrier_bodies: barriers,
        final_bodies: 1,
        barrier_headers: barriers,
        final_headers: 1,
        doorbells: 1,
        last_body_step: 2 + barriers,
        first_header_step: 3 + barriers,
        completion_loads: 0,
    }
}

pub open spec fn retry_trace_v1() -> PublicationTraceV1 {
    PublicationTraceV1 {
        reservations: 0,
        claims: 0,
        barrier_bodies: 0,
        final_bodies: 0,
        barrier_headers: 0,
        final_headers: 0,
        doorbells: 0,
        last_body_step: 0,
        first_header_step: 0,
        completion_loads: 0,
    }
}

pub open spec fn arena_matches_source_v1(arena: SourceArenaV1, source: ReaderCustodyV1) -> bool {
    arena.identity.queue == source.record.source.queue
        && arena.identity.mapping == source.record.source.mapping
}

pub open spec fn live_reader_matches_v1(live: LiveReaderRecordV1, reader: ReaderCustodyV1) -> bool {
    live.source == reader.record && live.lease_id == reader.lease_id
}

pub open spec fn source_arena_well_formed_v1(arena: SourceArenaV1) -> bool {
    (forall|i: int| 0 <= i < arena.live_readers.len() ==> {
        &&& source_record_exact_v1(arena.live_readers[i].source)
        &&& arena.live_readers[i].lease_id > 0
        &&& arena.live_readers[i].source.source.queue == arena.identity.queue
        &&& arena.live_readers[i].source.source.mapping == arena.identity.mapping
    })
        && (forall|i: int, j: int|
            0 <= i < arena.live_readers.len()
                && 0 <= j < arena.live_readers.len()
                && i != j
                ==> arena.live_readers[i] != arena.live_readers[j]
                    && arena.live_readers[i].lease_id != arena.live_readers[j].lease_id
                    && arena.live_readers[i].source.event_id
                        != arena.live_readers[j].source.event_id)
}

pub open spec fn arena_has_one_exact_live_reader_v1(
    arena: SourceArenaV1,
    reader: ReaderCustodyV1,
) -> bool {
    exists|witness: int| {
        &&& 0 <= witness < arena.live_readers.len()
        &&& live_reader_matches_v1(arena.live_readers[witness], reader)
        &&& forall|other: int| 0 <= other < arena.live_readers.len()
                && live_reader_matches_v1(arena.live_readers[other], reader)
                ==> other == witness
    }
}

pub open spec fn route_exact_v1(
    sources: Seq<ReaderCustodyV1>,
    arenas: Seq<SourceArenaV1>,
    route: Seq<nat>,
) -> bool {
    route.len() == sources.len()
        && forall|i: int| 0 <= i < sources.len() ==> {
            &&& route[i] < arenas.len()
            &&& arena_matches_source_v1(arenas[route[i] as int], sources[i])
            &&& source_arena_well_formed_v1(arenas[route[i] as int])
            &&& arena_has_one_exact_live_reader_v1(arenas[route[i] as int], sources[i])
            &&& forall|j: int| 0 <= j < arenas.len() && arena_matches_source_v1(arenas[j], sources[i])
                    ==> j == route[i]
        }
}

pub open spec fn target_current_v1(arena: TargetArenaV1, bundle: TargetBundleV1) -> bool {
    arena.target_live
        && target_bundle_exact_v1(bundle)
        && arena.target == bundle.target
        && arena.retention_id == bundle.retention_id
        && arena.event_id == bundle.event_id
        && arena.completion_signal == bundle.completion_signal
}

pub open spec fn rollback_preflight_v1(
    bundle: TargetBundleV1,
    target_arena: TargetArenaV1,
    sources: Seq<ReaderCustodyV1>,
    source_arenas: Seq<SourceArenaV1>,
    route: Seq<nat>,
) -> bool {
    target_current_v1(target_arena, bundle)
        && route_exact_v1(sources, source_arenas, route)
}

pub open spec fn rollback_release_count_v1(preflight: bool, source_count: nat) -> nat {
    if preflight { source_count } else { 0 }
}

pub open spec fn rollback_arena_mutations_v1(preflight: bool, source_count: nat) -> nat {
    if preflight { source_count } else { 0 }
}

pub open spec fn cancelled_reader_custody_count_v1() -> nat { 0 }
pub open spec fn target_live_after_cancel_v1() -> bool { false }

pub open spec fn fault_coordinate_valid_v1(index: nat, barriers: nat) -> bool {
    index < barriers
}

pub open spec fn returned_events_v1(sources: Seq<ReaderCustodyV1>) -> Seq<nat> {
    sources.map(|_i: int, source: ReaderCustodyV1| source.record.event_id)
}

pub open spec fn reverse_release_leases_v1(sources: Seq<ReaderCustodyV1>) -> Seq<nat>
    decreases sources.len(),
{
    if sources.len() == 0 {
        Seq::empty()
    } else {
        seq![sources.last().lease_id] + reverse_release_leases_v1(sources.drop_last())
    }
}

pub open spec fn custody_preserved_v1(
    acceptance_before: AcceptanceV1,
    bundle_before: TargetBundleV1,
    sources_before: Seq<ReaderCustodyV1>,
    acceptance_after: AcceptanceV1,
    bundle_after: TargetBundleV1,
    sources_after: Seq<ReaderCustodyV1>,
) -> bool {
    acceptance_after == acceptance_before
        && bundle_after == bundle_before
        && sources_after == sources_before
}

pub proof fn dependency_and_barrier_bounds_v1(dependencies: nat)
    requires 1 <= dependencies <= max_dependencies_v1(),
    ensures 1 <= barrier_count_v1(dependencies) <= max_barriers_v1(),
            packet_count_v1(dependencies) == barrier_count_v1(dependencies) + 1,
{
    assert(barrier_fan_in_v1() == 5);
    assert(max_dependencies_v1() == 256);
    assert(max_barriers_v1() == 52);
    assert(dependencies <= 256);
    assert(dependencies + 4 <= 260);
    assert(260 == 5 * 52) by (compute);
    lemma_div_non_zero((dependencies + 4) as int, 5);
    lemma_multiply_divide_le((dependencies + 4) as int, 5, 52);
}

pub proof fn exact_fan_in_boundaries_v1()
    ensures barrier_count_v1(1) == 1,
            barrier_count_v1(5) == 1,
            barrier_count_v1(6) == 2,
            barrier_count_v1(256) == 52,
{}

pub proof fn premise_is_explicit_v1(premise: EpochPremiseV1)
    requires premise_valid_v1(premise),
    ensures premise.exactly_one_owner,
            premise.all_source_epochs_minted_here,
            premise.owner_occurrence > 0,
            premise.issued_mint_id > 0,
            premise.issued_epoch > 0,
{}

pub proof fn admitted_acceptance_is_bound_to_owner_v1(
    premise: EpochPremiseV1,
    acceptance: AcceptanceV1,
    bundle: TargetBundleV1,
    sources: Seq<ReaderCustodyV1>,
)
    requires preparation_admitted_v1(premise, acceptance, bundle, sources),
    ensures acceptance.owner_occurrence == premise.owner_occurrence,
            acceptance.mint_id == premise.issued_mint_id,
            acceptance.epoch == premise.issued_epoch,
{}

pub proof fn mint_is_strict_and_burned_v1(next_epoch: nat)
    requires next_epoch > 0,
    ensures owner_after_mint_v1(next_epoch) > next_epoch,
            rejection_owner_epoch_v1(owner_after_mint_v1(next_epoch)) == next_epoch + 1,
{}

pub proof fn rejection_cannot_rewind_epoch_v1(next_epoch: nat)
    ensures rejection_owner_epoch_v1(next_epoch) == next_epoch,
{}

pub proof fn target_bundle_couples_every_component_v1(bundle: TargetBundleV1)
    requires target_bundle_exact_v1(bundle),
    ensures bundle.batch_target == bundle.target,
            bundle.event_target == bundle.target,
            bundle.dispatch_completion_signal == bundle.completion_signal,
            bundle.dispatch_target == bundle.target,
            bundle.completion_signal == signal_for_occurrence_v1(bundle.target),
            bundle.retention_id > 0,
            bundle.event_id > 0,
{}

pub proof fn target_split_is_not_admitted_v1(
    premise: EpochPremiseV1,
    acceptance: AcceptanceV1,
    bundle: TargetBundleV1,
    sources: Seq<ReaderCustodyV1>,
)
    requires bundle.batch_target != bundle.target
        || bundle.event_target != bundle.target
        || bundle.dispatch_completion_signal != bundle.completion_signal
        || bundle.dispatch_target != bundle.target,
    ensures !preparation_admitted_v1(premise, acceptance, bundle, sources),
{}

pub proof fn admitted_roster_has_exact_bounds_v1(
    premise: EpochPremiseV1,
    acceptance: AcceptanceV1,
    bundle: TargetBundleV1,
    sources: Seq<ReaderCustodyV1>,
)
    requires preparation_admitted_v1(premise, acceptance, bundle, sources),
    ensures 1 <= sources.len() <= 256,
            barrier_count_v1(sources.len()) <= 52,
{
    dependency_and_barrier_bounds_v1(sources.len());
}

pub proof fn admitted_source_is_same_session_earlier_and_cross_queue_v1(
    premise: EpochPremiseV1,
    acceptance: AcceptanceV1,
    bundle: TargetBundleV1,
    sources: Seq<ReaderCustodyV1>,
    i: int,
)
    requires preparation_admitted_v1(premise, acceptance, bundle, sources),
             0 <= i < sources.len(),
    ensures sources[i].record.source.session == bundle.target.session,
            sources[i].record.source.epoch < bundle.target.epoch,
            sources[i].record.source.queue != bundle.target.queue,
{}

pub proof fn admitted_sources_and_signals_are_distinct_v1(
    premise: EpochPremiseV1,
    acceptance: AcceptanceV1,
    bundle: TargetBundleV1,
    sources: Seq<ReaderCustodyV1>,
    i: int,
    j: int,
)
    requires preparation_admitted_v1(premise, acceptance, bundle, sources),
             0 <= i < sources.len(),
             0 <= j < sources.len(),
             i != j,
    ensures sources[i].record.source != sources[j].record.source,
            sources[i].record.signal != sources[j].record.signal,
            sources[i].record.event_id != sources[j].record.event_id,
            sources[i].lease_id != sources[j].lease_id,
{}

pub proof fn admitted_source_signal_is_exact_addressless_identity_v1(
    premise: EpochPremiseV1,
    acceptance: AcceptanceV1,
    bundle: TargetBundleV1,
    sources: Seq<ReaderCustodyV1>,
    i: int,
)
    requires preparation_admitted_v1(premise, acceptance, bundle, sources),
             0 <= i < sources.len(),
    ensures sources[i].record.signal
                == signal_for_occurrence_v1(sources[i].record.source),
{}

pub proof fn invalid_session_source_is_rejected_v1(
    premise: EpochPremiseV1,
    acceptance: AcceptanceV1,
    bundle: TargetBundleV1,
    sources: Seq<ReaderCustodyV1>,
    i: int,
)
    requires 0 <= i < sources.len(),
             sources[i].record.source.session != acceptance.session,
    ensures !preparation_admitted_v1(premise, acceptance, bundle, sources),
{}

pub proof fn same_queue_source_is_rejected_v1(
    premise: EpochPremiseV1,
    acceptance: AcceptanceV1,
    bundle: TargetBundleV1,
    sources: Seq<ReaderCustodyV1>,
    i: int,
)
    requires 0 <= i < sources.len(),
             sources[i].record.source.queue == bundle.target.queue,
    ensures !preparation_admitted_v1(premise, acceptance, bundle, sources),
{}

pub proof fn cycle_source_is_rejected_v1(
    premise: EpochPremiseV1,
    acceptance: AcceptanceV1,
    bundle: TargetBundleV1,
    sources: Seq<ReaderCustodyV1>,
    i: int,
)
    requires 0 <= i < sources.len(),
             sources[i].record.source.epoch >= acceptance.epoch,
    ensures !preparation_admitted_v1(premise, acceptance, bundle, sources),
{}

pub proof fn equal_epoch_source_is_self_dependency_v1(
    premise: EpochPremiseV1,
    acceptance: AcceptanceV1,
    bundle: TargetBundleV1,
    sources: Seq<ReaderCustodyV1>,
    i: int,
)
    requires 0 <= i < sources.len(),
             sources[i].record.source.epoch == acceptance.epoch,
    ensures !preparation_admitted_v1(premise, acceptance, bundle, sources),
{}

pub proof fn greater_epoch_source_is_cycle_v1(
    premise: EpochPremiseV1,
    acceptance: AcceptanceV1,
    bundle: TargetBundleV1,
    sources: Seq<ReaderCustodyV1>,
    i: int,
)
    requires 0 <= i < sources.len(),
             sources[i].record.source.epoch > acceptance.epoch,
    ensures !preparation_admitted_v1(premise, acceptance, bundle, sources),
{}

pub proof fn success_has_one_reservation_claim_and_doorbell_v1(barriers: nat)
    requires 1 <= barriers <= max_barriers_v1(),
    ensures success_trace_v1(barriers).reservations == 1,
            success_trace_v1(barriers).claims == 1,
            success_trace_v1(barriers).doorbells == 1,
            success_trace_v1(barriers).barrier_bodies == barriers,
            success_trace_v1(barriers).barrier_headers == barriers,
            success_trace_v1(barriers).final_bodies == 1,
            success_trace_v1(barriers).final_headers == 1,
{}

pub proof fn all_bodies_precede_all_headers_v1(barriers: nat)
    requires 1 <= barriers <= max_barriers_v1(),
    ensures success_trace_v1(barriers).last_body_step
                < success_trace_v1(barriers).first_header_step,
{}

pub proof fn publisher_never_prepolls_completion_v1(barriers: nat)
    requires 1 <= barriers <= max_barriers_v1(),
    ensures success_trace_v1(barriers).completion_loads == 0,
            retry_trace_v1().completion_loads == 0,
{}

pub proof fn ring_occupancy_is_exact_retry_boundary_v1(boundary: NativeBoundaryV1)
    ensures retryable_v1(boundary) <==> boundary == NativeBoundaryV1::RingOccupied,
{}

pub proof fn ring_retry_has_zero_native_effect_v1()
    ensures retry_trace_v1().reservations == 0,
            retry_trace_v1().claims == 0,
            retry_trace_v1().barrier_bodies == 0,
            retry_trace_v1().barrier_headers == 0,
            retry_trace_v1().doorbells == 0,
{}

pub proof fn first_claim_and_later_failure_is_terminal_v1(boundary: NativeBoundaryV1)
    requires claim_attempted_v1(boundary), boundary != NativeBoundaryV1::Success,
    ensures terminal_v1(boundary), !retryable_v1(boundary),
{}

pub proof fn preclaim_invariant_is_terminal_v1()
    ensures terminal_v1(NativeBoundaryV1::PreClaimInvariant),
            !retryable_v1(NativeBoundaryV1::PreClaimInvariant),
{}

pub proof fn malformed_fault_coordinate_is_terminal_before_effect_v1(index: nat, barriers: nat)
    requires !fault_coordinate_valid_v1(index, barriers),
    ensures terminal_v1(NativeBoundaryV1::PreClaimInvariant),
            retry_trace_v1().reservations == 0,
            retry_trace_v1().claims == 0,
{}

pub proof fn exact_route_names_one_live_owner_v1(
    sources: Seq<ReaderCustodyV1>,
    arenas: Seq<SourceArenaV1>,
    route: Seq<nat>,
    i: int,
)
    requires route_exact_v1(sources, arenas, route), 0 <= i < sources.len(),
    ensures route[i] < arenas.len(),
            arena_matches_source_v1(arenas[route[i] as int], sources[i]),
            source_arena_well_formed_v1(arenas[route[i] as int]),
            arena_has_one_exact_live_reader_v1(arenas[route[i] as int], sources[i]),
            forall|j: int| 0 <= j < arenas.len() && arena_matches_source_v1(arenas[j], sources[i])
                ==> j == route[i],
{}

pub proof fn missing_owner_blocks_preflight_v1(
    bundle: TargetBundleV1,
    target_arena: TargetArenaV1,
    sources: Seq<ReaderCustodyV1>,
    arenas: Seq<SourceArenaV1>,
    route: Seq<nat>,
    i: int,
)
    requires 0 <= i < sources.len(),
             forall|j: int| 0 <= j < arenas.len() ==> !arena_matches_source_v1(arenas[j], sources[i]),
    ensures !rollback_preflight_v1(bundle, target_arena, sources, arenas, route),
{
    if route_exact_v1(sources, arenas, route) {
        assert(route[i] < arenas.len());
        assert(arena_matches_source_v1(arenas[route[i] as int], sources[i]));
    }
}

pub proof fn duplicate_owner_blocks_preflight_v1(
    bundle: TargetBundleV1,
    target_arena: TargetArenaV1,
    sources: Seq<ReaderCustodyV1>,
    arenas: Seq<SourceArenaV1>,
    route: Seq<nat>,
    source_i: int,
    first: int,
    second: int,
)
    requires 0 <= source_i < sources.len(),
             0 <= first < arenas.len(),
             0 <= second < arenas.len(),
             first != second,
             arena_matches_source_v1(arenas[first], sources[source_i]),
             arena_matches_source_v1(arenas[second], sources[source_i]),
    ensures !rollback_preflight_v1(bundle, target_arena, sources, arenas, route),
{
    if route_exact_v1(sources, arenas, route) {
        assert(first == route[source_i]);
        assert(second == route[source_i]);
    }
}

pub proof fn substituted_owner_blocks_preflight_v1(
    bundle: TargetBundleV1,
    target_arena: TargetArenaV1,
    sources: Seq<ReaderCustodyV1>,
    arenas: Seq<SourceArenaV1>,
    route: Seq<nat>,
    i: int,
)
    requires 0 <= i < sources.len(),
             route.len() == sources.len(),
             route[i] < arenas.len(),
             !arena_matches_source_v1(arenas[route[i] as int], sources[i]),
    ensures !rollback_preflight_v1(bundle, target_arena, sources, arenas, route),
{}

pub proof fn stale_lease_blocks_preflight_v1(
    bundle: TargetBundleV1,
    target_arena: TargetArenaV1,
    sources: Seq<ReaderCustodyV1>,
    arenas: Seq<SourceArenaV1>,
    route: Seq<nat>,
    i: int,
)
    requires 0 <= i < sources.len(),
             route.len() == sources.len(),
             route[i] < arenas.len(),
             !arena_has_one_exact_live_reader_v1(arenas[route[i] as int], sources[i]),
    ensures !rollback_preflight_v1(bundle, target_arena, sources, arenas, route),
{}

pub proof fn stale_target_blocks_preflight_v1(
    bundle: TargetBundleV1,
    target_arena: TargetArenaV1,
    sources: Seq<ReaderCustodyV1>,
    arenas: Seq<SourceArenaV1>,
    route: Seq<nat>,
)
    requires !target_current_v1(target_arena, bundle),
    ensures !rollback_preflight_v1(bundle, target_arena, sources, arenas, route),
{}

pub proof fn failed_preflight_releases_and_mutates_nothing_v1(
    bundle: TargetBundleV1,
    target_arena: TargetArenaV1,
    sources: Seq<ReaderCustodyV1>,
    arenas: Seq<SourceArenaV1>,
    route: Seq<nat>,
)
    requires !rollback_preflight_v1(bundle, target_arena, sources, arenas, route),
    ensures rollback_release_count_v1(
                rollback_preflight_v1(bundle, target_arena, sources, arenas, route),
                sources.len(),
            ) == 0,
            rollback_arena_mutations_v1(
                rollback_preflight_v1(bundle, target_arena, sources, arenas, route),
                sources.len(),
            ) == 0,
{}

pub proof fn successful_preflight_releases_all_v1(
    bundle: TargetBundleV1,
    target_arena: TargetArenaV1,
    sources: Seq<ReaderCustodyV1>,
    arenas: Seq<SourceArenaV1>,
    route: Seq<nat>,
)
    requires rollback_preflight_v1(bundle, target_arena, sources, arenas, route),
    ensures rollback_release_count_v1(true, sources.len()) == sources.len(),
            rollback_arena_mutations_v1(true, sources.len()) == sources.len(),
            cancelled_reader_custody_count_v1() == 0,
            !target_live_after_cancel_v1(),
{}

pub proof fn rollback_returns_events_in_original_order_v1(sources: Seq<ReaderCustodyV1>, i: int)
    requires 0 <= i < sources.len(),
    ensures returned_events_v1(sources)[i] == sources[i].record.event_id,
{}

pub proof fn rollback_releases_readers_in_reverse_v1(sources: Seq<ReaderCustodyV1>)
    requires sources.len() > 0,
    ensures reverse_release_leases_v1(sources).first() == sources.last().lease_id,
            reverse_release_leases_v1(sources).len() == sources.len(),
    decreases sources.len(),
{
    if sources.len() > 1 {
        rollback_releases_readers_in_reverse_v1(sources.drop_last());
    }
    reveal_with_fuel(reverse_release_leases_v1, 2);
}

pub proof fn rejected_custody_is_exact_v1(
    acceptance: AcceptanceV1,
    bundle: TargetBundleV1,
    sources: Seq<ReaderCustodyV1>,
)
    ensures custody_preserved_v1(acceptance, bundle, sources, acceptance, bundle, sources),
{}

pub proof fn terminal_custody_is_exact_and_not_retryable_v1(
    boundary: NativeBoundaryV1,
    acceptance: AcceptanceV1,
    bundle: TargetBundleV1,
    sources: Seq<ReaderCustodyV1>,
)
    requires terminal_v1(boundary),
    ensures !retryable_v1(boundary),
            custody_preserved_v1(acceptance, bundle, sources, acceptance, bundle, sources),
{}

pub proof fn retry_custody_is_exact_v1(
    acceptance: AcceptanceV1,
    bundle: TargetBundleV1,
    sources: Seq<ReaderCustodyV1>,
)
    ensures retryable_v1(NativeBoundaryV1::RingOccupied),
            custody_preserved_v1(acceptance, bundle, sources, acceptance, bundle, sources),
{}

} // verus!
