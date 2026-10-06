//! Same-ledger, sticky checked scope for the distinct selected-memory family.
use super::*;

pub(super) struct SelectedScopeCaptureV30<'graph, 'launch, C> {
    owner: &'graph VerifiedCanonicalKernelIrModuleV18,
    launches: &'launch [ExplicitLaunchExtent],
    width: FormalIndexWidth,
    limits: CanonicalGuardedGlobalReadLimitsV1,
    consume: Option<C>,
}

pub(super) struct SelectedCallbackFrameV30<'view, 'scope, 'graph, 'budget, 'work, C> {
    view: &'view CheckedCanonicalSelectedSliceDomainsV30<'scope, 'graph>,
    budget: &'budget mut Budget<'work>,
    consume: C,
}

impl<'scope, 'graph, 'work, C> SelectedCallbackFrameV30<'_, 'scope, 'graph, '_, 'work, C> {
    fn invoke<T>(self) -> std::thread::Result<Result<T>>
    where
        C: FnOnce(
            &CheckedCanonicalSelectedSliceDomainsV30<'scope, 'graph>,
            &mut Budget<'work>,
        ) -> Result<T>,
    {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            (self.consume)(self.view, self.budget)
        }))
    }
}

pub(super) struct SelectedExecutionFrameV30<'capture, 'graph, 'launch, 'budget, 'work, 'denied, C> {
    capture: &'capture mut SelectedScopeCaptureV30<'graph, 'launch, C>,
    budget: &'budget mut Budget<'work>,
    denied: &'denied Cell<bool>,
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
}

impl<'graph, 'work, C> SelectedExecutionFrameV30<'_, 'graph, '_, '_, 'work, '_, C> {
    fn run<T>(self) -> Result<Option<T>>
    where
        C: for<'scope> FnOnce(
            &CheckedCanonicalSelectedSliceDomainsV30<'scope, 'graph>,
            &mut Budget<'work>,
        ) -> Result<T>,
    {
        let Self {
            capture,
            budget,
            denied,
            slot,
            ledger,
        } = self;
        budget.charge_work(2)?;
        let count = capture.owner.module().functions.len();
        if count > capture.limits.functions {
            return Err(Failure::FunctionLimit {
                actual: count,
                limit: capture.limits.functions,
            });
        }
        if capture.launches.len() != count {
            return Ok(None);
        }
        let mut functions =
            crate::verification_typed_storage_v2::allocate_vector_v2(count, budget)?;
        for (ordinal, function) in capture.owner.module().functions.iter().enumerate() {
            budget.charge_work(2)?;
            let coordinate =
                FunctionCoordinate(u32::try_from(ordinal).map_err(|_| ResourceError::Arithmetic)?);
            let Some(row) = build::function(
                function,
                coordinate,
                capture.launches[ordinal],
                capture.width,
                capture.limits,
                budget,
            )?
            else {
                return Ok(None);
            };
            functions.push(row);
        }
        let accounting = Accounting {
            slot,
            ledger,
            floor: budget.storage(),
            failure: RefCell::new(None),
            refund_denied: Cell::new(false),
        };
        let view = CheckedCanonicalSelectedSliceDomainsV30 {
            owner: capture.owner,
            functions: &functions,
            width: capture.width,
            accounting: &accounting,
        };
        let consume = capture.consume.take().ok_or(ResourceError::Accounting)?;
        let called = SelectedCallbackFrameV30 {
            view: &view,
            budget,
            consume,
        }
        .invoke();
        let mut result = match called {
            Ok(result) => result,
            Err(payload) => {
                drain(payload);
                Err(Failure::Panicked)
            }
        };
        let postflight = if !accounting.valid(budget) {
            Err(accounting.refuse_retained_custody())
        } else if budget.storage() != accounting.floor {
            accounting.save(Err(ResourceError::Accounting.into()))
        } else {
            accounting.enter(budget)
        };
        if let Err(error) = postflight {
            drain(std::mem::replace(&mut result, Err(error)));
        }
        denied.set(accounting.refund_denied.get());
        drop(view);
        drop(functions);
        result.map(Some)
    }
}

