//! Test-only observation of the actual pre-ranked stage, followed by its verifier.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use std::{
    cell::Cell,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

#[derive(Debug)]
pub(crate) enum ObservationError<E> {
    Pipeline(ProductionPipelineError),
    Resource(Resource),
    Callback(E),
}
impl<E> From<Resource> for ObservationError<E> {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl<E: std::fmt::Debug> std::fmt::Display for ObservationError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pre-ranked source observation: {self:?}")
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ObservationError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Pipeline(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::Callback(error) => Some(error),
        }
    }
}
type ResultV1<T, E> = Result<T, ObservationError<E>>;
type PanicPayload = Box<dyn std::any::Any + Send>;

struct Custody {
    ledger: Ledger,
    slot: usize,
    floor: usize,
    failed: Cell<Option<Resource>>,
}
impl Custody {
    fn check(&self, budget: &Budget<'_>) -> Result<(), Resource> {
        let error = self.failed.get().or_else(|| {
            (budget.work_ledger_identity_v1() != self.ledger
                || budget as *const Budget<'_> as usize != self.slot
                || budget.storage() < self.floor)
                .then_some(Resource::Accounting)
        });
        if let Some(error) = error {
            self.failed.set(Some(error));
            Err(error)
        } else {
            Ok(())
        }
    }
    fn query(&self, budget: &mut Budget<'_>) -> Result<(), Resource> {
        let result = self.check(budget).and_then(|()| budget.charge_work(1));
        if let Err(error) = result {
            self.failed.set(Some(error));
        }
        result
    }
}

pub(crate) struct PreRankedSourceView<'scope> {
    stage: &'scope MaterializedNeutralProductionCompilation,
    custody: &'scope Custody,
}
impl PreRankedSourceView<'_> {
    pub(crate) fn owner<'a>(
        &'a self,
        budget: &mut Budget<'_>,
    ) -> Result<&'a fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1, Resource> {
        self.custody.query(budget)?;
        Ok(&self.stage.materialized)
    }
    pub(crate) fn target<'a>(
        &'a self,
        budget: &mut Budget<'_>,
    ) -> Result<&'a crate::production_target_v1::AuthenticatedProductionTargetV1, Resource> {
        self.custody.query(budget)?;
        Ok(&self.stage.bindings.rustc_target)
    }
    pub(crate) fn roots<'a>(
        &'a self,
        budget: &mut Budget<'_>,
    ) -> Result<&'a [crate::production_ranked_projection_v1::ProductionRankedRootInputV1], Resource>
    {
        self.custody.query(budget)?;
        Ok(&self.stage.ranked_roots)
    }
    pub(crate) fn descriptors<'a>(
        &'a self,
        budget: &mut Budget<'_>,
    ) -> Result<&'a [crate::compiler_descriptor::TypedDescriptorRootV1], Resource> {
        self.custody.query(budget)?;
        Ok(&self.stage.bindings.typed_descriptor_roots)
    }
}

fn header<T, R, E>() -> Result<usize, Resource> {
    [
        size_of::<Option<T>>(),
        size_of::<Custody>(),
        size_of::<PreRankedSourceView<'_>>(),
        size_of::<Result<R, E>>(),
        size_of::<ResultV1<(T, R), E>>(),
        2 * size_of::<Option<PanicPayload>>(),
        size_of::<(Ledger, usize, usize)>(),
    ]
    .into_iter()
    .try_fold(0_usize, |a, b| a.checked_add(b).ok_or(Resource::Arithmetic))
}

// Only this closure lends the live stage. Failure drops it before refund; success
// transfers it for immediate consumption by the unchanged production verifier.
fn owned_scope<'work, T, R, E>(
    owner: T,
    retained: usize,
    budget: &mut Budget<'work>,
    observe: impl FnOnce(&T, &Custody, &mut Budget<'work>) -> Result<R, E>,
) -> ResultV1<(T, R), E> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'_> as usize;
    let paid = retained
        .checked_add(header::<T, R, E>()?)
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(paid)?;
    let mut owner = Some(owner);
    let custody = Custody {
        ledger,
        slot,
        floor: budget.storage(),
        failed: Cell::new(None),
    };
    let mut panic: Option<PanicPayload> = None;
    let mut drop_panic: Option<PanicPayload> = None;
    let mut result = match catch_unwind(AssertUnwindSafe(|| {
        observe(owner.as_ref().unwrap(), &custody, budget).map_err(ObservationError::Callback)
    })) {
        Ok(result) => result,
        Err(payload) => {
            panic = Some(payload);
            Err(Resource::Accounting.into())
        }
    };
    if let Err(error) = custody.check(budget) {
        let rejected = std::mem::replace(&mut result, Err(error.into()));
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
            drop_panic = Some(payload);
        }
    }
    let mut result = match result {
        Ok(value) => Ok((owner.take().unwrap(), value)),
        Err(error) => {
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(owner.take()))) {
                drop_panic = Some(payload);
            }
            Err(error)
        }
    };
    let same =
        budget.work_ledger_identity_v1() == ledger && budget as *const Budget<'_> as usize == slot;
    if same && budget.storage() >= floor {
        if let Err(error) = budget.release_storage(budget.storage() - floor) {
            let rejected = std::mem::replace(&mut result, Err(error.into()));
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
                drop_panic = Some(payload);
            }
        }
    }
    if let Some(payload) = panic {
        drop(result);
        drop(drop_panic);
        resume_unwind(payload);
    }
    if let Some(payload) = drop_panic {
        drop(result);
        resume_unwind(payload);
    }
    result
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn test_observe_pre_ranked_then_verify_v1<R, E>(
        self,
        budget: &mut Budget<'_>,
        observe: impl for<'scope> FnOnce(PreRankedSourceView<'scope>, &mut Budget<'_>) -> Result<R, E>,
    ) -> ResultV1<
        (
            R,
            Result<RankedVerifiedProductionCompilation, ProductionPipelineError>,
        ),
        E,
    > {
        let stage = self
            .import_semantic_mir()
            .map_err(ObservationError::Pipeline)?
            .construct_semantic_middle_end()
            .map_err(ObservationError::Pipeline)?
            .construct_semantic_ssa()
            .map_err(ObservationError::Pipeline)?
            .materialize_target_neutral()
            .map_err(|error| ObservationError::Pipeline(*error))?;
        let retained = stage.materialized.retained_analysis_storage_v1();
        let (stage, observation) =
            owned_scope(stage, retained, budget, |stage, custody, budget| {
                observe(PreRankedSourceView { stage, custody }, budget)
            })?;
        Ok((observation, stage.verify_general_kernel_checks()))
    }
}

