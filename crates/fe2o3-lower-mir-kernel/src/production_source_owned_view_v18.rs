/// A source-owning entrance or query refusal, not execution permission.
#[derive(Debug)]
pub enum ProductionSourceOwnedViewErrorV18 {
    /// Original source construction, occurrence capture or complete replay failed.
    Source(ProductionPendingScopedSourceErrorV29),
    /// The shared work or retained-storage ledger refused an operation.
    Resource(ArgumentResourceV1),
    /// A checked borrowed query does not belong to its retained original owner.
    Binding(&'static str),
    /// The same-owner analysis framework refused construction or settlement.
    Analysis(fe2o3_pliron::CanonicalAnalysisScopeErrorV1),
    /// The exact physical private-memory checker refused a supported-family obligation.
    PrivateMemory(fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1),
}

impl std::fmt::Display for ProductionSourceOwnedViewErrorV18 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => error.fmt(formatter),
            Self::Resource(error) => error.fmt(formatter),
            Self::Binding(detail) => write!(formatter, "source-owned V18 query: {detail}"),
            Self::Analysis(error) => error.fmt(formatter),
            Self::PrivateMemory(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ProductionSourceOwnedViewErrorV18 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::Binding(_) => None,
            Self::Analysis(error) => Some(error),
            Self::PrivateMemory(error) => Some(error),
        }
    }
}

impl From<ArgumentResourceV1> for ProductionSourceOwnedViewErrorV18 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Resource(error)
    }
}
impl From<fe2o3_pliron::CanonicalAnalysisScopeErrorV1> for ProductionSourceOwnedViewErrorV18 {
    fn from(error: fe2o3_pliron::CanonicalAnalysisScopeErrorV1) -> Self {
        use fe2o3_pliron::CanonicalAnalysisScopeErrorV1 as Analysis;
        match error {
            Analysis::Resource(error)
            | Analysis::Inventory(fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(
                error,
            ))
            | Analysis::Sparse(fe2o3_kernel_analysis::CanonicalKirSparseErrorV1::Resource(error))
            | Analysis::MemorySsa(fe2o3_kernel_analysis::CanonicalKirMemorySsaErrorV1::Resource(
                error,
            )) => Self::Resource(error),
            other => Self::Analysis(other),
        }
    }
}
impl From<ProductionPendingScopedSourceErrorV29> for ProductionSourceOwnedViewErrorV18 {
    fn from(error: ProductionPendingScopedSourceErrorV29) -> Self {
        Self::Source(error)
    }
}
impl From<fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1>
    for ProductionSourceOwnedViewErrorV18
{
    fn from(error: fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1) -> Self {
        use fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1 as Physical;
        match error {
            Physical::Resource(error) => error.into(),
            Physical::Inventory(error) => {
                fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(error).into()
            }
            other => Self::PrivateMemory(other),
        }
    }
}
impl From<ScopedModuleErrorV29> for ProductionSourceOwnedViewErrorV18 {
    fn from(error: ScopedModuleErrorV29) -> Self {
        Self::Source(error.into())
    }
}

type SourceOwnedResultV18<T> = Result<T, ProductionSourceOwnedViewErrorV18>;

// Framework construction/settlement and arbitrary consumer errors have distinct
// chronology. Only the former is automatically retained as a source query.
enum SourceAnalysisBoundaryV18<E> {
    Framework(fe2o3_pliron::CanonicalAnalysisScopeErrorV1),
    Consumer(E),
}
impl<E> From<fe2o3_pliron::CanonicalAnalysisScopeErrorV1> for SourceAnalysisBoundaryV18<E> {
    fn from(error: fe2o3_pliron::CanonicalAnalysisScopeErrorV1) -> Self {
        Self::Framework(error)
    }
}

// Transparent error transport lets existing cleanup primitives retain the
// consumer's actual error without requiring unrelated conversion impls from it.
#[repr(transparent)]
struct SourceConsumerErrorV18<E>(E);
impl<E: From<ProductionSourceOwnedViewErrorV18>> From<ArgumentResourceV1>
    for SourceConsumerErrorV18<E>
{
    fn from(error: ArgumentResourceV1) -> Self {
        Self(ProductionSourceOwnedViewErrorV18::from(error).into())
    }
}
impl<E: From<ProductionSourceOwnedViewErrorV18>> From<ScopedModuleErrorV29>
    for SourceConsumerErrorV18<E>
{
    fn from(error: ScopedModuleErrorV29) -> Self {
        Self(ProductionSourceOwnedViewErrorV18::from(error).into())
    }
}
impl<E: From<ProductionSourceOwnedViewErrorV18>> From<ProductionSourceOwnedViewErrorV18>
    for SourceConsumerErrorV18<E>
{
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self(error.into())
    }
}

