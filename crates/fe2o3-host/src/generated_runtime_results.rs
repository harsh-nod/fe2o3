//! Charged data ownership, not invocation or completion authority.

use std::any::Any;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use fe2o3_artifacts::{MAX_ABI_FIELDS, RustScalarElementTypeV1};
use fe2o3_resource_accounting::{
    ResourceCreditAccountV1, ResourceCreditErrorV1, ResourceKindV1, ResourceReservationV1,
    ResourceVectorV1, RetainedResourceCreditsV1,
};
use fe2o3_runtime::Gfx942RuntimeBufferAccessV1;

use crate::generated_argument_plan::GeneratedDeviceScalarV1;
use crate::generated_runtime_arguments::GeneratedRuntimeArgumentErrorV1 as Error;

/// Shared result-peak admission. Clones share the same account, not fresh limits.
///
/// Each output reserves its encoded-plus-typed byte peak until typed storage is
/// disposed. Returned read-only buffers reserve their byte extent separately.
/// Owner metadata, allocator overhead, caller construction and aggregate process
/// limits are not byte-accounted here. This handle grants no execution authority.
#[derive(Clone)]
pub struct GeneratedRuntimeResultBudgetV1 {
    account: ResourceCreditAccountV1,
}

impl GeneratedRuntimeResultBudgetV1 {
    pub fn new(peak_bytes: u64, members: usize) -> Result<Self, ResourceCreditErrorV1> {
        Ok(Self {
            account: ResourceCreditAccountV1::new(
                ResourceVectorV1::ZERO.with(ResourceKindV1::ReplyBytes, peak_bytes),
                members,
            )?,
        })
    }

    pub fn usage(&self) -> GeneratedRuntimeResultUsageV1 {
        let usage = self.account.usage();
        GeneratedRuntimeResultUsageV1 {
            peak_capacity_bytes: usage.capacity.get(ResourceKindV1::ReplyBytes),
            reserved_peak_bytes: usage.used.get(ResourceKindV1::ReplyBytes),
            member_capacity: usage.record_capacity,
            unissued_members: usage.reserved_records,
            retained_members: usage.retained_records,
            quarantined_members: usage.quarantined_records,
            poisoned: usage.poisoned,
        }
    }
}

/// Inert account observations; reserved peak is not current physical residency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedRuntimeResultUsageV1 {
    pub peak_capacity_bytes: u64,
    pub reserved_peak_bytes: u64,
    pub member_capacity: usize,
    pub unissued_members: usize,
    pub retained_members: usize,
    pub quarantined_members: usize,
    pub poisoned: bool,
}

/// Move-only typed data retaining its full result-peak reservation until disposal.
/// A result carries no native resource or GPU-completion authority.
///
/// ```compile_fail
/// use fe2o3_host::ChargedTypedResultV1;
/// fn duplicate(result: ChargedTypedResultV1<u32>) { let other = result.clone(); }
/// ```
///
/// ```compile_fail
/// use fe2o3_host::ChargedTypedResultV1;
/// fn shed_charge(result: ChargedTypedResultV1<u32>) -> Box<[u32]> {
///     result.into_boxed_slice()
/// }
/// ```
#[must_use = "typed storage retains its result credit until disposal"]
pub struct ChargedTypedResultV1<T: GeneratedDeviceScalarV1> {
    values: Option<Box<[T]>>,
    credit: Option<RetainedResourceCreditsV1>,
}

impl<T: GeneratedDeviceScalarV1> ChargedTypedResultV1<T> {
    pub fn as_slice(&self) -> &[T] {
        self.values.as_deref().expect("owned typed result")
    }

    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    pub fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }
}

impl<T: GeneratedDeviceScalarV1> fmt::Debug for ChargedTypedResultV1<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChargedTypedResultV1")
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

impl<T: GeneratedDeviceScalarV1> Drop for ChargedTypedResultV1<T> {
    fn drop(&mut self) {
        drop(self.values.take());
        if let Some(credit) = self.credit.take() {
            // Failed refund transitions retain/quarantine their debit in the core.
            let _ = credit.release_after_disposal();
        }
    }
}

pub(crate) struct ResultReadyGateV1(AtomicBool);

impl ResultReadyGateV1 {
    fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    pub(crate) fn commit(&self) {
        self.0.store(true, Ordering::Release);
    }

