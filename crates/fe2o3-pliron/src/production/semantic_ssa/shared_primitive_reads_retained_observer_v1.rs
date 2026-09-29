//! Concrete retained storage for the unchanged Shared Reads observer.
//! Private unadmitted preparation seam; no source/SSA or query authority.
use super::*;
use adapter::shared_primitive_v29::ReadObserver;
type AnalysisResult<T> = std::result::Result<T, ProductionSemanticSsaErrorV1>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Terminal,
    Ready,
}
#[derive(Clone, Copy, Eq, PartialEq)]
struct Snapshot {
    slot: usize,
    ledger: Ledger,
    counter: usize,
    owned: usize,
    storage: usize,
    work: usize,
    peak: usize,
    denied_work: bool,
    denied_storage: bool,
}
fn snapshot(budget: &Budget<'_>, owned: &usize) -> Snapshot {
    Snapshot {
        slot: budget as *const Budget<'_> as usize,
        ledger: budget.work_ledger_identity_v1(),
        counter: owned as *const usize as usize,
        owned: *owned,
        storage: budget.storage(),
        work: budget.work(),
        peak: budget.peak_storage(),
        denied_work: budget.failed_work().is_some(),
        denied_storage: budget.failed_storage().is_some(),
    }
}
#[derive(Clone, Copy)]
struct Source<'a> {
    function: &'a SemanticFunctionDeclV1,
    types: &'a [SemanticTypeDeclV1],
}
pub(in super::super) struct RetainedSharedObserverV1<'a> {
    phase: Phase,
    source: Option<Source<'a>>,
    entry: Option<Snapshot>,
    held: Option<Snapshot>,
    reads: Reads,
    scratch: usize,
}
impl<'a> RetainedSharedObserverV1<'a> {
    pub(in super::super) fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            source: None,
            entry: None,
            held: None,
            reads: Reads {
                rows: Vec::new(),
                cap: 0,
            },
            scratch: 0,
        }
    }
    /// Dimensions and source are explicit inputs, not independently admitted facts.
    /// Later whole-wrapper glue must place this after original discovery/sizing.
    pub(in super::super) fn prepare_into(
        &mut self,
        function: &'a SemanticFunctionDeclV1,
        types: &'a [SemanticTypeDeclV1],
        scratch: usize,
        units: usize,
        budget: &mut Budget<'_>,
        owned: &mut usize,
        failure: &mut Option<Resource>,
    ) -> std::result::Result<(), Resource> {
        let fresh = self.phase == Phase::Fresh;
        self.phase = Phase::Terminal;
        let entry = snapshot(budget, owned);
        if !fresh
            || failure.is_some()
            || entry.denied_work
            || entry.denied_storage
            || entry.owned > entry.storage
        {
            return Err(first(failure, Resource::Accounting));
        }
        self.source = Some(Source { function, types });
        self.entry = Some(entry);
        self.scratch = scratch;
        self.reads.cap = units;
        // Bounded component header admission, not the eventual outer header join.
        let bytes = frame().map_err(|error| first(failure, error))?;
        reserve(budget, owned, failure, bytes)?;
        // Original scratch -> row bytes -> allocation -> observed excess order.
        reserve(budget, owned, failure, scratch)?;
        let row_bytes = units
            .checked_mul(size_of::<Read>())
            .ok_or(Resource::Arithmetic)
            .map_err(|error| first(failure, error))?;
        reserve(budget, owned, failure, row_bytes)?;
        self.reads
            .rows
            .try_reserve_exact(units)
            .map_err(|_| first(failure, Resource::Allocation))?;
        let actual = self
            .reads
            .rows
            .capacity()
            .checked_mul(size_of::<Read>())
            .ok_or(Resource::Arithmetic)
            .map_err(|error| first(failure, error))?;
        if actual > row_bytes {
            reserve(budget, owned, failure, actual - row_bytes)?;
        }
        if self.reads.rows.capacity() < self.reads.cap {
            return Err(first(failure, Resource::Accounting));
        }
        self.check(function, types, budget, owned, failure)
            .map_err(|error| first(failure, error))?;
        self.held = Some(snapshot(budget, owned));
        self.phase = Phase::Ready;
        Ok(())
    }
    fn check(
        &self,
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        budget: &Budget<'_>,
        owned: &usize,
        failure: &Option<Resource>,
    ) -> std::result::Result<(), Resource> {
        let source = self.source.ok_or(Resource::Accounting)?;
        let entry = self.entry.ok_or(Resource::Accounting)?;
        let now = snapshot(budget, owned);
        let growth = now
            .owned
            .checked_sub(entry.owned)
            .ok_or(Resource::Accounting)?;
        let expected = entry
            .storage
            .checked_add(growth)
            .ok_or(Resource::Arithmetic)?;
        if !std::ptr::eq(source.function, function)
            || !std::ptr::eq(source.types, types)
            || entry.slot != now.slot
            || entry.ledger != now.ledger
            || entry.counter != now.counter
            || now.storage != expected
            || now.work < entry.work
            || now.peak < entry.peak
            || now.denied_work
            || now.denied_storage
            || failure.is_some()
            || self.reads.rows.len() > self.reads.cap
            || self.reads.rows.capacity() < self.reads.cap
        {
            return Err(Resource::Accounting);
        }
        if let Some(held) = self.held {
            if now.owned < held.owned
                || now.storage < held.storage
                || now.work < held.work
                || now.peak < held.peak
            {
                return Err(Resource::Accounting);
            }
        }
        Ok(())
    }
    /// Fixed concrete operation over the original pair, not a callback/budget loan.
    pub(in super::super) fn record(
        &mut self,
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        borrow: SemanticTransparentBorrowSiteV1,
        source: u32,
        site: (u32, u32),
        place: &'a SemanticPlaceV1,
        budget: &mut Budget<'_>,
        owned: &usize,
        failure: &mut Option<Resource>,
    ) -> AnalysisResult<()> {
        if self.phase != Phase::Ready {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        let result = (|| {
            self.check(function, types, budget, owned, failure)
                .map_err(|error| {
                    first(failure, error);
                    ProductionSemanticSsaErrorV1::ResourceOverflow
                })?;
            // Capacity >= cap and len <= cap before the old work1/cap guard.
            // If len < cap, unchanged Reads::read cannot allocate while pushing.
            let mut meter = ObserverMeter { budget, failure };
            self.reads.read(borrow, source, site, place, &mut meter)
        })();
        if result.is_err() {
            self.phase = Phase::Terminal;
        } else {
            self.held = Some(snapshot(budget, owned));
        }
        result
    }
    pub(in super::super) fn check_if_prepared(
        &self,
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        budget: &Budget<'_>,
        owned: &usize,
        failure: &Option<Resource>,
    ) -> std::result::Result<(), Resource> {
        if self.phase == Phase::Fresh {
            return Ok(());
        }
        self.postflight_for(function, types, budget, owned, failure)
    }
    pub(in super::super) fn postflight_for(
        &self,
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        budget: &Budget<'_>,
        owned: &usize,
        failure: &Option<Resource>,
    ) -> std::result::Result<(), Resource> {
        if self.phase != Phase::Ready {
            return Err(Resource::Accounting);
        }
        self.check(function, types, budget, owned, failure)
    }
    #[cfg(test)]
    pub(in super::super) fn test_rows(&self) -> Vec<(u32, u32, u32, u32, u32, usize)> {
        self.reads
            .rows
            .iter()
            .map(|row| {
                (
                    row.borrow.test_coordinates().0,
                    row.borrow.test_coordinates().1,
                    row.source,
                    row.block,
                    row.statement,
                    row.place,
                )
            })
            .collect()
    }
    #[cfg(test)]
    pub(in super::super) fn test_capacity(&self) -> (usize, usize) {
        (
            self.reads.rows.as_ptr() as usize,
            self.reads.rows.capacity(),
        )
    }
}
fn first(failure: &mut Option<Resource>, error: Resource) -> Resource {
    *failure.get_or_insert(error)
}
fn reserve(
    budget: &mut Budget<'_>,
    owned: &mut usize,
    failure: &mut Option<Resource>,
    bytes: usize,
) -> std::result::Result<(), Resource> {
    let next = owned
        .checked_add(bytes)
        .ok_or(Resource::Arithmetic)
        .map_err(|error| first(failure, error))?;
    budget
        .reserve_storage(bytes)
        .map_err(|error| first(failure, error))?;
    *owned = next;
    Ok(())
}
/// Its sole policy is charge_work on the already bound physical ledger.
struct ObserverMeter<'s, 'w> {
    budget: &'s mut Budget<'w>,
    failure: &'s mut Option<Resource>,
}
impl BorrowWork for ObserverMeter<'_, '_> {
    fn work(&mut self, units: usize) -> AnalysisResult<()> {
        self.budget.charge_work(units).map_err(|error| {
            first(self.failure, error);
            ProductionSemanticSsaErrorV1::ResourceOverflow
        })
    }
}
fn frame() -> std::result::Result<usize, Resource> {
    let rows = [
        size_of::<RetainedSharedObserverV1<'static>>(),
        size_of::<(
            Phase,
            Source<'static>,
            Option<Source<'static>>,
            Snapshot,
            Option<Snapshot>,
            Reads,
            Read,
            Vec<Read>,
            &Reads,
            &mut Reads,
            &Read,
            &Vec<Read>,
            &mut Vec<Read>,
        )>(),
        size_of::<(
            &SemanticFunctionDeclV1,
            &[SemanticTypeDeclV1],
            &SemanticPlaceV1,
            SemanticTransparentBorrowSiteV1,
            (u32, u32),
            (u32, u32, usize),
            u32,
            usize,
            usize,
            usize,
            usize,
            usize,
            bool,
            bool,
        )>(),
        size_of::<(
            &Budget<'static>,
            &mut Budget<'static>,
            &usize,
            &mut usize,
            *const Budget<'static>,
            *const usize,
            *const SemanticPlaceV1,
            Ledger,
            &Option<Resource>,
            &mut Option<Resource>,
            &mut Resource,
            Option<Resource>,
        )>(),
        size_of::<(
            ObserverMeter<'static, 'static>,
            &mut ObserverMeter<'static, 'static>,
            &RetainedSharedObserverV1<'static>,
            &mut RetainedSharedObserverV1<'static>,
        )>(),
        size_of::<(
            std::result::Result<(), Resource>,
            std::result::Result<usize, Resource>,
            std::result::Result<Source<'static>, Resource>,
            std::result::Result<Snapshot, Resource>,
            Option<usize>,
            std::result::Result<(), std::collections::TryReserveError>,
            std::collections::TryReserveError,
            AnalysisResult<()>,
            Resource,
            ProductionSemanticSsaErrorV1,
        )>(),
        size_of::<(
            fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1,
            fe2o3_kernel_ir::CanonicalKernelIrVerificationStorageLimitV1,
            Option<Snapshot>,
            Snapshot,
            &[Read],
            &mut [Read],
        )>(),
        size_of::<(
            [usize; 8],
            std::array::IntoIter<usize, 8>,
            usize,
            usize,
            std::result::Result<usize, Resource>,
            Resource,
        )>(),
    ];
    rows.into_iter().try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })
}
#[cfg(test)]
#[path = "shared_primitive_reads_retained_observer_v1_tests.rs"]
mod tests;