/// Move-only captured source input, before any module/table emission.
///
/// Preparation consumes the original source, launch and occurrence reservation.
/// It owns the projected input after the caller's temporary projection vectors
/// disappear. It cannot execute and has no raw-module or version-conversion API.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionPreparedSourceV18;
/// fn duplicate(source: ProductionPreparedSourceV18) { let _ = source.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionPreparedSourceV18;
/// fn execute(source: ProductionPreparedSourceV18) { let _ = source.into_executable(); }
/// ```
pub struct ProductionPreparedSourceV18 {
    source: ScopedSourceInputsV29,
    limits: ProductionSemanticKirLimitsV1,
    capture_storage: usize,
    retained: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

impl ProductionPendingScopedSourceOwnerV29 {
    /// Captures original source inputs without emitting a graph or storage table.
    ///
    /// Unlike the historical Pending constructor, this consuming entrance adopts
    /// any preexisting occurrence reservation as part of the returned prepared
    /// owner. That reservation must already be live, and must not be reserved or
    /// refunded again by the caller. A fresh capture is constructed once in the
    /// same production ledger. On failure, adopted payload drops before refund.
    pub fn prepare_source_with_budget_v18(
        owner: ProductionSemanticSsaOwnerV1,
        launch: crate::ProductionSourceLaunchRosterV1,
        input: crate::ProductionExecutionSourceInputV29<'_>,
        limits: ProductionSemanticKirLimitsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionPreparedSourceV18> {
        Self::prepare_source_and_kernel_abi_v18(owner, launch, input, None, limits, budget)
    }

    /// Captures the complete original kernel argument ABI beside the source.
    ///
    /// This retains descriptor/source correspondence only. It does not alter
    /// source address spaces, parameter types, or authorize an executable graph.
    pub fn prepare_source_with_kernel_abi_budget_v18(
        owner: ProductionSemanticSsaOwnerV1,
        launch: crate::ProductionSourceLaunchRosterV1,
        input: crate::ProductionExecutionSourceInputV29<'_>,
        kernel_abi: ProductionKernelArgumentAbiInputV18<'_>,
        limits: ProductionSemanticKirLimitsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionPreparedSourceV18> {
        Self::prepare_source_and_kernel_abi_v18(
            owner,
            launch,
            input,
            Some(kernel_abi),
            limits,
            budget,
        )
    }

    fn prepare_source_and_kernel_abi_v18(
        mut owner: ProductionSemanticSsaOwnerV1,
        launch: crate::ProductionSourceLaunchRosterV1,
        input: crate::ProductionExecutionSourceInputV29<'_>,
        kernel_abi: Option<ProductionKernelArgumentAbiInputV18<'_>>,
        limits: ProductionSemanticKirLimitsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionPreparedSourceV18> {
        let existing = owner
            .occurrence_storage()
            .map_or(0, |row| row.retained_storage());
        let floor = budget
            .storage()
            .checked_sub(existing)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let boundary = match ScopedSourceCleanupBoundaryV29::new::<
            ProductionPreparedSourceV18,
            ProductionSourceOwnedViewErrorV18,
        >(floor, budget)
        {
            Ok(boundary) => boundary,
            Err(error) => {
                drop((owner, launch));
                // The failed header reservation cannot mutate the checked
                // incoming floor. Preserve its selected error regardless.
                let _ = budget.release_storage(existing);
                return Err(error.into());
            }
        };
        boundary.run(budget, move |cleanup, budget| {
            let floor = budget
                .storage()
                .checked_sub(existing)
                .ok_or(ArgumentResourceV1::Accounting)?;
            scoped_source_attempt_v29(cleanup, budget, floor, move |budget| {
                // The attempt's own transient header is not retained source
                // credit. Existing occurrences, however, transfer exactly once.
                let floor = budget
                    .storage()
                    .checked_sub(existing)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                budget.reserve_storage(size_of::<ProductionPreparedSourceV18>())?;
                let capture_storage = match owner.occurrence_storage() {
                    Some(receipt) => receipt.retained_storage(),
                    None => {
                        let receipt = owner
                            .try_capture_occurrences_with_budget_v1(budget)
                            .map_err(ProductionPendingScopedSourceErrorV29::Occurrences)?;
                        budget.reserve_storage(receipt.retained_storage())?;
                        receipt.retained_storage()
                    }
                };
                let mut source = capture_pending_source_inputs_v18(owner, launch, input, budget)?;
                if let Some(kernel_abi) = kernel_abi {
                    source
                        .input
                        .capture_kernel_argument_abi_v18(&source.owner, kernel_abi, budget)
                        .map_err(ProductionPendingScopedSourceErrorV29::Source)?;
                }
                let retained = budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                if retained
                    != argument_sum_v1(&[
                        size_of::<ProductionPreparedSourceV18>(),
                        source.input.retained_storage,
                        capture_storage,
                    ])?
                {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                Ok(ProductionPreparedSourceV18 {
                    source,
                    limits,
                    capture_storage,
                    retained,
                    slot: std::ptr::from_ref(budget) as usize,
                    ledger: budget.work_ledger_identity_v1(),
                })
            })
        })
    }

    /// Replays this exact source owner before a same-ledger borrowed continuation.
    /// A successful callback keeps its own new storage live. Only this scope's
    /// known temporary headers are refunded; the pending owner remains borrowed.
    pub fn with_checked_source_v18<'work, T>(
        &self,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &ProductionSourceOwnedViewV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> SourceOwnedResultV18<T>,
    ) -> SourceOwnedResultV18<T> {
        self.check_source_owned_floor_v18(budget)?;
        let floor = budget.storage();
        with_scoped_source_cleanup_v29(budget, floor, |cleanup, budget| {
            self.with_checked_source_with_cleanup_v18(cleanup, budget, consume)
        })
    }

    fn check_source_owned_floor_v18(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        if self.inner.source.input.ledger != budget.work_ledger_identity_v1()
            || self.inner.pending.ledger != budget.work_ledger_identity_v1()
            || budget.storage()
                < argument_sum_v1(&[
                    self.inner.retained_storage,
                    self.inner.capture.preexisting_storage(),
                ])?
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    fn with_checked_source_with_cleanup_v18<'work, T>(
        &self,
        cleanup: &ScopedSourceCleanupV29,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &ProductionSourceOwnedViewV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> SourceOwnedResultV18<T>,
    ) -> SourceOwnedResultV18<T> {
        self.with_source_consumer_with_cleanup_v18(cleanup, budget, consume)
            .map_err(|error| error.0)
    }

    fn with_source_consumer_with_cleanup_v18<'work, T, E>(
        &self,
        cleanup: &ScopedSourceCleanupV29,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &ProductionSourceOwnedViewV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, SourceConsumerErrorV18<E>>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.check_source_owned_floor_v18(budget)?;
        let floor = budget.storage();
        let headers = scoped_source_attempt_v29(cleanup, budget, floor, |budget| {
            let headers = argument_sum_v1(&[
                size_of::<SourceOwnedQueryGuardV18>(),
                size_of::<ProductionSourceOwnedViewV18<'_>>(),
                size_of::<std::thread::Result<Result<T, E>>>(),
            ])?;
            budget.reserve_storage(headers)?;
            self.inner.replay_with_cleanup(cleanup, budget)?;
            Ok::<_, ProductionSourceOwnedViewErrorV18>(headers)
        })?;
        let guard = SourceOwnedQueryGuardV18::new(self, budget);
        let view = ProductionSourceOwnedViewV18 {
            owner: self,
            guard: &guard,
            cleanup,
        };
        let caught = {
            let view = &view;
            let budget = &mut *budget;
            // A refused first query still disposes the owned callback before
            // settling this view's already accepted header credit.
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                view.query(budget)?;
                consume(view, budget)
            }))
        };
        let prior_query_failure = guard.first.get();
        let postflight = if matches!(&caught, Ok(Ok(_))) {
            guard.check(self, cleanup, budget)
        } else {
            guard.observe_custody(cleanup, budget)
        };
        drop(view);
        drop(guard);
        source_owned_finish_callback_v18(
            caught,
            prior_query_failure,
            postflight,
            cleanup,
            budget,
            headers,
        )
        .map_err(SourceConsumerErrorV18)
    }
}

