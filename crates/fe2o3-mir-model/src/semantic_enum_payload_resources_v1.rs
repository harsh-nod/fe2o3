//! Original-caller resource admission for the shared enum-payload analyzer.
//! Logical reservations are retained by the caller until all results/scratch drop.
#[cfg(test)]
#[path = "semantic_enum_payload_resources_v1_tests.rs"]
mod tests;
use super::*;
use std::mem::size_of;

/// Live caller ledger, independent of any compiler-resource crate dependency.
pub trait SemanticEnumPayloadMeterV1 {
    /// Exact caller resource failure, preserved without erasing its kind.
    type Error;
    /// Admit work before traversal, comparison, initialization or copying.
    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error>;
    /// Admit coexisting allocation payload before allocation.
    fn reserve_storage(&mut self, amount: usize) -> Result<(), Self::Error>;
}

/// Exact metered failure; no refusal is converted into an absent source fact.
#[derive(Debug, Eq, PartialEq)]
pub enum SemanticEnumPayloadMeteredErrorV1<E> {
    /// Existing closed analysis/local work-limit refusal.
    Analysis(SemanticOptionDominanceErrorV1),
    /// Original caller ledger denial.
    Meter(E),
    /// Fallible allocation or unexpected capacity refused.
    Allocation,
    /// Checked resource arithmetic overflowed before its operation.
    Arithmetic,
}
impl<E: fmt::Display> fmt::Display for SemanticEnumPayloadMeteredErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Analysis(e) => e.fmt(f),
            Self::Meter(e) => e.fmt(f),
            Self::Allocation => f.write_str("enum payload allocation refused"),
            Self::Arithmetic => f.write_str("enum payload resource arithmetic overflow"),
        }
    }
}
impl<E: Error + 'static> Error for SemanticEnumPayloadMeteredErrorV1<E> {}

pub(super) trait InternalMeter {
    fn work(&mut self, amount: usize) -> Result<(), SemanticOptionDominanceErrorV1>;
    fn storage(&mut self, amount: usize) -> Result<(), SemanticOptionDominanceErrorV1>;
    fn failure(&mut self, arithmetic: bool);
}
struct Adapter<'a, M: SemanticEnumPayloadMeterV1> {
    original: &'a mut M,
    failure: Option<SemanticEnumPayloadMeteredErrorV1<M::Error>>,
}
impl<M: SemanticEnumPayloadMeterV1> InternalMeter for Adapter<'_, M> {
    fn work(&mut self, amount: usize) -> Result<(), SemanticOptionDominanceErrorV1> {
        match self.original.charge_work(amount) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.failure = Some(SemanticEnumPayloadMeteredErrorV1::Meter(error));
                Err(SemanticOptionDominanceErrorV1::Storage)
            }
        }
    }
    fn storage(&mut self, amount: usize) -> Result<(), SemanticOptionDominanceErrorV1> {
        match self.original.reserve_storage(amount) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.failure = Some(SemanticEnumPayloadMeteredErrorV1::Meter(error));
                Err(SemanticOptionDominanceErrorV1::Storage)
            }
        }
    }
    fn failure(&mut self, arithmetic: bool) {
        if self.failure.is_none() {
            self.failure = Some(if arithmetic {
                SemanticEnumPayloadMeteredErrorV1::Arithmetic
            } else {
                SemanticEnumPayloadMeteredErrorV1::Allocation
            });
        }
    }
}

// Explicitly account the generic error-bearing frames. M::Error has no size
// bound; the fixed lexical envelope must never stand in for any of these.
fn metered_frame_storage_v1<M: SemanticEnumPayloadMeterV1>() -> Option<usize> {
    type Facts = SemanticEnumPayloadDominanceV1;
    [
        size_of::<Facts>(),
        size_of::<DominatorIntervalsV1>(),
        size_of::<WorkBudgetV1<'_>>(),
        size_of::<Adapter<'_, M>>(),
        // Callee result and the retained `result` can coexist during transfer.
        size_of::<Result<Facts, SemanticOptionDominanceErrorV1>>(),
        size_of::<Result<Facts, SemanticOptionDominanceErrorV1>>(),
        // Error mapping/construction and the public return result.
        size_of::<Result<Facts, SemanticEnumPayloadMeteredErrorV1<M::Error>>>(),
        size_of::<Result<Facts, SemanticEnumPayloadMeteredErrorV1<M::Error>>>(),
        // A meter call result, extracted caller error, and failure-transfer value.
        size_of::<Result<(), M::Error>>(),
        size_of::<M::Error>(),
        size_of::<SemanticEnumPayloadMeteredErrorV1<M::Error>>(),
        // Only bounded, non-generic iterator/counter/borrow headers remain here.
        4096,
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| total.checked_add(bytes))
}

