// Independent finite R42 model for addressless compute-event signal custody.
// All identities, phases, packet IDs, duplicate-use classifications, and the
// target-epoch uniqueness premise are contracted mathematical inputs. In
// particular, this model checks but does not establish global target-epoch
// uniqueness and does not bind a target epoch to a target publication.
//
// This proves no Rust-to-Verus or production-Rust refinement and no KFD, HSA,
// HIP, ioctl, AQL packet, address, allocator, panic, atomic, clock, driver,
// firmware, native, hardware, progress, parity, or performance claim.

use vstd::prelude::*;

verus! {

pub open spec fn event_capacity_v1() -> nat { 8192 }
pub open spec fn native_reader_capacity_v1() -> nat { 8192 }

#[derive(PartialEq, Eq)]
pub enum SlotPhaseV1 {
    Bound,
    Published,
    Completed,
    Available,
}

#[derive(PartialEq, Eq)]
pub enum EventBindingV1 {
    Unbound,
    Bound,
}

#[derive(PartialEq, Eq)]
pub struct SourceOccurrenceV1 {
    pub session: nat,
    pub source_epoch: nat,
    pub queue_key: nat,
    pub signal_mapping: nat,
    pub slot: nat,
    pub slot_generation: nat,
    pub dispatch_generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct EventOccurrenceV1 {
    pub event_id: nat,
    pub source: SourceOccurrenceV1,
    pub binding: EventBindingV1,
    pub source_packet_id: nat,
}

#[derive(PartialEq, Eq)]
pub struct NativeReaderLeaseV1 {
    pub lease_id: nat,
    pub event_id: nat,
    pub source: SourceOccurrenceV1,
    pub source_packet_id: nat,
    pub dependent_epoch: nat,
}

// This is an explicit caller premise, not a generated uniqueness witness.
#[derive(PartialEq, Eq)]
pub struct TargetEpochUniquenessPremiseV1 {
    pub session: nat,
    pub dependent_epoch: nat,
    pub uniqueness_contracted: bool,
}

#[derive(PartialEq, Eq)]
pub struct RegistryV1 {
    pub source: SourceOccurrenceV1,
    pub phase: SlotPhaseV1,
    pub source_packet_id: nat,
    pub event_pins: nat,
    pub native_reader_pins: nat,
}

pub open spec fn valid_source_v1(source: SourceOccurrenceV1) -> bool {
    source.session > 0
        && source.source_epoch > 0
        && source.queue_key > 0
        && source.signal_mapping > 0
        && source.slot_generation > 0
        && source.dispatch_generation > 0
}

pub open spec fn valid_registry_v1(state: RegistryV1) -> bool {
    valid_source_v1(state.source)
        && state.event_pins <= event_capacity_v1()
        && state.native_reader_pins <= native_reader_capacity_v1()
        && (state.phase == SlotPhaseV1::Bound
            || state.phase == SlotPhaseV1::Available ==> state.source_packet_id == 0)
        && (state.phase == SlotPhaseV1::Published
            || state.phase == SlotPhaseV1::Completed ==> state.source_packet_id > 0)
}

pub open spec fn valid_unbound_event_v1(state: RegistryV1, event: EventOccurrenceV1) -> bool {
    event.event_id > 0
        && event.source == state.source
        && event.binding == EventBindingV1::Unbound
        && event.source_packet_id == 0
}

pub open spec fn valid_bound_event_v1(state: RegistryV1, event: EventOccurrenceV1) -> bool {
    event.event_id > 0
        && event.source == state.source
        && event.binding == EventBindingV1::Bound
        && event.source_packet_id > 0
        && event.source_packet_id == state.source_packet_id
}

pub open spec fn record_event_v1(state: RegistryV1, event_id: nat) -> (RegistryV1, EventOccurrenceV1) {
    (
        RegistryV1 {
            source: state.source,
            phase: state.phase,
            source_packet_id: state.source_packet_id,
            event_pins: state.event_pins + 1,
            native_reader_pins: state.native_reader_pins,
        },
        EventOccurrenceV1 {
            event_id,
            source: state.source,
            binding: EventBindingV1::Unbound,
            source_packet_id: 0,
        },
    )
}

pub open spec fn record_event_state_or_reject_v1(state: RegistryV1, event_id: nat) -> RegistryV1 {
    if valid_registry_v1(state)
        && state.phase == SlotPhaseV1::Bound
        && state.event_pins < event_capacity_v1()
        && event_id > 0
    {
        record_event_v1(state, event_id).0
    } else {
        state
    }
}

pub open spec fn publish_source_v1(state: RegistryV1, packet_id: nat) -> RegistryV1 {
    RegistryV1 {
        source: state.source,
        phase: SlotPhaseV1::Published,
        source_packet_id: packet_id,
        event_pins: state.event_pins,
        native_reader_pins: state.native_reader_pins,
    }
}

pub open spec fn bind_event_v1(state: RegistryV1, event: EventOccurrenceV1) -> EventOccurrenceV1 {
    EventOccurrenceV1 {
        event_id: event.event_id,
        source: event.source,
        binding: EventBindingV1::Bound,
        source_packet_id: state.source_packet_id,
    }
}

pub open spec fn bind_event_or_reject_v1(
    state: RegistryV1,
    event: EventOccurrenceV1,
) -> EventOccurrenceV1 {
    if valid_registry_v1(state)
        && state.phase == SlotPhaseV1::Published
        && valid_unbound_event_v1(state, event)
    {
        bind_event_v1(state, event)
    } else {
        event
    }
}

pub open spec fn complete_source_v1(state: RegistryV1) -> RegistryV1 {
    RegistryV1 {
        source: state.source,
        phase: SlotPhaseV1::Completed,
        source_packet_id: state.source_packet_id,
        event_pins: state.event_pins,
        native_reader_pins: state.native_reader_pins,
    }
}

pub open spec fn target_epoch_premise_matches_v1(
    state: RegistryV1,
    premise: TargetEpochUniquenessPremiseV1,
) -> bool {
    premise.uniqueness_contracted
        && premise.session == state.source.session
        && premise.dependent_epoch > state.source.source_epoch
}

pub open spec fn reader_admissible_v1(
    state: RegistryV1,
    event: EventOccurrenceV1,
    premise: TargetEpochUniquenessPremiseV1,
    duplicate_target_use: bool,
) -> bool {
    valid_bound_event_v1(state, event)
        && (state.phase == SlotPhaseV1::Published || state.phase == SlotPhaseV1::Completed)
        && target_epoch_premise_matches_v1(state, premise)
        && !duplicate_target_use
        && state.native_reader_pins < native_reader_capacity_v1()
}

pub open spec fn retain_reader_v1(
    state: RegistryV1,
    event: EventOccurrenceV1,
    premise: TargetEpochUniquenessPremiseV1,
    lease_id: nat,
) -> (RegistryV1, NativeReaderLeaseV1) {
    (
        RegistryV1 {
            source: state.source,
            phase: state.phase,
            source_packet_id: state.source_packet_id,
            event_pins: state.event_pins,
            native_reader_pins: state.native_reader_pins + 1,
        },
        NativeReaderLeaseV1 {
            lease_id,
            event_id: event.event_id,
            source: event.source,
            source_packet_id: event.source_packet_id,
            dependent_epoch: premise.dependent_epoch,
        },
    )
}

pub open spec fn retain_reader_state_or_reject_v1(
    state: RegistryV1,
    event: EventOccurrenceV1,
    premise: TargetEpochUniquenessPremiseV1,
    duplicate_target_use: bool,
    lease_id: nat,
) -> RegistryV1 {
    if valid_registry_v1(state)
        && reader_admissible_v1(state, event, premise, duplicate_target_use)
        && lease_id > 0
    {
        retain_reader_v1(state, event, premise, lease_id).0
    } else {
        state
    }
}

pub open spec fn release_event_pin_v1(state: RegistryV1) -> RegistryV1
    recommends state.event_pins > 0,
{
    RegistryV1 {
        source: state.source,
        phase: state.phase,
        source_packet_id: state.source_packet_id,
        event_pins: (state.event_pins - 1) as nat,
        native_reader_pins: state.native_reader_pins,
    }
}

pub open spec fn release_reader_pin_v1(state: RegistryV1) -> RegistryV1
    recommends state.native_reader_pins > 0,
{
    RegistryV1 {
        source: state.source,
        phase: state.phase,
        source_packet_id: state.source_packet_id,
        event_pins: state.event_pins,
        native_reader_pins: (state.native_reader_pins - 1) as nat,
    }
}

pub open spec fn can_recycle_v1(state: RegistryV1) -> bool {
    state.phase == SlotPhaseV1::Completed
        && state.event_pins == 0
        && state.native_reader_pins == 0
}

pub open spec fn recycle_v1(state: RegistryV1) -> RegistryV1 {
    RegistryV1 {
        source: SourceOccurrenceV1 {
            session: state.source.session,
            source_epoch: state.source.source_epoch,
            queue_key: state.source.queue_key,
            signal_mapping: state.source.signal_mapping,
            slot: state.source.slot,
            slot_generation: state.source.slot_generation + 1,
            dispatch_generation: state.source.dispatch_generation,
        },
        phase: SlotPhaseV1::Available,
        source_packet_id: 0,
        event_pins: 0,
        native_reader_pins: 0,
    }
}

pub open spec fn recycle_or_reject_v1(state: RegistryV1) -> RegistryV1 {
    if valid_registry_v1(state) && can_recycle_v1(state) {
        recycle_v1(state)
    } else {
        state
    }
}

// Obligation 1: both active ledgers use the authenticated finite bound.
pub proof fn custody_capacities_are_exact_v1()
    ensures event_capacity_v1() == 8192, native_reader_capacity_v1() == 8192,
{}

// Obligation 2: event record increments only event pins and yields Unbound.
pub proof fn record_event_is_unbound_and_independent_v1(state: RegistryV1, event_id: nat)
    requires
        valid_registry_v1(state),
        state.phase == SlotPhaseV1::Bound,
        state.event_pins < event_capacity_v1(),
        event_id > 0,
    ensures {
        let result = record_event_v1(state, event_id);
        &&& result.0.event_pins == state.event_pins + 1
        &&& result.0.native_reader_pins == state.native_reader_pins
        &&& valid_unbound_event_v1(result.0, result.1)
        &&& valid_registry_v1(result.0)
    },
{}

// Obligation 3: event-capacity refusal is failure atomic.
pub proof fn event_capacity_failure_has_no_mutation_v1(state: RegistryV1, event_id: nat)
    requires state.event_pins == event_capacity_v1(),
    ensures record_event_state_or_reject_v1(state, event_id) == state,
{}

// Obligation 4: publication preserves both independent pin counts.
pub proof fn publication_preserves_pins_v1(state: RegistryV1, packet_id: nat)
    requires valid_registry_v1(state), state.phase == SlotPhaseV1::Bound, packet_id > 0,
    ensures {
        let published = publish_source_v1(state, packet_id);
        &&& published.event_pins == state.event_pins
        &&& published.native_reader_pins == state.native_reader_pins
        &&& valid_registry_v1(published)
    },
{}

// Obligation 5: one Unbound event binds to the exact published packet.
pub proof fn unbound_event_binds_exactly_once_v1(state: RegistryV1, event: EventOccurrenceV1)
    requires
        valid_registry_v1(state),
        state.phase == SlotPhaseV1::Published,
        valid_unbound_event_v1(state, event),
    ensures valid_bound_event_v1(state, bind_event_v1(state, event)),
{}

// Obligation 6: a Bound event is not eligible for a second bind.
pub proof fn second_bind_is_rejected_without_mutation_v1(
    state: RegistryV1,
    event: EventOccurrenceV1,
)
    requires event.binding == EventBindingV1::Bound,
    ensures bind_event_or_reject_v1(state, event) == event,
{}

// Obligation 7: completion does not discharge event or reader custody.
pub proof fn completion_preserves_both_pin_classes_v1(state: RegistryV1)
    requires valid_registry_v1(state), state.phase == SlotPhaseV1::Published,
    ensures {
        let completed = complete_source_v1(state);
        &&& completed.event_pins == state.event_pins
        &&& completed.native_reader_pins == state.native_reader_pins
        &&& valid_registry_v1(completed)
    },
{}

// Obligation 8: reader admission consumes the explicit uniqueness premise.
// This implication does not prove the caller's uniqueness assertion.
pub proof fn reader_admission_requires_contracted_target_uniqueness_v1(
    state: RegistryV1,
    event: EventOccurrenceV1,
    premise: TargetEpochUniquenessPremiseV1,
    duplicate_target_use: bool,
)
    requires reader_admissible_v1(state, event, premise, duplicate_target_use),
    ensures
        premise.uniqueness_contracted,
        premise.session == state.source.session,
        premise.dependent_epoch > state.source.source_epoch,
{}

// Obligation 9: accepted reader custody increments only reader pins.
pub proof fn reader_pins_are_independent_v1(
    state: RegistryV1,
    event: EventOccurrenceV1,
    premise: TargetEpochUniquenessPremiseV1,
    lease_id: nat,
)
    requires
        valid_registry_v1(state),
        reader_admissible_v1(state, event, premise, false),
        lease_id > 0,
    ensures {
        let result = retain_reader_v1(state, event, premise, lease_id);
        &&& result.0.event_pins == state.event_pins
        &&& result.0.native_reader_pins == state.native_reader_pins + 1
        &&& result.1.source == state.source
        &&& result.1.source_packet_id == state.source_packet_id
        &&& result.1.dependent_epoch == premise.dependent_epoch
        &&& valid_registry_v1(result.0)
    },
{}

// Obligation 10: reader-capacity refusal is failure atomic.
pub proof fn reader_capacity_failure_has_no_mutation_v1(
    state: RegistryV1,
    event: EventOccurrenceV1,
    premise: TargetEpochUniquenessPremiseV1,
    duplicate_target_use: bool,
    lease_id: nat,
)
    requires state.native_reader_pins == native_reader_capacity_v1(),
    ensures retain_reader_state_or_reject_v1(
        state, event, premise, duplicate_target_use, lease_id,
    ) == state,
{}

// Obligation 11: an event/source generation substitution is stale and inert.
pub proof fn stale_event_failure_has_no_mutation_v1(
    state: RegistryV1,
    event: EventOccurrenceV1,
)
    requires event.source != state.source,
    ensures retain_reader_state_or_reject_v1(
        state,
        event,
        TargetEpochUniquenessPremiseV1 {
            session: state.source.session,
            dependent_epoch: state.source.source_epoch + 1,
            uniqueness_contracted: true,
        },
        false,
        1,
    ) == state,
{}

// Obligation 12: cross-session target admission is inert.
pub proof fn cross_session_failure_has_no_mutation_v1(
    state: RegistryV1,
    premise: TargetEpochUniquenessPremiseV1,
)
    requires premise.session != state.source.session,
    ensures forall|event: EventOccurrenceV1, lease_id: nat|
        retain_reader_state_or_reject_v1(state, event, premise, false, lease_id) == state,
{}

// Obligation 13: a self-dependency is inert.
pub proof fn self_dependency_failure_has_no_mutation_v1(
    state: RegistryV1,
    premise: TargetEpochUniquenessPremiseV1,
)
    requires premise.dependent_epoch == state.source.source_epoch,
    ensures forall|event: EventOccurrenceV1, lease_id: nat|
        retain_reader_state_or_reject_v1(state, event, premise, false, lease_id) == state,
{}

// Obligation 14: a backwards epoch edge is rejected as cyclic and inert.
pub proof fn cycle_failure_has_no_mutation_v1(
    state: RegistryV1,
    premise: TargetEpochUniquenessPremiseV1,
)
    requires premise.dependent_epoch < state.source.source_epoch,
    ensures forall|event: EventOccurrenceV1, lease_id: nat|
        retain_reader_state_or_reject_v1(state, event, premise, false, lease_id) == state,
{}

// Obligation 15: duplicate target-use classification is inert.
pub proof fn duplicate_reader_failure_has_no_mutation_v1(
    state: RegistryV1,
    event: EventOccurrenceV1,
    premise: TargetEpochUniquenessPremiseV1,
    lease_id: nat,
)
    ensures retain_reader_state_or_reject_v1(state, event, premise, true, lease_id) == state,
{}

// Obligation 16: omission of the uniqueness premise is inert.
pub proof fn missing_uniqueness_premise_has_no_mutation_v1(
    state: RegistryV1,
    premise: TargetEpochUniquenessPremiseV1,
)
    requires !premise.uniqueness_contracted,
    ensures forall|event: EventOccurrenceV1, lease_id: nat|
        retain_reader_state_or_reject_v1(state, event, premise, false, lease_id) == state,
{}

// Obligation 17: event release leaves reader custody unchanged.
pub proof fn event_release_does_not_release_reader_v1(state: RegistryV1)
    requires valid_registry_v1(state), state.event_pins > 0,
    ensures {
        let released = release_event_pin_v1(state);
        &&& released.event_pins + 1 == state.event_pins
        &&& released.native_reader_pins == state.native_reader_pins
        &&& valid_registry_v1(released)
    },
{}

// Obligation 18: reader release leaves event custody unchanged.
pub proof fn reader_release_does_not_release_event_v1(state: RegistryV1)
    requires valid_registry_v1(state), state.native_reader_pins > 0,
    ensures {
        let released = release_reader_pin_v1(state);
        &&& released.native_reader_pins + 1 == state.native_reader_pins
        &&& released.event_pins == state.event_pins
        &&& valid_registry_v1(released)
    },
{}

// Obligation 19: any event pin blocks reset/recycle.
pub proof fn event_pin_blocks_recycle_v1(state: RegistryV1)
    requires state.event_pins > 0,
    ensures !can_recycle_v1(state), recycle_or_reject_v1(state) == state,
{}

// Obligation 20: any native-reader pin independently blocks reset/recycle.
pub proof fn reader_pin_blocks_recycle_v1(state: RegistryV1)
    requires state.native_reader_pins > 0,
    ensures !can_recycle_v1(state), recycle_or_reject_v1(state) == state,
{}

// Obligation 21: a recyclable completed occurrence advances generation once.
pub proof fn recycle_advances_only_slot_generation_v1(state: RegistryV1)
    requires valid_registry_v1(state), can_recycle_v1(state),
    ensures {
        let recycled = recycle_v1(state);
        &&& recycled.phase == SlotPhaseV1::Available
        &&& recycled.source.slot_generation == state.source.slot_generation + 1
        &&& recycled.source.session == state.source.session
        &&& recycled.source.source_epoch == state.source.source_epoch
        &&& recycled.source.queue_key == state.source.queue_key
        &&& recycled.source.signal_mapping == state.source.signal_mapping
        &&& recycled.source.slot == state.source.slot
        &&& recycled.source.dispatch_generation == state.source.dispatch_generation
        &&& recycled.source_packet_id == 0
        &&& recycled.event_pins == 0
        &&& recycled.native_reader_pins == 0
        &&& valid_registry_v1(recycled)
    },
{}

}