    fn ready(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

enum OutputState<T: GeneratedDeviceScalarV1> {
    Unbound,
    Prepared {
        result: ChargedTypedResultV1<T>,
        gate: Arc<ResultReadyGateV1>,
    },
    Taken,
    Unavailable,
}

struct OutputSlot<T: GeneratedDeviceScalarV1> {
    elements: usize,
    state: Mutex<OutputState<T>>,
}

trait ErasedOutputSlot: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn scalar(&self) -> RustScalarElementTypeV1;
    fn elements(&self) -> usize;
    fn unbound(&self) -> Result<(), Error>;
    fn bound_to(&self, gate: &Arc<ResultReadyGateV1>) -> Result<(), Error>;
    fn decode(&self, bytes: &[u8], gate: &Arc<ResultReadyGateV1>) -> Result<(), Error>;
    fn abandon(&self);
}

impl<T: GeneratedDeviceScalarV1> ErasedOutputSlot for OutputSlot<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn scalar(&self) -> RustScalarElementTypeV1 {
        T::RUST_SCALAR_TYPE
    }

    fn elements(&self) -> usize {
        self.elements
    }

    fn unbound(&self) -> Result<(), Error> {
        if matches!(
            *self.state.lock().map_err(|_| Error::Custody)?,
            OutputState::Unbound
        ) {
            Ok(())
        } else {
            Err(Error::StaleOrAliasedOutput)
        }
    }

    fn bound_to(&self, expected: &Arc<ResultReadyGateV1>) -> Result<(), Error> {
        match &*self.state.lock().map_err(|_| Error::Custody)? {
            OutputState::Prepared { gate, .. } if Arc::ptr_eq(gate, expected) && !gate.ready() => {
                Ok(())
            }
            _ => Err(Error::StaleOrAliasedOutput),
        }
    }

    fn decode(&self, bytes: &[u8], expected: &Arc<ResultReadyGateV1>) -> Result<(), Error> {
        let mut state = self.state.lock().map_err(|_| Error::Custody)?;
        let OutputState::Prepared { result, gate } = &mut *state else {
            return Err(Error::StaleOrAliasedOutput);
        };
        if !Arc::ptr_eq(gate, expected)
            || gate.ready()
            || self.elements.checked_mul(size_of::<T>()) != Some(bytes.len())
        {
            return Err(Error::BindingMismatch);
        }
        let values = result.values.as_deref_mut().ok_or(Error::BindingMismatch)?;
        if values.len() != self.elements {
            return Err(Error::BindingMismatch);
        }
        for (value, encoded) in values.iter_mut().zip(bytes.chunks_exact(size_of::<T>())) {
            *value = T::decode_le_bytes_v1(encoded).ok_or(Error::BindingMismatch)?;
        }
        Ok(())
    }

    fn abandon(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if matches!(&*state, OutputState::Prepared { gate, .. } if gate.ready()) {
            return;
        }
        let discarded = std::mem::replace(&mut *state, OutputState::Unavailable);
        drop(state);
        drop(discarded);
    }
}

/// One observer of charged typed data. Dropping it does not cancel its producer.
/// Readiness means the private decoder committed all outputs, not that a GPU ran.
///
/// ```compile_fail
/// use fe2o3_host::GeneratedRuntimeChargedResultV1;
/// fn duplicate(result: GeneratedRuntimeChargedResultV1<u32>) { let other = result.clone(); }
/// ```
#[must_use]
pub struct GeneratedRuntimeChargedResultV1<T: GeneratedDeviceScalarV1> {
    slot: Arc<OutputSlot<T>>,
}

impl<T: GeneratedDeviceScalarV1> GeneratedRuntimeChargedResultV1<T> {
    /// Takes committed data without waiting for the slot mutex. `None` also
    /// covers transient contention; a future adapter must arrange its own wake/retry.
    pub fn try_take(&mut self) -> Result<Option<ChargedTypedResultV1<T>>, Error> {
        let mut state = match self.slot.state.try_lock() {
            Ok(state) => state,
            Err(std::sync::TryLockError::WouldBlock) => return Ok(None),
            Err(std::sync::TryLockError::Poisoned(_)) => return Err(Error::Custody),
        };
        match &*state {
            OutputState::Unbound => Ok(None),
            OutputState::Prepared { gate, .. } if !gate.ready() => Ok(None),
            OutputState::Prepared { .. } => {
                let OutputState::Prepared { result, .. } =
                    std::mem::replace(&mut *state, OutputState::Taken)
                else {
                    unreachable!("prepared result consumed under its lock")
                };
                Ok(Some(result))
            }
            OutputState::Taken | OutputState::Unavailable => Err(Error::OutputUnavailable),
        }
    }
}