impl SemanticEnumPayloadDominanceV1 {
    /// Derive real source facts with live original-ledger admission.
    ///
    /// The caller owns every accepted reservation, including construction and
    /// reallocation overlap, on success, error and unwind. Retain all of them
    /// until this result and all partial values have dropped. This method never
    /// releases the caller ledger and does not confer source/compiler authority.
    /// The local legacy limit is additional to, not a substitute for, the meter.
    pub fn analyze_with_meter_v1<M: SemanticEnumPayloadMeterV1>(
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        meter: &mut M,
    ) -> Result<Self, SemanticEnumPayloadMeteredErrorV1<M::Error>> {
        let Some(frame_storage) = metered_frame_storage_v1::<M>() else {
            return Err(SemanticEnumPayloadMeteredErrorV1::Arithmetic);
        };
        let mut adapter = Adapter {
            original: meter,
            failure: None,
        };
        let result = {
            let mut budget = WorkBudgetV1 {
                used: 0,
                meter: Some(&mut adapter),
            };
            // Fixed lexical frames/headers; nested heap Vec headers are charged
            // separately as elements of their actual outer allocations.
            budget
                .reserve_bytes(frame_storage)
                .and_then(|()| Self::analyze_with_budget(function, types, &mut budget))
        };
        match adapter.failure {
            Some(error) => Err(error),
            None => result.map_err(SemanticEnumPayloadMeteredErrorV1::Analysis),
        }
    }
}

impl WorkBudgetV1<'_> {
    // Extra work was absent from the legacy diagnostic counter. Charge it live
    // externally without changing old work_units or raising the old local cap.
    pub(super) fn extra(&mut self, amount: usize) -> Result<(), SemanticOptionDominanceErrorV1> {
        if let Some(meter) = &mut self.meter {
            meter.work(amount)?;
        }
        Ok(())
    }
    pub(super) fn fail(&mut self, arithmetic: bool) -> SemanticOptionDominanceErrorV1 {
        if let Some(meter) = &mut self.meter {
            meter.failure(arithmetic);
        }
        SemanticOptionDominanceErrorV1::Storage
    }
    fn product(
        &mut self,
        count: usize,
        unit: usize,
    ) -> Result<usize, SemanticOptionDominanceErrorV1> {
        count.checked_mul(unit).ok_or_else(|| self.fail(true))
    }
    pub(super) fn reserve_bytes(
        &mut self,
        bytes: usize,
    ) -> Result<(), SemanticOptionDominanceErrorV1> {
        if let Some(meter) = &mut self.meter {
            meter.storage(bytes)?;
        }
        Ok(())
    }
    pub(super) fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
    ) -> Result<(), SemanticOptionDominanceErrorV1> {
        let requested = values
            .len()
            .checked_add(additional)
            .ok_or_else(|| self.fail(true))?;
        if requested <= values.capacity() {
            return Ok(());
        }
        if self.meter.is_some() {
            // Old payload remains reserved: full new capacity prepays overlap.
            self.extra(values.len())?;
            let bytes = self.product(requested, size_of::<T>())?;
            self.reserve_bytes(bytes)?;
        }
        if self.meter.is_some() {
            values.try_reserve_exact(additional)
        } else {
            // Preserve amortized legacy Vec growth; exact cumulative growth is
            // an external-meter accounting policy, not a legacy algorithm cost.
            values.try_reserve(additional)
        }
        .map_err(|_| self.fail(false))?;
        // Never initialize or retain unexpected unreserved allocator capacity.
        if self.meter.is_some() && size_of::<T>() != 0 && values.capacity() != requested {
            return Err(self.fail(false));
        }
        Ok(())
    }
    pub(super) fn push<T>(
        &mut self,
        values: &mut Vec<T>,
        value: T,
    ) -> Result<(), SemanticOptionDominanceErrorV1> {
        self.extra(1)?;
        self.reserve(values, 1)?;
        values.push(value);
        Ok(())
    }
    pub(super) fn filled<T: Clone>(
        &mut self,
        count: usize,
        value: T,
    ) -> Result<Vec<T>, SemanticOptionDominanceErrorV1> {
        self.extra(count)?;
        let mut values = Vec::new();
        self.reserve(&mut values, count)?;
        values.resize(count, value);
        Ok(values)
    }
    pub(super) fn nested<T>(
        &mut self,
        count: usize,
    ) -> Result<Vec<Vec<T>>, SemanticOptionDominanceErrorV1> {
        self.extra(count)?;
        let mut values = Vec::new();
        self.reserve(&mut values, count)?;
        values.resize_with(count, Vec::new);
        Ok(values)
    }
    pub(super) fn boxed<T>(
        &mut self,
        values: Vec<T>,
    ) -> Result<Box<[T]>, SemanticOptionDominanceErrorV1> {
        if values.len() != values.capacity() {
            self.extra(values.len())?;
            let bytes = self.product(values.len(), size_of::<T>())?;
            self.reserve_bytes(bytes)?;
        }
        Ok(values.into_boxed_slice())
    }
}