impl ProductionPreparedSourceV18 {
    /// Returns the already live source-input/header/occurrence reservation.
    /// It transfers into the consuming continuation and must not be double-counted.
    pub const fn adopted_storage(&self) -> usize {
        self.retained
    }

    /// Emits and independently replays the original source on its original slot.
    ///
    /// The complete source, V18 graph/table and instance metadata remain alive
    /// throughout the callback. After they drop, only their exact known credits
    /// are refunded. Callback-owned growth stays live on success. Errors and raw
    /// panic payloads retain their original chronology; lost custody forbids all
    /// containing refunds. No executable authority is produced by this method.
    pub fn with_checked_source_v18<'work, T>(
        self,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &ProductionSourceOwnedViewV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> SourceOwnedResultV18<T>,
    ) -> SourceOwnedResultV18<T> {
        self.with_source_consumer_v18(budget, consume)
    }

    /// Consumes the same prepared owner with a typed downstream continuation.
    /// Source replay, query chronology and cleanup are identical to the source
    /// inspection wrapper. The error type carries real downstream refusals;
    /// neither it nor the returned value grants execution authority.
    pub fn with_source_consumer_v18<'work, T, E>(
        self,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &ProductionSourceOwnedViewV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.consume_source_v18(budget, consume)
            .map_err(|error| error.0)
    }

    fn consume_source_v18<'work, T, E>(
        self,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &ProductionSourceOwnedViewV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, SourceConsumerErrorV18<E>>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || self.source.input.ledger != self.ledger
            || budget.storage() < self.retained
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let floor = budget.storage() - self.retained;
        let boundary = match ScopedSourceCleanupBoundaryV29::new::<T, SourceConsumerErrorV18<E>>(
            floor, budget,
        ) {
            Ok(boundary) => boundary,
            Err(error) => {
                let retained = self.retained;
                drop(self);
                let _ = budget.release_storage(retained);
                return Err(error.into());
            }
        };
        boundary.run(budget, move |cleanup, budget| {
            let floor = budget
                .storage()
                .checked_sub(self.retained)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let (pending, retained) =
                scoped_source_attempt_v29(cleanup, budget, floor, move |budget| {
                    let Self {
                        source,
                        limits,
                        capture_storage,
                        ..
                    } = self;
                    let mut donor = Some(source);
                    let inner = SourceOwnedScopedModuleV29::try_new_with_cleanup(
                        &mut donor, limits, cleanup, budget,
                    )?;
                    let pending = ProductionPendingScopedSourceOwnerV29 { inner };
                    budget.release_storage(size_of::<Self>())?;
                    let retained = argument_sum_v1(&[pending.adopted_storage(), capture_storage])?;
                    Ok::<_, ProductionSourceOwnedViewErrorV18>((pending, retained))
                })?;
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                pending.with_source_consumer_with_cleanup_v18(cleanup, budget, consume)
            }));
            drop(pending);
            source_owned_finish_callback_v18(caught, None, Ok(()), cleanup, budget, retained)
        })
    }
}

