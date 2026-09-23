//! Real allocator-transition prefix owned by the same accepted-record collector.
use super::*;
use fe2o3_kir_sim::{
    SimulationAllocationDescriptorV1, SimulationAllocationScopeV1,
    SimulationAllocationTransitionKindV1, SimulationAllocationTransitionV1 as Transition,
    SimulationAllocationWatermarkV1 as Watermark,
};

pub(super) struct AllocationRetention {
    transitions: Vec<Transition>,
    records: Vec<Watermark>,
    limits: Option<RuntimeAllocationCaptureLimitsV1>,
    transition_limit: usize,
    record_limit: usize,
    coverage: Coverage,
    last_allocation: u64,
    last_storage_slot: u64,
    validation_remaining: usize,
}
pub(super) fn minimum_metadata_bytes() -> usize {
    size_of::<AllocationRetention>() + size_of::<Transition>() + size_of::<Watermark>()
}

#[derive(Clone, Copy, Debug)]
enum ValidationError {
    Invalid,
    WorkLimit,
}
fn valid_scope(descriptor: SimulationAllocationDescriptorV1) -> bool {
    use fe2o3_kernel_ir::AddressSpace;
    matches!(
        (descriptor.address_space(), descriptor.scope()),
        (
            AddressSpace::Private,
            SimulationAllocationScopeV1::Invocation(_)
        ) | (
            AddressSpace::Workgroup,
            SimulationAllocationScopeV1::Workgroup { .. }
        ) | (
            AddressSpace::Global | AddressSpace::Constant | AddressSpace::Generic,
            SimulationAllocationScopeV1::Dispatch
        )
    )
}

pub(super) struct PendingAllocation {
    index: usize,
    result: Result<Watermark, Coverage>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAllocationMissingV1 {
    NoSuchRecord,
    NoCurrentRecord,
    Disabled,
    PrefixTruncated(Cutoff),
    RuntimeUnavailable(Watermark),
    InvalidJoin,
    NotLive,
    WorkLimit,
}
#[derive(Clone, Copy, Debug)]
pub struct RuntimeAllocationObservationV1<'a> {
    record: &'a SimulationDebugRecordV1,
    transitions: &'a [Transition],
    through_sequence: u64,
}
impl<'a> RuntimeAllocationObservationV1<'a> {
    pub fn record(self) -> &'a SimulationDebugRecordV1 {
        self.record
    }
    pub fn transitions(self) -> &'a [Transition] {
        self.transitions
    }
    pub const fn through_sequence(self) -> u64 {
        self.through_sequence
    }
    pub const fn watermark(self) -> Watermark {
        Watermark::Available {
            through_sequence: self.through_sequence,
        }
    }
    /// Resolve only a currently live semantic allocation at this exact watermark.
    /// Earlier incarnations remain historical; no slot-only fallback is permitted.
    pub fn descriptor(
        self,
        allocation: u64,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<SimulationAllocationDescriptorV1, RuntimeAllocationMissingV1> {
        for transition in self.transitions.iter().rev() {
            work.charge(1)
                .map_err(|_| RuntimeAllocationMissingV1::WorkLimit)?;
            if transition.descriptor().identity().allocation() != allocation {
                continue;
            }
            return match transition.kind() {
                SimulationAllocationTransitionKindV1::Release => {
                    Err(RuntimeAllocationMissingV1::NotLive)
                }
                _ => Ok(transition.descriptor()),
            };
        }
        Err(RuntimeAllocationMissingV1::NotLive)
    }
}

