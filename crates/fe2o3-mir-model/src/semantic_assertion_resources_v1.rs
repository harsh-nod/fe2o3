//! Shared local limits and caller-owned live reservations.
use super::*;

pub const MAX_SEMANTIC_ASSERTION_WORK_V1: usize = 1_024 * (1_024 + 2_048);
pub const MAX_SEMANTIC_ASSERTION_BLOCKS_V1: usize = 1_024;
pub const MAX_SEMANTIC_ASSERTION_EDGES_V1: usize = 2_048;
pub const MAX_SEMANTIC_ASSERTION_CACHE_ENTRIES_V1: usize = 65_536 * 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticAssertionLimitsV1 {
    work_units: usize,
    storage_bytes: usize,
}
impl SemanticAssertionLimitsV1 {
    pub const fn new(work_units: usize, storage_bytes: usize) -> Self {
        Self {
            work_units,
            storage_bytes,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticAssertionErrorV1 {
    LogicalVisitLimit { actual: usize, limit: usize },
    InvalidModel(&'static str),
    Unsupported(&'static str),
    Incomplete(&'static str),
    WorkLimit { actual: usize, limit: usize },
    StorageLimit { actual: usize, limit: usize },
    Arithmetic,
    Allocation,
    Accounting,
}
impl fmt::Display for SemanticAssertionErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "semantic assertion analysis: {self:?}")
    }
}
impl std::error::Error for SemanticAssertionErrorV1 {}

/// Calls precede work and requested payload allocation. A scoped caller keeps
/// all reservations until results, the analysis and partial backing are dropped,
/// including failures and unwinding. This interface never refunds caller credit.
pub trait SemanticAssertionMeterV1 {
    type Error;
    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error>;
    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Self::Error>;

    /// Historical logical visits are part of the same total-work ledger. The
    /// separate hook permits a legacy diagnostic counter, not a second grant.
    fn charge_legacy_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.charge_work(amount)
    }
    fn charge_legacy_scan_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.charge_legacy_work(amount)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticAssertionMeteredErrorV1<E> {
    Analysis(SemanticAssertionErrorV1),
    Meter(E),
}
impl<E> From<SemanticAssertionErrorV1> for SemanticAssertionMeteredErrorV1<E> {
    fn from(error: SemanticAssertionErrorV1) -> Self {
        Self::Analysis(error)
    }
}
impl<E: fmt::Display> fmt::Display for SemanticAssertionMeteredErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Analysis(error) => error.fmt(f),
            Self::Meter(error) => write!(f, "semantic assertion meter: {error}"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for SemanticAssertionMeteredErrorV1<E> {}

pub(super) type MR<T, M> =
    Result<T, SemanticAssertionMeteredErrorV1<<M as SemanticAssertionMeterV1>::Error>>;
pub(super) type ErrorFor<M> =
    SemanticAssertionMeteredErrorV1<<M as SemanticAssertionMeterV1>::Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticAssertionResourceObservationV1 {
    work: usize,
    storage: usize,
    legacy_visits: usize,
}
impl SemanticAssertionResourceObservationV1 {
    pub const fn work(self) -> usize {
        self.work
    }
    pub const fn reserved_payload_bytes(self) -> usize {
        self.storage
    }
    pub const fn legacy_visits(self) -> usize {
        self.legacy_visits
    }
}

pub(super) struct Budget {
    limits: SemanticAssertionLimitsV1,
    work: usize,
    storage: usize,
    legacy_visits: usize,
    failed: bool,
}
impl Budget {
    pub(super) fn new(limits: SemanticAssertionLimitsV1) -> Result<Self, SemanticAssertionErrorV1> {
        Ok(Self {
            limits,
            work: 0,
            storage: 0,
            legacy_visits: 0,
            failed: false,
        })
    }
    pub(super) fn observation(&self) -> SemanticAssertionResourceObservationV1 {
        SemanticAssertionResourceObservationV1 {
            work: self.work,
            storage: self.storage,
            legacy_visits: self.legacy_visits,
        }
    }
    pub(super) fn metered<'b, 'm, M: SemanticAssertionMeterV1>(
        &'b mut self,
        meter: &'m mut M,
    ) -> Metered<'b, 'm, M> {
        Metered { local: self, meter }
    }
}

pub(super) struct Metered<'b, 'm, M> {
    local: &'b mut Budget,
    meter: &'m mut M,
}
impl<M: SemanticAssertionMeterV1> Metered<'_, '_, M> {
    pub(super) fn charge(&mut self, amount: usize) -> MR<(), M> {
        self.charge_kind(amount, 0)
    }
    pub(super) fn legacy(&mut self, amount: usize) -> MR<(), M> {
        self.charge_kind(amount, 1)
    }
    pub(super) fn legacy_scan(&mut self, amount: usize) -> MR<(), M> {
        self.charge_kind(amount, 2)
    }
    fn charge_kind(&mut self, amount: usize, kind: u8) -> MR<(), M> {
        if self.local.failed {
            return Err(SemanticAssertionErrorV1::Accounting.into());
        }
        let result = self.charge_kind_inner(amount, kind);
        if result.is_err() {
            self.local.failed = true;
        }
        result
    }
    fn charge_kind_inner(&mut self, amount: usize, kind: u8) -> MR<(), M> {
        let legacy = kind != 0;
        let actual = self
            .local
            .work
            .checked_add(amount)
            .ok_or(SemanticAssertionErrorV1::Arithmetic)?;
        if actual > self.local.limits.work_units {
            return Err(SemanticAssertionErrorV1::WorkLimit {
                actual,
                limit: self.local.limits.work_units,
            }
            .into());
        }
        let visits = if legacy {
            self.local
                .legacy_visits
                .checked_add(amount)
                .ok_or(SemanticAssertionErrorV1::Arithmetic)?
        } else {
            self.local.legacy_visits
        };
        self.local.work = actual;
        self.local.legacy_visits = visits;
        if kind == 2 {
            self.meter
                .charge_legacy_scan_work(amount)
                .map_err(SemanticAssertionMeteredErrorV1::Meter)?;
        } else if legacy {
            self.meter
                .charge_legacy_work(amount)
                .map_err(SemanticAssertionMeteredErrorV1::Meter)?;
        } else {
            self.meter
                .charge_work(amount)
                .map_err(SemanticAssertionMeteredErrorV1::Meter)?;
        }
        if visits > MAX_SEMANTIC_ASSERTION_WORK_V1 {
            return Err(SemanticAssertionErrorV1::LogicalVisitLimit {
                actual: visits,
                limit: MAX_SEMANTIC_ASSERTION_WORK_V1,
            }
            .into());
        }
        Ok(())
    }
    pub(super) fn reserve(&mut self, bytes: usize) -> MR<(), M> {
        if self.local.failed {
            return Err(SemanticAssertionErrorV1::Accounting.into());
        }
        let result = self.reserve_inner(bytes);
        if result.is_err() {
            self.local.failed = true;
        }
        result
    }
    fn reserve_inner(&mut self, bytes: usize) -> MR<(), M> {
        let actual = self
            .local
            .storage
            .checked_add(bytes)
            .ok_or(SemanticAssertionErrorV1::Arithmetic)?;
        if actual > self.local.limits.storage_bytes {
            return Err(SemanticAssertionErrorV1::StorageLimit {
                actual,
                limit: self.local.limits.storage_bytes,
            }
            .into());
        }
        self.meter
            .reserve_storage(bytes)
            .map_err(SemanticAssertionMeteredErrorV1::Meter)?;
        self.local.storage = actual;
        Ok(())
    }
    pub(super) fn payload<T>(&mut self, count: usize) -> MR<(), M> {
        self.reserve(
            count
                .checked_mul(size_of::<T>())
                .ok_or(SemanticAssertionErrorV1::Arithmetic)?,
        )
    }
    pub(super) fn backing<T>(&mut self, count: usize) -> MR<Vec<T>, M> {
        self.payload::<T>(count)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| SemanticAssertionErrorV1::Allocation)?;
        self.payload::<T>(
            result
                .capacity()
                .checked_sub(count)
                .ok_or(SemanticAssertionErrorV1::Accounting)?,
        )?;
        Ok(result)
    }
    pub(super) fn table<T: Clone>(&mut self, count: usize, value: T) -> MR<Vec<T>, M> {
        self.charge(count)?;
        let mut result = self.backing(count)?;
        result.resize(count, value);
        Ok(result)
    }
    pub(super) fn grow<T>(&mut self, rows: &mut Vec<T>, additional: usize) -> MR<(), M> {
        let required = rows
            .len()
            .checked_add(additional)
            .ok_or(SemanticAssertionErrorV1::Arithmetic)?;
        let old = rows.capacity();
        if required <= old {
            return Ok(());
        }
        self.charge(rows.len())?;
        self.payload::<T>(required - old)?;
        rows.try_reserve_exact(additional)
            .map_err(|_| SemanticAssertionErrorV1::Allocation)?;
        self.payload::<T>(
            rows.capacity()
                .checked_sub(required)
                .ok_or(SemanticAssertionErrorV1::Accounting)?,
        )
    }
    pub(super) fn push<T>(&mut self, rows: &mut Vec<T>, value: T) -> MR<(), M> {
        self.grow(rows, 1)?;
        rows.push(value);
        Ok(())
    }
    pub(super) fn deque<T>(&mut self, count: usize) -> MR<VecDeque<T>, M> {
        self.payload::<T>(count)?;
        let mut rows = VecDeque::new();
        rows.try_reserve_exact(count)
            .map_err(|_| SemanticAssertionErrorV1::Allocation)?;
        self.payload::<T>(
            rows.capacity()
                .checked_sub(count)
                .ok_or(SemanticAssertionErrorV1::Accounting)?,
        )?;
        Ok(rows)
    }
    pub(super) fn set_reserve<T: Eq + std::hash::Hash>(
        &mut self,
        rows: &mut HashSet<T>,
        additional: usize,
    ) -> MR<(), M> {
        let required = rows
            .len()
            .checked_add(additional)
            .ok_or(SemanticAssertionErrorV1::Arithmetic)?;
        let old = rows.capacity();
        if required <= old {
            return Ok(());
        }
        self.charge(rows.len())?;
        self.payload::<T>(required - old)?;
        rows.try_reserve(additional)
            .map_err(|_| SemanticAssertionErrorV1::Allocation)?;
        self.payload::<T>(
            rows.capacity()
                .checked_sub(required)
                .ok_or(SemanticAssertionErrorV1::Accounting)?,
        )
    }
    pub(super) fn set_insert<T: Eq + std::hash::Hash>(
        &mut self,
        rows: &mut HashSet<T>,
        value: T,
    ) -> MR<bool, M> {
        self.charge(
            rows.len()
                .checked_add(1)
                .ok_or(SemanticAssertionErrorV1::Arithmetic)?,
        )?;
        self.set_reserve(rows, 1)?;
        Ok(rows.insert(value))
    }
    pub(super) fn set_contains<T: Eq + std::hash::Hash>(
        &mut self,
        rows: &HashSet<T>,
        value: &T,
    ) -> MR<bool, M> {
        self.charge(sum(rows.len(), 1)?)?;
        Ok(rows.contains(value))
    }
    pub(super) fn set_remove<T: Eq + std::hash::Hash>(
        &mut self,
        rows: &mut HashSet<T>,
        value: &T,
    ) -> MR<bool, M> {
        self.charge(sum(rows.len(), 1)?)?;
        Ok(rows.remove(value))
    }
    pub(super) fn map_reserve<K: Eq + std::hash::Hash, V>(
        &mut self,
        rows: &mut HashMap<K, V>,
        additional: usize,
    ) -> MR<(), M> {
        let required = rows
            .len()
            .checked_add(additional)
            .ok_or(SemanticAssertionErrorV1::Arithmetic)?;
        let old = rows.capacity();
        if required <= old {
            return Ok(());
        }
        self.charge(rows.len())?;
        self.payload::<(K, V)>(required - old)?;
        rows.try_reserve(additional)
            .map_err(|_| SemanticAssertionErrorV1::Allocation)?;
        self.payload::<(K, V)>(
            rows.capacity()
                .checked_sub(required)
                .ok_or(SemanticAssertionErrorV1::Accounting)?,
        )
    }
    pub(super) fn sort_work(&mut self, count: usize, fields: usize) -> MR<(), M> {
        let logarithm = (usize::BITS - count.leading_zeros()) as usize;
        self.charge(
            count
                .checked_mul(
                    logarithm
                        .checked_add(1)
                        .ok_or(SemanticAssertionErrorV1::Arithmetic)?,
                )
                .and_then(|n| n.checked_mul(fields))
                .ok_or(SemanticAssertionErrorV1::Arithmetic)?,
        )
    }
    pub(super) fn clone_place(&mut self, place: &SemanticPlaceV1) -> MR<SemanticPlaceV1, M> {
        self.charge(place.projections().len())?;
        let mut projections = self.backing(place.projections().len())?;
        projections.extend_from_slice(place.projections());
        self.payload::<SemanticProjectionV1>(place.projections().len())?;
        self.charge(place.projections().len())?;
        SemanticPlaceV1::new(place.local(), projections, place.ty())
            .map_err(|_| SemanticAssertionErrorV1::InvalidModel("copied source place").into())
    }
    pub(super) fn clone_operand(&mut self, value: &SemanticOperandV1) -> MR<SemanticOperandV1, M> {
        self.charge(1)?;
        Ok(match value {
            SemanticOperandV1::Copy(place) => SemanticOperandV1::Copy(self.clone_place(place)?),
            SemanticOperandV1::Move(place) => SemanticOperandV1::Move(self.clone_place(place)?),
            SemanticOperandV1::Constant(constant) => {
                let value = match constant.value() {
                    SemanticConstantValueV1::Bytes(bytes) => {
                        self.charge(bytes.as_bytes().len())?;
                        let mut copy = self.backing(bytes.as_bytes().len())?;
                        copy.extend_from_slice(bytes.as_bytes());
                        self.payload::<u8>(bytes.as_bytes().len())?;
                        self.charge(bytes.as_bytes().len())?;
                        SemanticConstantValueV1::Bytes(SemanticConstantBytesV1::new(copy).map_err(
                            |_| SemanticAssertionErrorV1::InvalidModel("copied constant bytes"),
                        )?)
                    }
                    value => value.clone(),
                };
                SemanticOperandV1::Constant(SemanticConstantV1::new(constant.ty(), value))
            }
        })
    }
    pub(super) fn clone_checked(
        &mut self,
        value: &SemanticCheckedBinaryRvalueV1,
    ) -> MR<SemanticCheckedBinaryRvalueV1, M> {
        Ok(SemanticCheckedBinaryRvalueV1::new(
            value.operation(),
            self.clone_operand(value.left())?,
            self.clone_operand(value.right())?,
        ))
    }
}

pub(super) fn analysis_error<M: SemanticAssertionMeterV1>(
    error: SemanticAssertionErrorV1,
) -> ErrorFor<M> {
    SemanticAssertionMeteredErrorV1::Analysis(error)
}

pub(super) fn sum(left: usize, right: usize) -> Result<usize, SemanticAssertionErrorV1> {
    left.checked_add(right)
        .ok_or(SemanticAssertionErrorV1::Arithmetic)
}