#[derive(Clone, Copy)]
enum SourceOwnedQueryFailureV18 {
    Resource(ArgumentResourceV1),
    Binding(&'static str),
    Analysis(fe2o3_pliron::CanonicalAnalysisScopeErrorV1),
    PrivateMemoryUnsupported {
        phase: &'static str,
        detail: &'static str,
    },
    PrivateMemoryPanicked,
}

// All scope-owned values have dropped before this exact-credit settlement.
// Consumer-owned backing, including an escaping error or panic payload, is not
// scratch and must never be included in a rollback-to-floor calculation.
fn source_owned_finish_callback_v18<T, E>(
    caught: std::thread::Result<Result<T, E>>,
    first: Option<SourceOwnedQueryFailureV18>,
    postflight: SourceOwnedResultV18<()>,
    cleanup: &ScopedSourceCleanupV29,
    budget: &mut ArgumentBudgetV1<'_>,
    storage: usize,
) -> Result<T, E>
where
    E: From<ProductionSourceOwnedViewErrorV18>,
{
    let released = if cleanup.is_denied() {
        Err(ArgumentResourceV1::Accounting)
    } else {
        budget
            .release_storage(storage)
            .inspect_err(|_| cleanup.deny_refund())
    };
    match caught {
        Err(payload) => std::panic::resume_unwind(payload),
        Ok(Err(error)) => match first {
            Some(first) => Err(first.error().into()),
            None => Err(error),
        },
        Ok(Ok(value)) => match postflight.and(released.map_err(Into::into)) {
            Ok(()) => Ok(value),
            Err(error) => {
                drop(value);
                Err(error.into())
            }
        },
    }
}
impl SourceOwnedQueryFailureV18 {
    // Resource and inventory causes reuse the existing latch variants. Keeping
    // only the physical refusal payload avoids a second enum discriminant.
    fn private_memory(error: fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1) -> Self {
        use fe2o3_kernel_analysis::{
            CanonicalKirInventoryErrorV1 as Inventory, CanonicalKirPrivateMemoryErrorV1 as Physical,
        };
        match error {
            Physical::Resource(error) | Physical::Inventory(Inventory::Resource(error)) => {
                Self::Resource(error)
            }
            Physical::Inventory(error) => Self::Analysis(
                fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(error),
            ),
            Physical::Unsupported { phase, detail } => {
                Self::PrivateMemoryUnsupported { phase, detail }
            }
            Physical::Panicked => Self::PrivateMemoryPanicked,
        }
    }

    fn error(self) -> ProductionSourceOwnedViewErrorV18 {
        match self {
            Self::Resource(error) => error.into(),
            Self::Binding(detail) => ProductionSourceOwnedViewErrorV18::Binding(detail),
            Self::Analysis(error) => ProductionSourceOwnedViewErrorV18::Analysis(error),
            Self::PrivateMemoryUnsupported { phase, detail } => {
                ProductionSourceOwnedViewErrorV18::PrivateMemory(
                    fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1::Unsupported {
                        phase,
                        detail,
                    },
                )
            }
            Self::PrivateMemoryPanicked => ProductionSourceOwnedViewErrorV18::PrivateMemory(
                fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1::Panicked,
            ),
        }
    }
}

struct SourceOwnedQueryGuardV18 {
    owner: usize,
    source: usize,
    graph: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    first: std::cell::Cell<Option<SourceOwnedQueryFailureV18>>,
}

impl SourceOwnedQueryGuardV18 {
    fn new(owner: &ProductionPendingScopedSourceOwnerV29, budget: &ArgumentBudgetV1<'_>) -> Self {
        Self {
            owner: std::ptr::from_ref(owner) as usize,
            source: std::ptr::from_ref(&owner.inner.source.owner) as usize,
            graph: std::ptr::from_ref(&owner.inner.pending.graph) as usize,
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            first: std::cell::Cell::new(None),
        }
    }

    fn reject<T>(&self, failure: SourceOwnedQueryFailureV18) -> SourceOwnedResultV18<T> {
        let first = self.first.get().unwrap_or(failure);
        self.first.set(Some(first));
        Err(first.error())
    }

    fn observe_custody(
        &self,
        cleanup: &ScopedSourceCleanupV29,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            cleanup.deny_refund();
        }
        if cleanup.is_denied() {
            Err(ArgumentResourceV1::Accounting.into())
        } else {
            Ok(())
        }
    }

    fn check(
        &self,
        owner: &ProductionPendingScopedSourceOwnerV29,
        cleanup: &ScopedSourceCleanupV29,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            cleanup.deny_refund();
            return self.reject(SourceOwnedQueryFailureV18::Resource(
                ArgumentResourceV1::Accounting,
            ));
        }
        if let Some(error) = self.first.get() {
            return Err(error.error());
        }
        if cleanup.is_denied() {
            return self.reject(SourceOwnedQueryFailureV18::Resource(
                ArgumentResourceV1::Accounting,
            ));
        }
        if self.owner != std::ptr::from_ref(owner) as usize
            || self.source != std::ptr::from_ref(&owner.inner.source.owner) as usize
            || self.graph != std::ptr::from_ref(&owner.inner.pending.graph) as usize
        {
            return self.reject(SourceOwnedQueryFailureV18::Binding(
                "original owner changed",
            ));
        }
        Ok(())
    }
}

/// Scoped original-source/SSA/launch and canonical V18 reconstruction evidence.
///
/// The graph and its physical table belong to the retained source owner. Queries
/// are metered and sticky on failure. This view proves neither value equivalence,
/// assertion elision, memory safety, execution discharge nor launch authority.
/// It cannot be fabricated, cloned, or retained past its checked callback.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18;
/// fn forge() -> ProductionSourceOwnedViewV18<'static> {
///     ProductionSourceOwnedViewV18 { owner: panic!(), guard: panic!(), cleanup: panic!() }
/// }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionPreparedSourceV18;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn escape(source: ProductionPreparedSourceV18, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = source.with_checked_source_v18(budget, |view, budget| view.canonical(budget));
/// }
/// ```
/// ```no_run
/// use fe2o3_lower_mir_kernel::{ProductionPreparedSourceV18, ProductionSourceOwnedViewErrorV18};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn inspect(source: ProductionPreparedSourceV18, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) -> Result<usize, ProductionSourceOwnedViewErrorV18> {
///     source.with_checked_source_v18(budget, |view, budget| view.root_count(budget))
/// }
/// ```
pub struct ProductionSourceOwnedViewV18<'scope> {
    owner: &'scope ProductionPendingScopedSourceOwnerV29,
    guard: &'scope SourceOwnedQueryGuardV18,
    cleanup: &'scope ScopedSourceCleanupV29,
}

