//! Executable finite R42 model for compute-event signal custody.
//!
//! The model owns one addressless completion-slot occurrence and bounded event
//! and native-reader ledgers. A recorded event is initially unbound, binds to
//! one nonzero packet ID exactly once, and pins the source independently from
//! every native-reader lease. Completion does not release either pin. Recycle
//! requires completed source state, empty ledgers, and both pin counts zero.
//!
//! Target acceptance-epoch uniqueness is a caller-constructed contracted
//! premise. This model checks that the premise names the same session and use,
//! but does not prove uniqueness or bind the epoch to a target publication.
//! All identities are mathematical inputs. There is no production-Rust, KFD,
//! HSA, HIP, packet, atomic, allocator, panic, native, hardware, progress,
//! parity, or performance refinement claim.

use alloc::vec::Vec;

pub const R42_EVENT_CAPACITY_V1: usize = 8192;
pub const R42_NATIVE_READER_CAPACITY_V1: usize = 8192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R42CompletionSlotPhaseV1 {
    Bound,
    Published,
    Completed,
    Available,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R42EventBindingStateV1 {
    Unbound,
    Bound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R42SignalOccurrenceIdentityV1 {
    pub session_occurrence: u64,
    pub source_acceptance_epoch: u64,
    pub queue_key: u64,
    pub signal_mapping: u64,
    pub slot: u16,
    pub slot_generation: u64,
    pub dispatch_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R42TargetEpochUniquenessPremiseV1 {
    pub session_occurrence: u64,
    pub dependent_acceptance_epoch: u64,
    pub uniqueness_contracted: bool,
}

impl R42TargetEpochUniquenessPremiseV1 {
    pub const fn new_model_only(
        session_occurrence: u64,
        dependent_acceptance_epoch: u64,
        uniqueness_contracted: bool,
    ) -> Self {
        Self {
            session_occurrence,
            dependent_acceptance_epoch,
            uniqueness_contracted,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct R42EventRecordV1 {
    event_id: u64,
    source: R42SignalOccurrenceIdentityV1,
    packet_id: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct R42ReaderRecordV1 {
    lease_id: u64,
    event_id: u64,
    source: R42SignalOccurrenceIdentityV1,
    source_packet_id: u64,
    dependent_acceptance_epoch: u64,
}

/// Move-only model custody for one addressless event occurrence.
#[derive(Debug, Eq, PartialEq)]
pub struct R42ComputeEventOccurrenceV1 {
    record: R42EventRecordV1,
}

impl R42ComputeEventOccurrenceV1 {
    pub const fn binding_state_model_only(&self) -> R42EventBindingStateV1 {
        if self.record.packet_id.is_some() {
            R42EventBindingStateV1::Bound
        } else {
            R42EventBindingStateV1::Unbound
        }
    }

    pub fn substitute_slot_generation_model_only(mut self, generation: u64) -> Self {
        self.record.source.slot_generation = generation;
        self
    }
}

/// Move-only model custody for one exact native-reader use.
#[derive(Debug, Eq, PartialEq)]
pub struct R42NativeDependencyReaderLeaseV1 {
    record: R42ReaderRecordV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R42SignalCustodySnapshotV1 {
    pub source: R42SignalOccurrenceIdentityV1,
    pub phase: R42CompletionSlotPhaseV1,
    pub packet_id: Option<u64>,
    pub event_pins: u16,
    pub native_reader_pins: u16,
    pub live_event_ids: Vec<u64>,
    pub live_reader_uses: Vec<(u64, u64)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R42SignalCustodyErrorV1 {
    InvalidIdentity,
    WrongPhase,
    EventCapacity,
    ReaderCapacity,
    EventIdentityExhausted,
    ReaderIdentityExhausted,
    PinCountExhausted,
    EventAlreadyBound,
    EventNotPublished,
    StaleEvent,
    StaleReader,
    CrossSession,
    SelfDependency,
    DependencyCycle,
    DuplicateDependency,
    MissingTargetEpochUniquenessPremise,
    SignalPinned,
    SignalGenerationExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R42RecycleObservationV1 {
    pub prior_generation: u64,
    pub next_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R42SignalCustodyRegistryV1 {
    source: R42SignalOccurrenceIdentityV1,
    phase: R42CompletionSlotPhaseV1,
    packet_id: Option<u64>,
    event_pins: u16,
    native_reader_pins: u16,
    next_event_id: u64,
    next_reader_id: u64,
    events: Vec<R42EventRecordV1>,
    readers: Vec<R42ReaderRecordV1>,
}

impl R42SignalCustodyRegistryV1 {
    pub fn new_model_only(
        source: R42SignalOccurrenceIdentityV1,
    ) -> Result<Self, R42SignalCustodyErrorV1> {
        if source.session_occurrence == 0
            || source.source_acceptance_epoch == 0
            || source.queue_key == 0
            || source.signal_mapping == 0
            || source.slot_generation == 0
            || source.dispatch_generation == 0
        {
            return Err(R42SignalCustodyErrorV1::InvalidIdentity);
        }
        Ok(Self {
            source,
            phase: R42CompletionSlotPhaseV1::Bound,
            packet_id: None,
            event_pins: 0,
            native_reader_pins: 0,
            next_event_id: 1,
            next_reader_id: 1,
            events: Vec::new(),
            readers: Vec::new(),
        })
    }

    pub fn snapshot_model_only(&self) -> R42SignalCustodySnapshotV1 {
        R42SignalCustodySnapshotV1 {
            source: self.source,
            phase: self.phase,
            packet_id: self.packet_id,
            event_pins: self.event_pins,
            native_reader_pins: self.native_reader_pins,
            live_event_ids: self.events.iter().map(|event| event.event_id).collect(),
            live_reader_uses: self
                .readers
                .iter()
                .map(|reader| (reader.event_id, reader.dependent_acceptance_epoch))
                .collect(),
        }
    }

    pub fn record_event_model_only(
        &mut self,
    ) -> Result<R42ComputeEventOccurrenceV1, R42SignalCustodyErrorV1> {
        if self.phase != R42CompletionSlotPhaseV1::Bound {
            return Err(R42SignalCustodyErrorV1::WrongPhase);
        }
        if self.events.len() >= R42_EVENT_CAPACITY_V1 {
            return Err(R42SignalCustodyErrorV1::EventCapacity);
        }
        let next_event_id = self
            .next_event_id
            .checked_add(1)
            .ok_or(R42SignalCustodyErrorV1::EventIdentityExhausted)?;
        let next_pins = self
            .event_pins
            .checked_add(1)
            .ok_or(R42SignalCustodyErrorV1::PinCountExhausted)?;
        let record = R42EventRecordV1 {
            event_id: self.next_event_id,
            source: self.source,
            packet_id: None,
        };
        self.events.push(record);
        self.next_event_id = next_event_id;
        self.event_pins = next_pins;
        Ok(R42ComputeEventOccurrenceV1 { record })
    }

    pub fn publish_source_model_only(
        &mut self,
        packet_id: u64,
    ) -> Result<(), R42SignalCustodyErrorV1> {
        if self.phase != R42CompletionSlotPhaseV1::Bound || packet_id == 0 {
            return Err(R42SignalCustodyErrorV1::WrongPhase);
        }
        self.phase = R42CompletionSlotPhaseV1::Published;
        self.packet_id = Some(packet_id);
        Ok(())
    }

    pub fn bind_event_model_only(
        &mut self,
        mut event: R42ComputeEventOccurrenceV1,
    ) -> Result<R42ComputeEventOccurrenceV1, (R42SignalCustodyErrorV1, R42ComputeEventOccurrenceV1)>
    {
        let packet_id = match self.packet_id {
            Some(packet_id) if self.phase == R42CompletionSlotPhaseV1::Published => packet_id,
            _ => return Err((R42SignalCustodyErrorV1::EventNotPublished, event)),
        };
        if event.record.packet_id.is_some() {
            return Err((R42SignalCustodyErrorV1::EventAlreadyBound, event));
        }
        let Some(index) = self
            .events
            .iter()
            .position(|record| *record == event.record)
        else {
            return Err((R42SignalCustodyErrorV1::StaleEvent, event));
        };
        event.record.packet_id = Some(packet_id);
        self.events[index] = event.record;
        Ok(event)
    }

    pub fn complete_source_model_only(&mut self) -> Result<(), R42SignalCustodyErrorV1> {
        if self.phase != R42CompletionSlotPhaseV1::Published {
            return Err(R42SignalCustodyErrorV1::WrongPhase);
        }
        self.phase = R42CompletionSlotPhaseV1::Completed;
        Ok(())
    }

    pub fn retain_reader_model_only(
        &mut self,
        event: R42ComputeEventOccurrenceV1,
        premise: R42TargetEpochUniquenessPremiseV1,
    ) -> Result<
        (
            R42ComputeEventOccurrenceV1,
            R42NativeDependencyReaderLeaseV1,
        ),
        (R42SignalCustodyErrorV1, R42ComputeEventOccurrenceV1),
    > {
        let Some(source_packet_id) = event.record.packet_id else {
            return Err((R42SignalCustodyErrorV1::EventNotPublished, event));
        };
        if !matches!(
            self.phase,
            R42CompletionSlotPhaseV1::Published | R42CompletionSlotPhaseV1::Completed
        ) || self.packet_id != Some(source_packet_id)
            || self.events.iter().all(|record| *record != event.record)
        {
            return Err((R42SignalCustodyErrorV1::StaleEvent, event));
        }
        if premise.session_occurrence != self.source.session_occurrence {
            return Err((R42SignalCustodyErrorV1::CrossSession, event));
        }
        if !premise.uniqueness_contracted {
            return Err((
                R42SignalCustodyErrorV1::MissingTargetEpochUniquenessPremise,
                event,
            ));
        }
        if premise.dependent_acceptance_epoch == self.source.source_acceptance_epoch {
            return Err((R42SignalCustodyErrorV1::SelfDependency, event));
        }
        if premise.dependent_acceptance_epoch < self.source.source_acceptance_epoch {
            return Err((R42SignalCustodyErrorV1::DependencyCycle, event));
        }
        if self.readers.iter().any(|reader| {
            reader.event_id == event.record.event_id
                && reader.dependent_acceptance_epoch == premise.dependent_acceptance_epoch
        }) {
            return Err((R42SignalCustodyErrorV1::DuplicateDependency, event));
        }
        if self.readers.len() >= R42_NATIVE_READER_CAPACITY_V1 {
            return Err((R42SignalCustodyErrorV1::ReaderCapacity, event));
        }
        let next_reader_id = match self.next_reader_id.checked_add(1) {
            Some(next) => next,
            None => {
                return Err((R42SignalCustodyErrorV1::ReaderIdentityExhausted, event));
            }
        };
        let next_pins = match self.native_reader_pins.checked_add(1) {
            Some(next) => next,
            None => return Err((R42SignalCustodyErrorV1::PinCountExhausted, event)),
        };
        let record = R42ReaderRecordV1 {
            lease_id: self.next_reader_id,
            event_id: event.record.event_id,
            source: event.record.source,
            source_packet_id,
            dependent_acceptance_epoch: premise.dependent_acceptance_epoch,
        };
        self.readers.push(record);
        self.next_reader_id = next_reader_id;
        self.native_reader_pins = next_pins;
        Ok((event, R42NativeDependencyReaderLeaseV1 { record }))
    }

    pub fn release_event_model_only(
        &mut self,
        event: R42ComputeEventOccurrenceV1,
    ) -> Result<(), (R42SignalCustodyErrorV1, R42ComputeEventOccurrenceV1)> {
        let Some(index) = self
            .events
            .iter()
            .position(|record| *record == event.record)
        else {
            return Err((R42SignalCustodyErrorV1::StaleEvent, event));
        };
        let Some(next_pins) = self.event_pins.checked_sub(1) else {
            return Err((R42SignalCustodyErrorV1::StaleEvent, event));
        };
        self.events.remove(index);
        self.event_pins = next_pins;
        Ok(())
    }

    pub fn release_reader_model_only(
        &mut self,
        reader: R42NativeDependencyReaderLeaseV1,
    ) -> Result<(), (R42SignalCustodyErrorV1, R42NativeDependencyReaderLeaseV1)> {
        let Some(index) = self
            .readers
            .iter()
            .position(|record| *record == reader.record)
        else {
            return Err((R42SignalCustodyErrorV1::StaleReader, reader));
        };
        let Some(next_pins) = self.native_reader_pins.checked_sub(1) else {
            return Err((R42SignalCustodyErrorV1::StaleReader, reader));
        };
        self.readers.remove(index);
        self.native_reader_pins = next_pins;
        Ok(())
    }

    pub fn recycle_model_only(
        &mut self,
    ) -> Result<R42RecycleObservationV1, R42SignalCustodyErrorV1> {
        if self.phase != R42CompletionSlotPhaseV1::Completed {
            return Err(R42SignalCustodyErrorV1::WrongPhase);
        }
        if self.event_pins != 0
            || self.native_reader_pins != 0
            || !self.events.is_empty()
            || !self.readers.is_empty()
        {
            return Err(R42SignalCustodyErrorV1::SignalPinned);
        }
        let next_generation = self
            .source
            .slot_generation
            .checked_add(1)
            .ok_or(R42SignalCustodyErrorV1::SignalGenerationExhausted)?;
        let prior_generation = self.source.slot_generation;
        self.source.slot_generation = next_generation;
        self.phase = R42CompletionSlotPhaseV1::Available;
        self.packet_id = None;
        Ok(R42RecycleObservationV1 {
            prior_generation,
            next_generation,
        })
    }
}