pub(crate) struct ChargedOutputCustodyV1 {
    slot: Arc<dyn ErasedOutputSlot>,
}

impl ChargedOutputCustodyV1 {
    pub(crate) fn new<T: GeneratedDeviceScalarV1>(
        elements: usize,
    ) -> (Self, GeneratedRuntimeChargedResultV1<T>) {
        let slot = Arc::new(OutputSlot {
            elements,
            state: Mutex::new(OutputState::Unbound),
        });
        (
            Self { slot: slot.clone() },
            GeneratedRuntimeChargedResultV1 { slot },
        )
    }

    pub(crate) fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.slot, &other.slot)
    }

    pub(crate) fn bind_seed<T: GeneratedDeviceScalarV1>(
        &self,
        values: Box<[T]>,
        member: ResultMemberV1,
    ) -> Result<(), Error> {
        // Dispose caller storage before an unissued reservation is cancelled on any failure.
        let mut pending = PendingSeed {
            values: Some(values),
            reservation: Some(member.reservation),
        };
        let slot = self
            .slot
            .as_any()
            .downcast_ref::<OutputSlot<T>>()
            .ok_or(Error::BindingMismatch)?;
        if pending.values.as_ref().map(|values| values.len()) != Some(slot.elements) {
            return Err(Error::BindingMismatch);
        }
        let mut state = slot.state.lock().map_err(|_| Error::Custody)?;
        if !matches!(*state, OutputState::Unbound) {
            return Err(Error::StaleOrAliasedOutput);
        }
        let credit = pending
            .reservation
            .take()
            .expect("reserved output")
            .retain();
        *state = OutputState::Prepared {
            result: ChargedTypedResultV1 {
                values: pending.values.take(),
                credit: Some(credit),
            },
            gate: member.gate,
        };
        Ok(())
    }

    pub(crate) fn with_seed<T: GeneratedDeviceScalarV1, R>(
        &self,
        use_seed: impl FnOnce(&[T]) -> Result<R, Error>,
    ) -> Result<R, Error> {
        let slot = self
            .slot
            .as_any()
            .downcast_ref::<OutputSlot<T>>()
            .ok_or(Error::BindingMismatch)?;
        let state = slot.state.lock().map_err(|_| Error::Custody)?;
        match &*state {
            OutputState::Prepared { result, gate } if !gate.ready() => use_seed(result.as_slice()),
            _ => Err(Error::StaleOrAliasedOutput),
        }
    }

    pub(crate) fn bound_to(&self, gate: &Arc<ResultReadyGateV1>) -> Result<(), Error> {
        self.slot.bound_to(gate)
    }

    pub(crate) fn decode(&self, bytes: &[u8], gate: &Arc<ResultReadyGateV1>) -> Result<(), Error> {
        self.slot.decode(bytes, gate)
    }
}

impl Drop for ChargedOutputCustodyV1 {
    fn drop(&mut self) {
        self.slot.abandon();
    }
}

struct PendingSeed<T> {
    values: Option<Box<[T]>>,
    reservation: Option<ResourceReservationV1>,
}

impl<T> Drop for PendingSeed<T> {
    fn drop(&mut self) {
        drop(self.values.take());
        drop(self.reservation.take());
    }
}

pub(crate) struct ResultDescriptorV1 {
    bytes: usize,
    scalar: RustScalarElementTypeV1,
    access: Gfx942RuntimeBufferAccessV1,
    slot: Option<Arc<dyn ErasedOutputSlot>>,
}

impl ResultDescriptorV1 {
    pub(crate) fn new<T: GeneratedDeviceScalarV1>(
        elements: usize,
        access: Gfx942RuntimeBufferAccessV1,
        custody: Option<&ChargedOutputCustodyV1>,
    ) -> Result<Self, Error> {
        let bytes = elements
            .checked_mul(size_of::<T>())
            .ok_or(Error::ByteLength)?;
        if (access != Gfx942RuntimeBufferAccessV1::ReadOnly) != custody.is_some() {
            return Err(Error::BindingMismatch);
        }
        if let Some(custody) = custody {
            if custody.slot.scalar() != T::RUST_SCALAR_TYPE || custody.slot.elements() != elements {
                return Err(Error::BindingMismatch);
            }
            custody.slot.unbound()?;
        }
        Ok(Self {
            bytes,
            scalar: T::RUST_SCALAR_TYPE,
            access,
            slot: custody.map(|custody| custody.slot.clone()),
        })
    }