fn source_analysis_owned_headers_v18<T, E, F>(_: &F) -> Result<usize, ArgumentResourceV1> {
    type Entry<F> = (usize, F);
    type Capture<'a, 'work, F> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a std::cell::Cell<usize>,
        F,
    );
    type Invoke<'a, F> = (F, &'a std::cell::Cell<bool>);
    type Framework<'a, 'work, F> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a fe2o3_pliron::CanonicalAnalysisCleanupV1<'a>,
        Invoke<'a, F>,
    );
    type Analysis<T, E> = Result<T, SourceAnalysisBoundaryV18<E>>;
    argument_sum_v1(&[
        size_of::<Capture<'_, '_, F>>(),
        std::mem::align_of::<Capture<'_, '_, F>>(),
        size_of::<std::panic::AssertUnwindSafe<Capture<'_, '_, F>>>(),
        size_of::<Entry<F>>(),
        std::mem::align_of::<Entry<F>>(),
        argument_product_v1(2, size_of::<SourceOwnedResultV18<Entry<F>>>())?,
        size_of::<std::thread::Result<SourceOwnedResultV18<Entry<F>>>>(),
        size_of::<std::panic::AssertUnwindSafe<Entry<F>>>(),
        size_of::<std::thread::Result<()>>(),
        size_of::<std::cell::Cell<usize>>(),
        argument_product_v1(4, size_of::<usize>())?,
        size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
        size_of::<SourceOwnedResultV18<()>>(),
        size_of::<fe2o3_pliron::CanonicalAnalysisCleanupV1<'_>>(),
        size_of::<std::cell::Cell<bool>>(),
        size_of::<Invoke<'_, F>>(),
        std::mem::align_of::<Invoke<'_, F>>(),
        size_of::<Framework<'_, '_, F>>(),
        std::mem::align_of::<Framework<'_, '_, F>>(),
        size_of::<std::panic::AssertUnwindSafe<Framework<'_, '_, F>>>(),
        size_of::<Analysis<T, E>>(),
        size_of::<std::thread::Result<Analysis<T, E>>>(),
        size_of::<std::thread::Result<Result<T, E>>>(),
        size_of::<Result<T, E>>(),
        size_of::<std::panic::AssertUnwindSafe<Result<T, E>>>(),
    ])
}