impl AllocationRetention {
    pub(super) fn new(limits: Option<RuntimeAllocationCaptureLimitsV1>) -> Self {
        let mut retained = Self {
            transitions: Vec::new(),
            records: Vec::new(),
            limits,
            transition_limit: 0,
            record_limit: 0,
            coverage: Coverage::Disabled,
            last_allocation: 0,
            last_storage_slot: 0,
            validation_remaining: limits.map_or(0, |value| value.validation_work),
        };
        let Some(limits) = limits else {
            return retained;
        };
        let available = limits.bytes - size_of::<Self>();
        retained.record_limit = limits
            .records
            .min((available - size_of::<Transition>()) / size_of::<Watermark>());
        let remaining = available - retained.record_limit * size_of::<Watermark>();
        retained.transition_limit = limits.transitions.min(remaining / size_of::<Transition>());
        retained.coverage = Coverage::Complete;
        if retained
            .records
            .try_reserve_exact(retained.record_limit)
            .is_err()
            || retained
                .transitions
                .try_reserve_exact(retained.transition_limit)
                .is_err()
        {
            retained.records = Vec::new();
            retained.transitions = Vec::new();
            retained.coverage = Coverage::PrefixTruncated(Cutoff::AllocationFailure);
        } else if retained
            .checked_bytes()
            .is_none_or(|bytes| bytes > limits.bytes)
            || retained.records.capacity() < retained.record_limit
            || retained.transitions.capacity() < retained.transition_limit
        {
            retained.records = Vec::new();
            retained.transitions = Vec::new();
            retained.coverage = Coverage::PrefixTruncated(Cutoff::InvalidCapacity);
        }
        retained
    }
    fn checked_bytes(&self) -> Option<usize> {
        self.records
            .capacity()
            .checked_mul(size_of::<Watermark>())?
            .checked_add(
                self.transitions
                    .capacity()
                    .checked_mul(size_of::<Transition>())?,
            )?
            .checked_add(size_of::<Self>())
    }
    pub(super) fn usage(&self) -> RuntimeAllocationUsageV1 {
        RuntimeAllocationUsageV1 {
            retained_records: self.records.len(),
            retained_transitions: self.transitions.len(),
            record_capacity: self.records.capacity(),
            transition_capacity: self.transitions.capacity(),
            metadata_bytes: self
                .checked_bytes()
                .expect("validated immutable capacities"),
            validation_work_limit: self.limits.map_or(0, |value| value.validation_work),
            validation_work_used: self.limits.map_or(0, |value| value.validation_work)
                - self.validation_remaining,
        }
    }
    pub(super) fn coverage(&self) -> Coverage {
        self.coverage
    }
    pub(super) fn enabled(&self) -> bool {
        self.limits.is_some()
    }