    fn matches(&self, other: &Self) -> bool {
        self.bytes == other.bytes
            && self.scalar == other.scalar
            && self.access == other.access
            && match (&self.slot, &other.slot) {
                (None, None) => true,
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                _ => false,
            }
    }

    fn charge(&self) -> Result<ResourceVectorV1, Error> {
        let bytes = u64::try_from(self.bytes).map_err(|_| Error::ByteLength)?;
        fe2o3_runtime_model::r73_generated_result_peak_v1(bytes, self.slot.is_some())
            .ok_or(Error::ByteLength)
    }
}

pub(crate) struct ResultPreflightV1 {
    descriptors: Vec<ResultDescriptorV1>,
}

impl ResultPreflightV1 {
    pub(crate) fn new() -> Result<Self, Error> {
        let mut descriptors = Vec::new();
        descriptors
            .try_reserve_exact(MAX_ABI_FIELDS)
            .map_err(|_| Error::Allocation)?;
        Ok(Self { descriptors })
    }

    pub(crate) fn push(&mut self, descriptor: ResultDescriptorV1) -> Result<(), Error> {
        if self.descriptors.len() == MAX_ABI_FIELDS {
            return Err(Error::PayloadLimit);
        }
        descriptor.charge()?;
        if let Some(slot) = &descriptor.slot
            && self.descriptors.iter().any(|other| {
                other
                    .slot
                    .as_ref()
                    .is_some_and(|other| Arc::ptr_eq(other, slot))
            })
        {
            return Err(Error::StaleOrAliasedOutput);
        }
        self.descriptors.push(descriptor);
        Ok(())
    }

    pub(crate) fn reserve(
        self,
        budget: &GeneratedRuntimeResultBudgetV1,
    ) -> Result<ResultBindingBudgetV1, Error> {
        let mut charges = [ResourceVectorV1::ZERO; MAX_ABI_FIELDS];
        for (charge, descriptor) in charges.iter_mut().zip(&self.descriptors) {
            if let Some(slot) = &descriptor.slot {
                slot.unbound()?;
            }
            *charge = descriptor.charge()?;
        }
        let gate = Arc::new(ResultReadyGateV1::new());
        let reservations = if self.descriptors.is_empty() {
            Vec::new()
        } else {
            budget
                .account
                .reserve_batch(&charges[..self.descriptors.len()])
                .map_err(Error::ResultCredit)?
                .into_vec()
        };
        Ok(ResultBindingBudgetV1 {
            descriptors: self.descriptors.into_iter(),
            reservations: reservations.into_iter(),
            gate,
        })
    }
}

pub(crate) struct ResultBindingBudgetV1 {
    descriptors: std::vec::IntoIter<ResultDescriptorV1>,
    reservations: std::vec::IntoIter<ResourceReservationV1>,
    pub(crate) gate: Arc<ResultReadyGateV1>,
}

impl ResultBindingBudgetV1 {
    pub(crate) fn take(
        &mut self,
        descriptor: &ResultDescriptorV1,
    ) -> Result<ResultMemberV1, Error> {
        if !self
            .descriptors
            .as_slice()
            .first()
            .is_some_and(|expected| expected.matches(descriptor))
        {
            return Err(Error::BindingMismatch);
        }
        let reservation = self.reservations.next().ok_or(Error::BindingMismatch)?;
        self.descriptors.next();
        Ok(ResultMemberV1 {
            reservation,
            gate: self.gate.clone(),
        })
    }

    pub(crate) fn complete(&self) -> bool {
        self.descriptors.len() == 0 && self.reservations.len() == 0
    }
}

pub(crate) struct ResultMemberV1 {
    reservation: ResourceReservationV1,
    gate: Arc<ResultReadyGateV1>,
}

pub(crate) struct ReadResultCreditV1 {
    credit: Option<RetainedResourceCreditsV1>,
    gate: Arc<ResultReadyGateV1>,
}

impl ReadResultCreditV1 {
    pub(crate) fn bound_to(&self, gate: &Arc<ResultReadyGateV1>) -> bool {
        self.credit.is_some() && Arc::ptr_eq(&self.gate, gate) && !gate.ready()
    }
}

impl ResultMemberV1 {
    pub(crate) fn retain_read(self) -> ReadResultCreditV1 {
        ReadResultCreditV1 {
            credit: Some(self.reservation.retain()),
            gate: self.gate,
        }
    }
}

impl Drop for ReadResultCreditV1 {
    fn drop(&mut self) {
        if let Some(credit) = self.credit.take() {
            let _ = credit.release_after_disposal();
        }
    }
}