#[test]
fn pre_ranked_observer_preserves_floor_and_refuses_swallowed_custody_failures() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(17).unwrap();
    let value = owned_scope(41_u32, 23, &mut budget, |owner, custody, budget| {
        custody.query(budget)?;
        Ok::<_, Resource>(*owner + 1)
    })
    .unwrap();
    assert_eq!(value, (41, 42));
    assert_eq!(budget.storage(), 17);
    for repair in [false, true] {
        let result = owned_scope(41_u32, 23, &mut budget, |_, custody, budget| {
            budget.release_storage(1).unwrap();
            assert_eq!(custody.query(budget), Err(Resource::Accounting));
            if repair {
                budget.reserve_storage(1).unwrap();
            }
            Ok::<_, Resource>(())
        });
        assert!(matches!(
            result,
            Err(ObservationError::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.storage(), 17);
    }
    let result = owned_scope(41_u32, 23, &mut budget, |_, custody, _| {
        let mut other_work = Work::new(1000);
        let mut other = Budget::new(&mut other_work, 100_000);
        other.reserve_storage(99_999).unwrap();
        assert_eq!(custody.query(&mut other), Err(Resource::Accounting));
        assert_eq!(other.work(), 0);
        Ok::<_, Resource>(())
    });
    assert!(matches!(
        result,
        Err(ObservationError::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), 17);
}

#[test]
fn pre_ranked_observer_rejects_same_ledger_foreign_slot_and_swallowed_work_error() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    let mut work = Work::new(0);
    let mut other_work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 100_000);
    let mut other = Budget::new(&mut other_work, 100_000);
    budget.reserve_storage(17).unwrap();
    other.reserve_storage(31).unwrap();
    let result = owned_scope(1_u32, 23, &mut budget, |_, custody, budget| {
        std::mem::swap(budget, &mut other);
        assert_eq!(custody.query(&mut other), Err(Resource::Accounting));
        std::mem::swap(budget, &mut other);
        Ok::<_, Resource>(())
    });
    assert!(matches!(
        result,
        Err(ObservationError::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), 17);
    assert_eq!(other.storage(), 31);
    assert_eq!(budget.work(), 0);
    let result = owned_scope(1_u32, 23, &mut budget, |_, custody, budget| {
        assert!(matches!(custody.query(budget), Err(Resource::Work(_))));
        Ok::<_, Resource>(())
    });
    assert!(matches!(
        result,
        Err(ObservationError::Resource(Resource::Work(_)))
    ));
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.work(), 0);
}

#[test]
fn pre_ranked_observer_callback_errors_keep_their_typed_source() {
    use std::error::Error;
    let error = ObservationError::Callback(Resource::Accounting);
    assert!(error.source().unwrap().downcast_ref::<Resource>().is_some());
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(17).unwrap();
    let error = owned_scope(
        1_u32,
        23,
        &mut budget,
        |_, custody, budget| -> Result<(), Resource> {
            custody.query(budget)?;
            Err(Resource::Allocation)
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ObservationError::Callback(Resource::Allocation)
    ));
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.work(), 1);
    let result = owned_scope(1_u32, 23, &mut budget, |_, _, budget| {
        budget.release_storage(1).unwrap();
        Ok::<_, Resource>(())
    });
    assert!(matches!(
        result,
        Err(ObservationError::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), 17);
    let error: ObservationError<Resource> = ObservationError::Resource(Resource::Arithmetic);
    assert!(error.source().unwrap().downcast_ref::<Resource>().is_some());
}

#[test]
fn pre_ranked_observer_exact_header_refusal_and_panic_cleanup() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    let paid = 23 + header::<u32, (), Resource>().unwrap();
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 17 + paid - 1);
    budget.reserve_storage(17).unwrap();
    let result = owned_scope(41_u32, 23, &mut budget, |_, _, _| -> Result<(), Resource> {
        panic!("must refuse before callback")
    });
    assert!(
        matches!(result, Err(ObservationError::Resource(Resource::Storage(limit)))
        if limit.actual() == 17 + paid && limit.limit() == 17 + paid - 1)
    );
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.peak_storage(), 17);
    assert_eq!(budget.failed_storage(), Some(17 + paid));
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(17).unwrap();
    let dropped = Cell::new(false);
    struct DropMark<'a>(&'a Cell<bool>);
    impl Drop for DropMark<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        owned_scope(
            DropMark(&dropped),
            23,
            &mut budget,
            |_, custody, budget| -> Result<(), Resource> {
                custody.query(budget)?;
                panic!("managed callback panic")
            },
        )
    }));
    assert!(result.is_err());
    assert!(dropped.get());
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.work(), 1);
}