pub(super) fn headers<T, C>() -> Result<usize> {
    fn h<T>() -> Result<usize> {
        size_of::<T>()
            .checked_add(2 * size_of::<Result<T>>())
            .ok_or_else(|| ResourceError::Arithmetic.into())
    }
    type Query<'a> = (
        &'a CheckedCanonicalSelectedSliceDomainsV30<'a, 'a>,
        &'a mut Budget<'a>,
        FunctionCoordinate,
        Coordinate,
        Option<&'a CanonicalSelectedSliceAccessV30>,
        Option<&'a [CanonicalSelectedSliceChoiceV30]>,
        &'a [CanonicalSelectedPointerNodeV30],
        &'a [CanonicalSelectedPointerIncomingV30],
        &'a [CanonicalSelectedSliceAccessV30],
        &'a [Option<CanonicalSelectedSliceParameterV30>],
        Option<usize>,
        Range<usize>,
        Failure,
        &'a Coordinate,
    );
    type Scope<'a> = (
        &'a mut Budget<'a>,
        &'a Cell<bool>,
        Accounting,
        Cell<bool>,
        &'a VerifiedCanonicalKernelIrModuleV18,
        Vec<SelectedFunctionV30>,
        Option<SelectedFunctionV30>,
        CheckedCanonicalSelectedSliceDomainsV30<'a, 'a>,
        CanonicalKernelIrWorkLedgerIdentityV1,
        [usize; 6],
        Box<dyn std::any::Any + Send>,
    );
    let mut bytes = 0usize;
    for size in [
        h::<Query<'_>>()?,
        h::<Scope<'_>>()?,
        h::<SelectedScopeCaptureV30<'_, '_, C>>()?,
        h::<SelectedCallbackFrameV30<'_, '_, '_, '_, '_, C>>()?,
        h::<SelectedExecutionFrameV30<'_, '_, '_, '_, '_, '_, C>>()?,
        h::<Option<T>>()?,
        h::<Result<T>>()?,
        2 * size_of::<std::thread::Result<Result<T>>>(),
        2 * size_of::<std::thread::Result<Result<Option<T>>>>(),
        2 * std::mem::align_of::<SelectedScopeCaptureV30<'_, '_, C>>(),
        2 * std::mem::align_of::<SelectedCallbackFrameV30<'_, '_, '_, '_, '_, C>>(),
        2 * std::mem::align_of::<SelectedExecutionFrameV30<'_, '_, '_, '_, '_, '_, C>>(),
    ] {
        bytes = bytes.checked_add(size).ok_or(ResourceError::Arithmetic)?;
    }
    Ok(bytes)
}

/// Derives ordered choices on the exact actual final owner. `None` means at
/// least one external effect or selected path lacks a complete local proof;
/// the consumer is not called. The caller must not fall back after any typed
/// resource refusal. No source identity, runtime binding or publication proof
/// can be constructed from this callback alone.
pub fn with_canonical_selected_slice_domains_v30<'graph, 'work, T>(
    owner: &'graph VerifiedCanonicalKernelIrModuleV18,
    launches: &[ExplicitLaunchExtent],
    width: FormalIndexWidth,
    limits: CanonicalGuardedGlobalReadLimitsV1,
    budget: &mut Budget<'work>,
    consume: impl for<'scope> FnOnce(
        &CheckedCanonicalSelectedSliceDomainsV30<'scope, 'graph>,
        &mut Budget<'work>,
    ) -> Result<T>,
) -> Result<Option<T>> {
    enter(
        SelectedScopeCaptureV30 {
            owner,
            launches,
            width,
            limits,
            consume: Some(consume),
        },
        budget,
    )
}

fn enter<'graph, 'work, T, C>(
    mut capture: SelectedScopeCaptureV30<'graph, '_, C>,
    budget: &mut Budget<'work>,
) -> Result<Option<T>>
where
    C: for<'scope> FnOnce(
        &CheckedCanonicalSelectedSliceDomainsV30<'scope, 'graph>,
        &mut Budget<'work>,
    ) -> Result<T>,
{
    let floor = budget.storage();
    let slot = std::ptr::from_ref(&*budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    let denied = Cell::new(false);
    let prepaid =
        headers::<T, C>().and_then(|bytes| budget.reserve_storage(bytes).map_err(Into::into));
    if let Err(error) = prepaid {
        drain(capture.consume.take());
        return Err(error);
    }
    let frame = SelectedExecutionFrameV30 {
        capture: &mut capture,
        budget,
        denied: &denied,
        slot,
        ledger,
    };
    let mut returned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || frame.run()));
    drain(capture.consume.take());
    if denied.get()
        || slot != std::ptr::from_ref(&*budget) as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage() < floor
    {
        return refused_retained_result(returned, ResourceError::Accounting.into());
    }
    if returned.is_err() {
        drain(std::mem::replace(&mut returned, Ok(Err(Failure::Panicked))));
    }
    let result = match returned {
        Ok(result) => result,
        Err(_) => unreachable!(),
    };
    if let Err(error) = budget.release_storage(budget.storage() - floor) {
        drain(result);
        return Err(error.into());
    }
    result
}
