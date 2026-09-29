//! Store-local bounds and conditional index injectivity on the actual graph.
//! Neither fact discharges conflicts with any other memory occurrence.
use super::*;

/// Descriptive Store domain. It is not a read certificate or allocation grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalGuardedGlobalStoreDomainV24(FormalRuntimeSliceReadDomainV1);

impl CanonicalGuardedGlobalStoreDomainV24 {
    pub const fn allocation(self) -> FormalAllocationIdentity {
        self.0.allocation()
    }
    pub const fn slice(self) -> ValueId {
        self.0.slice()
    }
    pub const fn index(self) -> ValueId {
        self.0.index()
    }
    pub const fn guard_index(self) -> ValueId {
        self.0.guard_index()
    }
    pub const fn length(self) -> ValueId {
        self.0.length()
    }
    pub const fn predicate(self) -> ValueId {
        self.0.predicate()
    }
    pub const fn pointer(self) -> ValueId {
        self.0.pointer()
    }
    pub const fn element_bytes(self) -> u64 {
        self.0.element_bytes()
    }
    pub const fn path(self) -> FormalGuardedPathV1 {
        self.0.path()
    }
}

#[derive(Clone, Copy)]
struct InvocationRow {
    function: FunctionCoordinate,
    value: ValueId,
    axis: Axis,
}

/// Unsupported Store conditions are never a completed safety report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalGuardedGlobalStoreReasonV24 {
    NotOrdinaryGlobalStore,
    MissingBoundOrProvenance,
}

/// A fresh Store observation borrowed from one graph and paid analysis scope.
pub struct CanonicalGuardedGlobalStoreFactV24<'scope, 'g> {
    inner: CanonicalGuardedGlobalReadFactV18<'scope, 'g>,
    invocations: &'scope [InvocationRow],
    accounting: &'scope Accounting,
}

impl<'scope, 'g> CanonicalGuardedGlobalStoreFactV24<'scope, 'g> {
    pub const fn owner(&self) -> &VerifiedCanonicalKernelIrModuleV18 {
        self.inner.owner()
    }
    pub const fn operation(&self) -> Coordinate {
        self.inner.operation()
    }
    pub const fn domain(&self) -> CanonicalGuardedGlobalStoreDomainV24 {
        CanonicalGuardedGlobalStoreDomainV24(*self.inner.domain())
    }
    pub const fn normalized_index_origin(&self) -> CanonicalGuardedReadIndexOriginV1 {
        self.inner.normalized_index_origin()
    }
    pub const fn normalized_length_origin(&self) -> ValueId {
        self.inner.normalized_length_origin()
    }
    pub const fn comparison_operands(&self) -> (ValueId, ValueId) {
        self.inner.comparison_operands()
    }

    /// Actual Global invocation intrinsic, after checked representation links.
    /// This is not injectivity over an arbitrary multidimensional launch.
    pub fn invocation_projection(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<Option<(Axis, ValueId)>> {
        self.accounting.enter(budget)?;
        self.accounting.save((|| {
            let CanonicalGuardedReadIndexOriginV1::ProvenOrigin(value) =
                self.normalized_index_origin()
            else {
                return Ok(None);
            };
            let function = self.operation().block.function;
            let selected = verification_find_last_by_v1(self.invocations, 2, budget, |row| {
                (row.function, row.value).cmp(&(function, value))
            })?;
            Ok(selected.map(|i| (self.invocations[i].axis, value)))
        })())
    }

    /// Conditional self-injectivity only. The caller must authenticate these
    /// launch/width inputs and check every other potentially conflicting access.
    pub fn distinct_invocations<'fact>(
        &'fact self,
        launch: ExplicitLaunchExtent,
        width: FormalIndexWidth,
        budget: &mut Budget<'_>,
    ) -> Result<Option<CanonicalGuardedStoreInjectivityV24<'fact, 'scope, 'g>>> {
        self.accounting.enter(budget)?;
        self.accounting.save((|| {
            budget.charge_work(20)?;
            let ExplicitLaunchExtent::Exact { rank, extents } = launch else {
                return Ok(None);
            };
            if !(1..=3).contains(&rank)
                || extents.iter().any(|n| *n == 0)
                || extents[usize::from(rank)..].iter().any(|n| *n != 1)
            {
                return Ok(None);
            }
            let maximum_extent = match width {
                FormalIndexWidth::Bits32 => 1_u64 << 32,
                FormalIndexWidth::Bits64 => u64::MAX,
                FormalIndexWidth::Unknown => return Ok(None),
            };
            let Some((axis, value)) = self.invocation_projection(budget)? else {
                return Ok(None);
            };
            let selected = match axis {
                Axis::X => 0,
                Axis::Y => 1,
                Axis::Z => 2,
            };
            if extents[selected] > maximum_extent
                || extents
                    .iter()
                    .enumerate()
                    .any(|(i, n)| i != selected && *n != 1)
            {
                return Ok(None);
            }
            Ok(Some(CanonicalGuardedStoreInjectivityV24 {
                store: self,
                launch,
                width,
                axis,
                value,
            }))
        })())
    }
}