impl ProductionSourceOwnedViewV18<'_> {
    /// Checks the existing source query latch and custody without another debit.
    pub fn check_query_v18(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.guard.check(self.owner, self.cleanup, budget)
    }

    /// Retains a delegated query's resource refusal in this exact source owner.
    /// Prior query failures keep precedence; unrelated raw budget history is not observed.
    pub fn retain_query_resource_error_v18(
        &self,
        error: ArgumentResourceV1,
    ) -> ProductionSourceOwnedViewErrorV18 {
        self.guard
            .reject::<()>(SourceOwnedQueryFailureV18::Resource(error))
            .unwrap_err()
    }

    /// Lends one inventory and the shared lazy analyses of this exact V18 graph.
    /// Reports cannot escape. Consumer output storage must be prepaid outside
    /// the analysis scope, whose callback allocations are temporary scratch.
    /// Detected lost custody denies cleanup through the original source attempt
    /// before either the selected error or raw panic leaves this boundary.
    pub fn with_analysis_v18<'work, T, E>(
        &self,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl FnOnce(
            &mut fe2o3_pliron::CanonicalAnalysisScopeV18<'_, '_, '_, 'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>
            + From<fe2o3_pliron::CanonicalAnalysisScopeErrorV1>,
    {
        let floor = budget.storage();
        let slot = std::ptr::from_ref(budget) as usize;
        let ledger = budget.work_ledger_identity_v1();
        let accepted = std::cell::Cell::new(0_usize);
        // Own F before the first query, including refusal and Drop paths.
        let caught = {
            let budget = &mut *budget;
            let accepted = &accepted;
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                self.query(budget)?;
                let headers = self.retain_query(
                    source_analysis_owned_headers_v18::<T, E, _>(&consume).map_err(Into::into),
                )?;
                self.retain_query(budget.reserve_storage(headers).map_err(Into::into))?;
                accepted.set(headers);
                Ok::<_, ProductionSourceOwnedViewErrorV18>((headers, consume))
            }))
        };
        let custody = self.observe_analysis_entry_v18(budget, floor, accepted.get(), slot, ledger);
        let (headers, consume) = match caught {
            Ok(Ok(entry)) if custody.is_ok() => entry,
            Ok(Ok(entry)) => {
                let disposed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    drop(entry);
                }));
                let _ =
                    self.observe_analysis_entry_v18(budget, floor, accepted.get(), slot, ledger);
                if let Err(payload) = disposed {
                    std::panic::resume_unwind(payload);
                }
                return self
                    .retain_query(Err(ArgumentResourceV1::Accounting.into()))
                    .map_err(Into::into);
            }
            Ok(Err(error)) => {
                if custody.is_ok() && budget.release_storage(accepted.get()).is_err() {
                    self.cleanup.deny_refund();
                }
                return self.retain_query(Err(error)).map_err(Into::into);
            }
            Err(payload) => {
                if custody.is_ok() && budget.release_storage(accepted.get()).is_err() {
                    self.cleanup.deny_refund();
                }
                std::panic::resume_unwind(payload);
            }
        };
        let cleanup = fe2o3_pliron::CanonicalAnalysisCleanupV1::linked(&self.cleanup.denied);
        let entered = std::cell::Cell::new(false);
        let caught = {
            let budget = &mut *budget;
            let cleanup = &cleanup;
            let entered = &entered;
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                fe2o3_pliron::with_canonical_analysis_scope_v18(
                    &self.owner.inner.pending.graph,
                    budget,
                    cleanup,
                    move |scope| {
                        entered.set(true);
                        consume(scope).map_err(SourceAnalysisBoundaryV18::Consumer)
                    },
                )
            }))
        };
        if cleanup.refund_denied() {
            self.cleanup.deny_refund();
        }
        let caught = match caught {
            Ok(Err(SourceAnalysisBoundaryV18::Framework(error))) => {
                let retained = self.retain_query::<()>(Err(error.into())).unwrap_err();
                Ok(Err(retained.into()))
            }
            Ok(Err(SourceAnalysisBoundaryV18::Consumer(error))) => Ok(Err(error)),
            Ok(Ok(value)) => Ok(Ok(value)),
            Err(payload) => {
                if !entered.get() {
                    let _ = self.guard.reject::<()>(SourceOwnedQueryFailureV18::Binding(
                        "source analysis construction panicked",
                    ));
                }
                Err(payload)
            }
        };
        let prior_query_failure = self.guard.first.get();
        // An enclosing source scope must not mistake a later postflight
        // error for a query failure preceding this selected consumer error.
        let postflight = if matches!(&caught, Ok(Ok(_))) {
            self.guard.check(self.owner, self.cleanup, budget)
        } else {
            self.guard.observe_custody(self.cleanup, budget)
        };
        drop(cleanup);
        source_owned_finish_callback_v18(
            caught,
            prior_query_failure,
            postflight,
            self.cleanup,
            budget,
            headers,
        )
    }

    fn observe_analysis_entry_v18(
        &self,
        budget: &ArgumentBudgetV1<'_>,
        floor: usize,
        accepted: usize,
        slot: usize,
        ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    ) -> SourceOwnedResultV18<()> {
        if slot != std::ptr::from_ref(budget) as usize
            || ledger != budget.work_ledger_identity_v1()
            || floor.checked_add(accepted) != Some(budget.storage())
        {
            self.cleanup.deny_refund();
        }
        self.guard.observe_custody(self.cleanup, budget)
    }

    fn query(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.guard.check(self.owner, self.cleanup, budget)?;
        if let Err(error) = budget.charge_work(1) {
            return self
                .guard
                .reject(SourceOwnedQueryFailureV18::Resource(error));
        }
        Ok(())
    }

    fn retain_query<T>(&self, result: SourceOwnedResultV18<T>) -> SourceOwnedResultV18<T> {
        match result {
            Err(ProductionSourceOwnedViewErrorV18::Resource(error)) => self
                .guard
                .reject(SourceOwnedQueryFailureV18::Resource(error)),
            Err(ProductionSourceOwnedViewErrorV18::Binding(detail)) => self
                .guard
                .reject(SourceOwnedQueryFailureV18::Binding(detail)),
            Err(ProductionSourceOwnedViewErrorV18::Analysis(error)) => self
                .guard
                .reject(SourceOwnedQueryFailureV18::Analysis(error)),
            Err(ProductionSourceOwnedViewErrorV18::PrivateMemory(error)) => self
                .guard
                .reject(SourceOwnedQueryFailureV18::private_memory(error)),
            other => other,
        }
    }

    fn retain_construction<T>(
        &self,
        construct: impl FnOnce() -> SourceOwnedResultV18<T>,
    ) -> SourceOwnedResultV18<T> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(construct)) {
            Ok(result) => self.retain_query(result),
            Err(payload) => {
                let _ = self.guard.reject::<()>(SourceOwnedQueryFailureV18::Binding(
                    "source query construction panicked",
                ));
                std::panic::resume_unwind(payload)
            }
        }
    }

    fn missing<T>(&self, detail: &'static str) -> SourceOwnedResultV18<T> {
        self.guard
            .reject(SourceOwnedQueryFailureV18::Binding(detail))
    }

    fn root_row(&self, ordinal: usize) -> SourceOwnedResultV18<&ScopedModuleRootV29> {
        match self.owner.inner.pending.roots.get(ordinal) {
            Some(row) => Ok(row),
            None => self.missing("root ordinal"),
        }
    }

    fn sidecar(
        &self,
        root: usize,
        instance: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&PendingInstanceSidecarsV29> {
        match self.optional_sidecar(root, instance, budget)? {
            Some(row) => Ok(row),
            None => self.missing("inactive original instance has no emitted sidecar"),
        }
    }

    fn optional_sidecar(
        &self,
        root: usize,
        instance: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<&PendingInstanceSidecarsV29>> {
        let ordinal = self.active_ordinal(root, instance, budget)?;
        Ok(ordinal.map(|ordinal| &self.owner.inner.pending.roots[root].sidecars.rows[ordinal]))
    }

    fn active_ordinal(
        &self,
        root: usize,
        instance: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<usize>> {
        self.retain_query((|| {
            self.instance(root, instance, budget)?;
            let root = self.root_row(root)?;
            let ordinal = root
                .active_instances
                .sidecar_ordinal(
                    instance,
                    root.coordinates.sources.rows.len(),
                    &root.sidecars.rows,
                    budget,
                )
                .map_err(|error| match error {
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => {
                        ProductionSourceOwnedViewErrorV18::Resource(error)
                    }
                    _ => ProductionSourceOwnedViewErrorV18::Binding(
                        "active instance index changed its original roster",
                    ),
                })?;
            Ok(ordinal)
        })())
    }

    /// Returns the exact original admitted source, without changing its profile.
    pub fn source_semantic(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&AdmittedInertSemanticMirV1> {
        self.query(budget)?;
        Ok(self.owner.inner.source.owner.source_semantic())
    }

    /// Returns the original SSA owner with its retained occurrence capture.
    pub fn source_ssa(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&ProductionSemanticSsaOwnerV1> {
        self.query(budget)?;
        Ok(&self.owner.inner.source.owner)
    }

    /// Requires the exact retained SSA owner, not an equal-byte reconstruction.
    pub fn check_original_source(
        &self,
        source: &ProductionSemanticSsaOwnerV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.query(budget)?;
        if !std::ptr::eq(source, &self.owner.inner.source.owner) {
            return self.missing("foreign original SSA owner");
        }
        Ok(())
    }

    /// Compares a complete detached execution input with this retained source.
    ///
    /// This allocation-free query returns no source authentication or execution
    /// authority. The backend must retain its authentic receipt and exact source
    /// owner separately. Projection callbacks should only perform this comparison
    /// and return unit; subsequent recipe consumers require source-owned cleanup.
    /// Before entering projection, its caller must check the source query latch
    /// and exact owner. This closed comparison cannot allocate, release credits,
    /// invoke a consumer, or replace that already-authenticated custody.
    pub fn check_execution_input_v18(
        &self,
        input: ProductionExecutionSourceInputV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.query(budget)?;
        self.retain_query(
            self.owner
                .inner
                .source
                .input
                .check_candidate_v18(&self.owner.inner.source.owner, input, budget)
                .map_err(|error| match error {
                    crate::ProductionContextRootErrorV29::Resource(error) => error.into(),
                    _ => ProductionSourceOwnedViewErrorV18::Binding(
                        "execution source candidate differs from retained input",
                    ),
                }),
        )
    }

    /// Returns the original checked source launch roster, not launch permission.
    pub fn source_launch(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&crate::ProductionSourceLaunchRosterV1> {
        self.query(budget)?;
        Ok(&self.owner.inner.source.launch)
    }

    fn descriptor_root_abi_v29(
        &self,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<kernel_argument_abi_v18::SourceDescriptorRootAbiV29<'_>>> {
        self.retain_query((|| {
            self.query(budget)?;
            self.root_row(root)?;
            self.owner
                .inner
                .source
                .input
                .kernel_argument_abi
                .as_ref()
                .map(|profile| {
                    profile.descriptor_root(&self.owner.inner.source.owner, root, budget)
                })
                .transpose()
                .map_err(kernel_argument_abi_v18::query_error)
        })())
    }

    /// Returns the captured complete kernel argument count, if an ABI profile exists.
    ///
    /// Absence is not an empty ABI and supplies no address-space refinement.
    pub fn kernel_argument_abi_count(
        &self,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<usize>> {
        self.retain_query((|| {
            self.query(budget)?;
            self.root_row(root)?;
            budget.charge_work(4)?;
            let Some(profile) = &self.owner.inner.source.input.kernel_argument_abi else {
                return Ok(None);
            };
            profile
                .argument_count(root)
                .map(Some)
                .map_err(kernel_argument_abi_v18::query_error)
        })())
    }

    /// Returns an original argument's descriptor class, not pointer provenance.
    ///
    /// `None` denotes a compiler-laid-out by-value argument. Its source-owned
    /// proposal is not final native/runtime authority. Missing evidence fails.
    pub fn kernel_argument_abi_kind(
        &self,
        root: usize,
        argument: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<fe2o3_kernel_descriptor::SourceTypeDescriptorV3>> {
        self.retain_query((|| {
            self.query(budget)?;
            self.root_row(root)?;
            budget.charge_work(4)?;
            let profile = self
                .owner
                .inner
                .source
                .input
                .kernel_argument_abi
                .as_ref()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "original source has no kernel argument ABI profile",
                ))?;
            profile
                .argument_kind(root, argument)
                .map_err(kernel_argument_abi_v18::query_error)
        })())
    }

    /// Borrows an original by-value ABI proposal, not initialized memory.
    ///
    /// `None` denotes a descriptor argument. Missing profile/root/argument fails.
    pub fn kernel_argument_by_value_abi_v29(
        &self,
        root: usize,
        argument: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionKernelByValueAbiV29<'_>>> {
        self.retain_query((|| {
            self.query(budget)?;
            self.root_row(root)?;
            let profile = self
                .owner
                .inner
                .source
                .input
                .kernel_argument_abi
                .as_ref()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "original source has no kernel argument ABI profile",
                ))?;
            profile
                .by_value_argument_v29(&self.owner.inner.source.owner, root, argument, budget)
                .map_err(kernel_argument_abi_v18::query_error)
        })())
    }

    /// Rejoins the full retained ABI with an independently supplied original descriptor input.
    ///
    /// This checks original source and component identity, not final native
    /// parameters or runtime allocations. Final publication must check those too.
    pub fn require_kernel_argument_abi_v18(
        &self,
        input: ProductionKernelArgumentAbiInputV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.retain_query((|| {
            self.query(budget)?;
            let profile = self
                .owner
                .inner
                .source
                .input
                .kernel_argument_abi
                .as_ref()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "original source has no kernel argument ABI profile",
                ))?;
            profile
                .matches_original_input(&self.owner.inner.source.owner, input, budget)
                .map_err(kernel_argument_abi_v18::query_error)
        })())
    }

    /// Returns the single structural V18 graph/table owner, not an executable.
    pub fn canonical(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18> {
        self.query(budget)?;
        Ok(&self.owner.inner.pending.graph)
    }

    /// Returns the retained caller policy, never limits inferred from output rows.
    pub fn limits(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticKirLimitsV1> {
        self.query(budget)?;
        Ok(self.owner.inner.limits)
    }

    /// Returns the complete original root count.
    pub fn root_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<usize> {
        self.query(budget)?;
        Ok(self.owner.inner.pending.roots.len())
    }

    /// Returns an original semantic root and its complete-module function ordinal.
    pub fn root(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(SemanticFunctionIdV1, usize)> {
        self.query(budget)?;
        let row = self.root_row(ordinal)?;
        Ok((row.coordinates.root, row.function_ordinal))
    }

    /// Returns the dense original instance count, including inactive invocations.
    pub fn instance_count(
        &self,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        self.query(budget)?;
        Ok(self.root_row(root)?.coordinates.sources.rows.len())
    }

    /// Reports original control-plan activity without renumbering source IDs.
    /// False is authenticated inactive control, not inferred missing metadata.
    pub fn instance_active(
        &self,
        root: usize,
        instance: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<bool> {
        Ok(self.optional_sidecar(root, instance, budget)?.is_some())
    }

    /// Returns the exact source function and optional caller/block for one instance.
    pub fn instance(
        &self,
        root: usize,
        instance: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(SemanticFunctionIdV1, Option<(usize, SemanticBlockIdV1)>)> {
        self.query(budget)?;
        let Some(row) = self.root_row(root)?.coordinates.sources.rows.get(instance) else {
            return self.missing("instance ordinal");
        };
        if row.instance.index() != instance {
            return self.missing("instance identity");
        }
        Ok((
            row.function,
            row.incoming.map(|call| (call.caller.index(), call.block)),
        ))
    }

    /// Returns original invocation-entry coverage before instance relocation.
    /// The caller must retain root/instance qualification and use mapped segments
    /// for final graph positions; absence is not permission to invent a preheader.
    pub fn invocation_entry(
        &self,
        root: usize,
        instance: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionInvocationEntrySpanV1>> {
        self.query(budget)?;
        Ok(self
            .optional_sidecar(root, instance, budget)?
            .and_then(|row| row.invocation_entry.as_ref())
            .map(|row| row.span))
    }

    /// Returns the exact count of source-to-final mapped spans for one root.
    pub fn span_count(
        &self,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        self.query(budget)?;
        Ok(self.root_row(root)?.coordinates.spans.rows.len())
    }

    /// Returns one final segment as instance, block, first operation and count.
    /// The two segment slots retain removed-call and allocation-relocation gaps.
    pub fn span_segment(
        &self,
        root: usize,
        ordinal: usize,
        segment: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<(usize, BlockId, u32, u32)>> {
        self.query(budget)?;
        let Some(row) = self.root_row(root)?.coordinates.spans.rows.get(ordinal) else {
            return self.missing("span ordinal");
        };
        let Some(slot) = row.segments.get(segment) else {
            return self.missing("segment ordinal");
        };
        Ok(slot.map(|span| (row.instance.index(), span.block, span.first, span.count)))
    }

    /// Counts retained memory anchors, including diagnostic reads and kills.
    /// An authenticated inactive original instance has zero emitted anchors.
    pub fn memory_anchor_count(
        &self,
        root: usize,
        instance: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        self.query(budget)?;
        let Some(sidecar) = self.optional_sidecar(root, instance, budget)? else {
            return Ok(0);
        };
        match sidecar.scoped_memory_anchors.as_ref() {
            Some(rows) => Ok(rows.rows.len()),
            None => self.missing("memory anchor census missing"),
        }
    }

    /// Returns a retained access position/pointer before instance relocation.
    /// None denotes a retained non-access event, never an empty memory-effect proof.
    pub fn memory_access(
        &self,
        root: usize,
        instance: usize,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<(BlockId, usize, ValueId)>> {
        self.query(budget)?;
        let Some(anchors) = self
            .sidecar(root, instance, budget)?
            .scoped_memory_anchors
            .as_ref()
        else {
            return self.missing("memory anchor census missing");
        };
        let Some(row) = anchors.rows.get(ordinal) else {
            return self.missing("memory anchor ordinal");
        };
        Ok(match row.kind {
            ScopedMemoryAnchorKindV29::Object(_) => {
                return self.missing("typed object is not a scalar single-pointer memory access");
            }
            ScopedMemoryAnchorKindV29::Access {
                pointer,
                payload: _,
            } => Some((row.block, row.position, pointer)),
            ScopedMemoryAnchorKindV29::Kill { .. }
            | ScopedMemoryAnchorKindV29::FailureRead { .. } => None,
        })
    }

    /// Returns the complete original typed-storage payload, not currentness authority.
    /// Both copy endpoints, projection indices, access policies and the result are retained.
    pub fn memory_object(
        &self,
        root: usize,
        instance: usize,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<
        Option<(
            BlockId,
            usize,
            fe2o3_kernel_ir::StorageOperationV1,
            Option<ValueId>,
        )>,
    > {
        self.query(budget)?;
        let Some(anchors) = self
            .sidecar(root, instance, budget)?
            .scoped_memory_anchors
            .as_ref()
        else {
            return self.missing("memory anchor census missing");
        };
        let Some(row) = anchors.rows.get(ordinal) else {
            return self.missing("memory anchor ordinal");
        };
        match row.kind {
            ScopedMemoryAnchorKindV29::Object(_) => {
                let payload = anchors
                    .object_payload(row, budget)
                    .map_err(|error| source_attachment_error_v18(error.into()))?;
                Ok(Some((
                    row.block,
                    row.position,
                    payload.operation,
                    payload.result,
                )))
            }
            ScopedMemoryAnchorKindV29::Access { .. }
            | ScopedMemoryAnchorKindV29::Kill { .. }
            | ScopedMemoryAnchorKindV29::FailureRead { .. } => Ok(None),
        }
    }

    /// Returns the complete retained instance-qualified assertion count.
    pub fn assertion_count(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        self.query(budget)?;
        Ok(self.owner.inner.assertions.len())
    }

    /// Returns a replayed instance/site/physical binding, not an assertion proof.
    pub fn assertion(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(
        usize,
        SemanticKirAssertSiteV1,
        SemanticKirAssertConditionBindingV1,
    )> {
        self.query(budget)?;
        let Some(row) = self.owner.inner.assertions.get(ordinal) else {
            return self.missing("assertion ordinal");
        };
        Ok((row.instance.index(), row.site, row.binding))
    }
}