    fn charge_validation(&mut self) -> Result<(), ValidationError> {
        self.validation_remaining = self
            .validation_remaining
            .checked_sub(1)
            .ok_or(ValidationError::WorkLimit)?;
        Ok(())
    }
    fn latest_matching(
        &mut self,
        predicate: impl Fn(Transition) -> bool,
    ) -> Result<Option<Transition>, ValidationError> {
        for index in (0..self.transitions.len()).rev() {
            self.charge_validation()?;
            let row = self.transitions[index];
            if predicate(row) {
                return Ok(Some(row));
            }
        }
        Ok(None)
    }
    fn validate_next(&mut self, transition: Transition) -> Result<(u64, u64), ValidationError> {
        self.charge_validation()?;
        let descriptor = transition.descriptor();
        let identity = descriptor.identity();
        if u64::try_from(self.transitions.len())
            .ok()
            .and_then(|value| value.checked_add(1))
            != Some(transition.sequence())
            || identity.allocation() == 0
            || identity.storage_slot() == 0
            || identity.generation() == 0
            || !descriptor.alignment().is_power_of_two()
            || !valid_scope(descriptor)
        {
            return Err(ValidationError::Invalid);
        }
        match transition.kind() {
            SimulationAllocationTransitionKindV1::Release => {
                if matches!(descriptor.scope(), SimulationAllocationScopeV1::Dispatch) {
                    return Err(ValidationError::Invalid);
                }
                let previous = self
                    .latest_matching(|row| {
                        row.descriptor().identity().allocation() == identity.allocation()
                    })?
                    .ok_or(ValidationError::Invalid)?;
                if previous.descriptor() != descriptor
                    || matches!(
                        previous.kind(),
                        SimulationAllocationTransitionKindV1::Release
                    )
                {
                    return Err(ValidationError::Invalid);
                }
                Ok((self.last_allocation, self.last_storage_slot))
            }
            kind => {
                if identity.allocation() <= self.last_allocation {
                    return Err(ValidationError::Invalid);
                }
                let predecessor = match kind {
                    SimulationAllocationTransitionKindV1::Preexisting => {
                        if !matches!(descriptor.scope(), SimulationAllocationScopeV1::Dispatch)
                            || descriptor.creation_site().is_some()
                        {
                            return Err(ValidationError::Invalid);
                        }
                        None
                    }
                    SimulationAllocationTransitionKindV1::Create {
                        previous_allocation,
                    } => previous_allocation,
                    SimulationAllocationTransitionKindV1::Release => unreachable!(),
                };
                match predecessor {
                    None => {
                        if identity.generation() != 1
                            || identity.storage_slot() <= self.last_storage_slot
                        {
                            return Err(ValidationError::Invalid);
                        }
                        Ok((identity.allocation(), identity.storage_slot()))
                    }
                    Some(allocation) => {
                        if matches!(descriptor.scope(), SimulationAllocationScopeV1::Dispatch) {
                            return Err(ValidationError::Invalid);
                        }
                        let previous = self
                            .latest_matching(|row| {
                                row.descriptor().identity().storage_slot()
                                    == identity.storage_slot()
                            })?
                            .ok_or(ValidationError::Invalid)?;
                        let old = previous.descriptor();
                        if !matches!(
                            previous.kind(),
                            SimulationAllocationTransitionKindV1::Release
                        ) || old.identity().allocation() != allocation
                            || old.identity().generation().checked_add(1)
                                != Some(identity.generation())
                            || old.address_space() != descriptor.address_space()
                            || old.access() != descriptor.access()
                            || old.alignment() != descriptor.alignment()
                            || old.byte_len() != descriptor.byte_len()
                        {
                            return Err(ValidationError::Invalid);
                        }
                        Ok((identity.allocation(), self.last_storage_slot))
                    }
                }
            }
        }
    }
    pub(super) fn receive(&mut self, transition: Transition) -> Control {
        if self.coverage != Coverage::Complete {
            return Control::DropAndStop;
        }
        if self.transitions.len() == self.transition_limit {
            let limits = self.limits.expect("enabled");
            self.coverage =
                Coverage::PrefixTruncated(if self.transition_limit == limits.transitions {
                    Cutoff::TransitionLimit
                } else {
                    Cutoff::ByteLimit
                });
            return Control::DropAndStop;
        }
        let (last_allocation, last_storage_slot) = match self.validate_next(transition) {
            Ok(value) => value,
            Err(ValidationError::Invalid) => {
                self.coverage = Coverage::InvalidJoin;
                return Control::DropAndStop;
            }
            Err(ValidationError::WorkLimit) => {
                self.coverage = Coverage::PrefixTruncated(Cutoff::ValidationWorkLimit);
                return Control::DropAndStop;
            }
        };
        self.transitions.push(transition);
        self.last_allocation = last_allocation;
        self.last_storage_slot = last_storage_slot;
        Control::Continue
    }
    pub(super) fn prepare(
        &self,
        index: usize,
        record: &SimulationDebugRecordV1,
        watermark: Watermark,
    ) -> PendingAllocation {
        let result = if self.coverage != Coverage::Complete {
            Err(self.coverage)
        } else if index != self.records.len() || u64::try_from(index) != Ok(record.ordinal) {
            Err(Coverage::InvalidJoin)
        } else if self.records.len() == self.record_limit {
            Err(Coverage::PrefixTruncated(
                if self.record_limit == self.limits.expect("enabled").records {
                    Cutoff::RowLimit
                } else {
                    Cutoff::ByteLimit
                },
            ))
        } else if matches!(watermark, Watermark::Available { through_sequence }
            if u64::try_from(self.transitions.len()) != Ok(through_sequence))
        {
            Err(Coverage::InvalidJoin)
        } else {
            Ok(watermark)
        };
        PendingAllocation { index, result }
    }
    pub(super) fn commit(
        &mut self,
        pending: PendingAllocation,
        control: Control,
        accepted_len: usize,
    ) {
        let accepted = matches!(control, Control::Continue | Control::Stop);
        if pending.index.checked_add(usize::from(accepted)) != Some(accepted_len) {
            self.coverage = Coverage::InvalidJoin;
            return;
        }
        if !accepted || self.coverage != Coverage::Complete {
            return;
        }
        match pending.result {
            Err(coverage) => self.coverage = coverage,
            Ok(row) => {
                if pending.index != self.records.len() || self.records.len() >= self.record_limit {
                    self.coverage = Coverage::InvalidJoin;
                    return;
                }
                self.records.push(row);
            }
        }
    }
    pub(super) fn validate_seal(&mut self, len: usize) {
        if self.records.len() > len
            || (self.coverage == Coverage::Complete && self.records.len() != len)
        {
            self.coverage = Coverage::InvalidJoin;
        }
    }
    pub(super) fn at<'a>(
        &'a self,
        transcript: &'a DebugTranscriptV1,
        index: usize,
    ) -> Result<RuntimeAllocationObservationV1<'a>, RuntimeAllocationMissingV1> {
        use RuntimeAllocationMissingV1 as Error;
        let record = transcript.records().get(index).ok_or(Error::NoSuchRecord)?;
        if self.coverage == Coverage::InvalidJoin {
            return Err(Error::InvalidJoin);
        }
        let Some(watermark) = self.records.get(index).copied() else {
            return Err(match self.coverage {
                Coverage::Disabled => Error::Disabled,
                Coverage::PrefixTruncated(reason) => Error::PrefixTruncated(reason),
                _ => Error::InvalidJoin,
            });
        };
        let Watermark::Available { through_sequence } = watermark else {
            return Err(Error::RuntimeUnavailable(watermark));
        };
        let end = usize::try_from(through_sequence).map_err(|_| Error::InvalidJoin)?;
        let transitions = self.transitions.get(..end).ok_or(Error::InvalidJoin)?;
        Ok(RuntimeAllocationObservationV1 {
            record,
            transitions,
            through_sequence,
        })
    }
}