/// Conditional self-injectivity of this exact Store, not whole-program race freedom.
pub struct CanonicalGuardedStoreInjectivityV24<'fact, 'scope, 'g> {
    store: &'fact CanonicalGuardedGlobalStoreFactV24<'scope, 'g>,
    launch: ExplicitLaunchExtent,
    width: FormalIndexWidth,
    axis: Axis,
    value: ValueId,
}
impl<'fact, 'scope, 'g> CanonicalGuardedStoreInjectivityV24<'fact, 'scope, 'g> {
    pub const fn store(&self) -> &'fact CanonicalGuardedGlobalStoreFactV24<'scope, 'g> {
        self.store
    }
    pub const fn launch(&self) -> ExplicitLaunchExtent {
        self.launch
    }
    pub const fn index_width(&self) -> FormalIndexWidth {
        self.width
    }
    pub const fn projection(&self) -> (Axis, ValueId) {
        (self.axis, self.value)
    }
}

pub enum CanonicalGuardedGlobalStoreOutcomeV24<'scope, 'g> {
    ProvedLocalConditions(CanonicalGuardedGlobalStoreFactV24<'scope, 'g>),
    NotProved(CanonicalGuardedGlobalStoreReasonV24),
}

/// Store-only view; the underlying read-shaped implementation is not exposed.
pub struct CheckedCanonicalGuardedGlobalStoresV24<'scope, 'g> {
    inner: &'scope CheckedCanonicalGuardedGlobalReadsV18<'scope, 'g>,
    invocations: &'scope [InvocationRow],
}
impl<'scope, 'g> CheckedCanonicalGuardedGlobalStoresV24<'scope, 'g> {
    pub fn owner(&self, budget: &mut Budget<'_>) -> Result<&'g VerifiedCanonicalKernelIrModuleV18> {
        self.inner.owner(budget)
    }
    pub fn function_count(&self, budget: &mut Budget<'_>) -> Result<usize> {
        self.inner.function_count(budget)
    }
    /// Store occurrences, all other global effects, unresolved calls.
    pub fn function_effects(
        &self,
        function: FunctionCoordinate,
        budget: &mut Budget<'_>,
    ) -> Result<(usize, usize, usize)> {
        self.inner.function_effects(function, budget)
    }
    pub fn store_at(
        &self,
        at: Coordinate,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalGuardedGlobalStoreOutcomeV24<'_, 'g>> {
        Ok(match self.inner.read_at(at, budget)? {
            CanonicalGuardedGlobalReadOutcomeV1::ProvedLocalConditions(inner) => {
                CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(
                    CanonicalGuardedGlobalStoreFactV24 {
                        inner,
                        invocations: self.invocations,
                        accounting: self.inner.accounting,
                    },
                )
            }
            CanonicalGuardedGlobalReadOutcomeV1::NotProved(reason) => {
                CanonicalGuardedGlobalStoreOutcomeV24::NotProved(
                    if reason == CanonicalGuardedGlobalReadReasonV1::NotOrdinaryGlobalRead {
                        CanonicalGuardedGlobalStoreReasonV24::NotOrdinaryGlobalStore
                    } else {
                        CanonicalGuardedGlobalStoreReasonV24::MissingBoundOrProvenance
                    },
                )
            }
        })
    }
    pub fn true_at(
        &self,
        at: Coordinate,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> Result<Option<CanonicalGuardedPredicateFactV18<'_, 'g>>> {
        self.inner.true_at(at, value, budget)
    }
    pub fn no_wrap_at(
        &self,
        at: Coordinate,
        checked: Coordinate,
        budget: &mut Budget<'_>,
    ) -> Result<Option<CanonicalGuardedNoWrapFactV18<'_, 'g>>> {
        self.inner.no_wrap_at(at, checked, budget)
    }
    /// Project a read from this exact owner, without granting write authority.
    /// This enables a caller's complete read/write pair census.
    pub fn read_invocation_projection(
        &self,
        read: &CanonicalGuardedGlobalReadFactV18<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<Option<(Axis, ValueId)>> {
        self.inner.accounting.enter(budget)?;
        self.inner.accounting.save((|| {
            budget.charge_work(2)?;
            if !std::ptr::eq(self.inner.facts.owner, read.owner()) {
                return Err(ResourceError::Accounting.into());
            }
            let CanonicalGuardedReadIndexOriginV1::ProvenOrigin(value) =
                read.normalized_index_origin()
            else {
                return Ok(None);
            };
            let function = read.operation().block.function;
            let selected = verification_find_last_by_v1(self.invocations, 2, budget, |row| {
                (row.function, row.value).cmp(&(function, value))
            })?;
            Ok(selected.map(|i| (self.invocations[i].axis, value)))
        })())
    }
}

fn invocation_index(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    budget: &mut Budget<'_>,
) -> Result<Vec<InvocationRow>> {
    let mut count = 0_usize;
    for function in &owner.module().functions {
        budget.charge_work(1)?;
        if let Some(body) = &function.body {
            for block in &body.blocks {
                budget.charge_work(1)?;
                for operation in &block.operations {
                    budget.charge_work(2)?;
                    if matches!(&operation.kind, OperationKind::Intrinsic(i) if matches!(i.kind, IntrinsicKind::InvocationIndex { kind: IndexKind::Global, .. }))
                    {
                        count = count.checked_add(1).ok_or(ResourceError::Arithmetic)?;
                    }
                }
            }
        }
    }
    let mut rows = Vec::new();
    budget.reserve_storage(
        count
            .checked_mul(size_of::<InvocationRow>())
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    rows.try_reserve_exact(count)
        .map_err(|_| ResourceError::Allocation)?;
    budget.reserve_storage(
        rows.capacity()
            .checked_sub(count)
            .and_then(|n| n.checked_mul(size_of::<InvocationRow>()))
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    for (ordinal, function) in owner.module().functions.iter().enumerate() {
        budget.charge_work(2)?;
        let function_coordinate =
            FunctionCoordinate(u32::try_from(ordinal).map_err(|_| ResourceError::Arithmetic)?);
        if let Some(body) = &function.body {
            for block in &body.blocks {
                budget.charge_work(1)?;
                for operation in &block.operations {
                    budget.charge_work(4)?;
                    let OperationKind::Intrinsic(i) = &operation.kind else {
                        continue;
                    };
                    let IntrinsicKind::InvocationIndex {
                        kind: IndexKind::Global,
                        axis,
                    } = i.kind
                    else {
                        continue;
                    };
                    let [result] = operation.results.as_slice() else {
                        return Err(ResourceError::Accounting.into());
                    };
                    if result.ty != Type::INDEX {
                        return Err(ResourceError::Accounting.into());
                    }
                    rows.push(InvocationRow {
                        function: function_coordinate,
                        value: result.id,
                        axis,
                    });
                }
            }
        }
    }
    verification_bounded_sort_by_v1(&mut rows, 2, budget, |a, b| {
        (a.function, a.value).cmp(&(b.function, b.value))
    })?;
    Ok(rows)
}

/// Derive Store bounds from the exact graph, preserving every other effect as
/// pending. Launch, allocation, alias, initializedness and pair conflicts are
/// separate obligations. The callback cannot detach an owning proof token.
pub fn with_canonical_guarded_global_stores_v24<'g, 'w, T>(
    owner: &'g VerifiedCanonicalKernelIrModuleV18,
    limits: CanonicalGuardedGlobalReadLimitsV1,
    budget: &mut Budget<'w>,
    consume: impl for<'scope> FnOnce(
        &CheckedCanonicalGuardedGlobalStoresV24<'scope, 'g>,
        &mut Budget<'w>,
    ) -> Result<T>,
) -> Result<T> {
    with_owner::<true, _, _>(owner, owner.module(), limits, budget, |inner, budget| {
        let floor = budget.storage();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let headers = size_of::<Vec<InvocationRow>>()
                .checked_add(size_of::<CheckedCanonicalGuardedGlobalStoresV24<'_, '_>>())
                .and_then(|n| {
                    n.checked_add(size_of::<CanonicalGuardedGlobalStoreOutcomeV24<'_, '_>>())
                })
                .and_then(|n| {
                    n.checked_add(size_of::<
                        Option<CanonicalGuardedStoreInjectivityV24<'_, '_, '_>>,
                    >())
                })
                .and_then(|n| n.checked_add(3 * size_of::<std::thread::Result<Result<T>>>()))
                .and_then(|n| {
                    n.checked_add(size_of::<(
                        [usize; 16],
                        InvocationRow,
                        ExplicitLaunchExtent,
                        FormalIndexWidth,
                    )>())
                })
                .ok_or(ResourceError::Arithmetic)?;
            budget.reserve_storage(headers)?;
            let rows = invocation_index(owner, budget)?;
            let view = CheckedCanonicalGuardedGlobalStoresV24 {
                inner,
                invocations: &rows,
            };
            let callback_floor = budget.storage();
            let result = consume(&view, budget);
            if !inner.accounting.valid(budget) || budget.storage() != callback_floor {
                drain(result);
                return Err(ResourceError::Accounting.into());
            }
            result
        }));
        let result = match caught {
            Ok(result) => result,
            Err(payload) => {
                drain(payload);
                Err(Failure::Panicked)
            }
        };
        if !inner.accounting.valid(budget) || budget.storage() < floor {
            drain(result);
            return Err(ResourceError::Accounting.into());
        }
        if let Err(error) = budget.release_storage(budget.storage() - floor) {
            drain(result);
            return Err(error.into());
        }
        inner.accounting.save(result)
    })
}

#[cfg(test)]
#[path = "canonical_guarded_stores_v24_tests.rs"]
mod tests;
