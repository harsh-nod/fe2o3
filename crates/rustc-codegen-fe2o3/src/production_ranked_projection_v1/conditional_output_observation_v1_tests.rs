//! Test-only observation of the same live canonical owner and prepared request.
//! An active observer always stops before proof execution, never admits a root.
use super::ProductionRankedProjectionErrorV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, ConditionalTotalViewAnalysisV1,
    derive_conditional_total_view_from_verified_v1,
};
use fe2o3_lower_mir_kernel::{
    NativeRankedSourceCandidateV1, ProductionConditionalOutputBindingV1,
    ProductionConditionalRankedExtentV1, ProductionConditionalRankedOutputErrorV1 as Error,
    ProductionPreRankedKirOwnerV1,
};
use fe2o3_pliron::ProductionRankedValueV1;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

pub(crate) const STOP: &str =
    "test-only conditional output observation stopped before proof execution";
pub(crate) const UNANNOTATED: &str = "test-only conditional output observation stopped an unannotated root before ranked compilation";

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Observation {
    pub(crate) kernel: String,
    pub(crate) canonical_digest: [u8; 32],
    pub(crate) root: u32,
    pub(crate) source_argument: u32,
    pub(crate) physical_parameter: u32,
    pub(crate) reference_argument: u32,
    pub(crate) ranked_block: u32,
    pub(crate) ranked_operation: u32,
    pub(crate) ranked_extent_argument: u32,
    pub(crate) canonical_length_value: u32,
    pub(crate) address_domain: String,
    pub(crate) work: usize,
}

#[derive(Default)]
struct Active {
    result: Option<Result<Observation, String>>,
}
thread_local! {
    static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
}
struct Restore(Option<Active>);
impl Drop for Restore {
    fn drop(&mut self) {
        ACTIVE.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}
pub(crate) fn is_active() -> bool {
    ACTIVE.with(|slot| slot.borrow().is_some())
}
pub(super) fn reject_unannotated() -> Result<(), ProductionRankedProjectionErrorV1> {
    if is_active() {
        Err(ProductionRankedProjectionErrorV1::Incomplete(UNANNOTATED))
    } else {
        Ok(())
    }
}
pub(crate) fn observe<R>(run: impl FnOnce() -> R) -> (R, Result<Observation, String>) {
    let restore = Restore(ACTIVE.with(|slot| slot.replace(Some(Active::default()))));
    let result = run();
    let observation = ACTIVE.with(|slot| {
        slot.borrow_mut()
            .take()
            .unwrap()
            .result
            .unwrap_or_else(|| Err("prepared request was not observed".into()))
    });
    drop(restore);
    (result, observation)
}

pub(super) fn observe_candidate(
    owner: &ProductionPreRankedKirOwnerV1,
    candidate: NativeRankedSourceCandidateV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    ACTIVE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let active = slot.as_mut().expect("explicit observation scope");
        assert!(
            active.result.is_none(),
            "exactly one prepared root in this fixture"
        );
        active.result = Some(inspect(owner, candidate, budget));
    });
    Err(ProductionRankedProjectionErrorV1::Incomplete(STOP))
}

fn inspect(
    owner: &ProductionPreRankedKirOwnerV1,
    candidate: NativeRankedSourceCandidateV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<Observation, String> {
    let floor = budget.storage();
    let work = budget.work();
    let ledger = budget.work_ledger_identity_v1();
    let result = (|| {
        let [kernel] = owner.executable().module().kernels.as_slice() else {
            return Err("source test requires exactly one actual kernel".into());
        };
        let ConditionalTotalViewAnalysisV1::Established(facts) =
            derive_conditional_total_view_from_verified_v1(
                owner.executable().verified_module_ref_v1(),
                &kernel.id,
                budget,
            )
            .map_err(|e| e.to_string())?
        else {
            return Err("actual canonical output does not establish conditional coverage".into());
        };
        let binding = owner
            .bind_conditional_output_v1(facts, budget)
            .map_err(|e| e.to_string())?;
        let joined = binding
            .inspect_ranked_output_v1(candidate, budget)
            .and_then(|joined| joined.rederive_output_extent_v1(budget))
            .map_err(|e| e.to_string())?;
        assert!(std::ptr::eq(joined.binding().owner(), owner));
        assert!(std::ptr::eq(
            joined.candidate().kernel(),
            candidate.kernel()
        ));
        let ProductionConditionalRankedExtentV1::CanonicalOutputLength {
            operand: ProductionRankedValueV1::Argument(ranked_extent_argument),
            length,
        } = joined.dynamic_extent()
        else {
            return Err("source extent rederivation did not retain the exact relation".into());
        };
        assert_eq!(length, binding.coverage().length());
        check_extent_budget(&binding, candidate);
        Ok(Observation {
            kernel: kernel.id.as_str().to_owned(),
            canonical_digest: *owner.executable().canonical().identity().digest(),
            root: candidate.semantic_root(),
            source_argument: binding.source_argument(),
            physical_parameter: binding.coverage().output_parameter_index(),
            reference_argument: joined.contract().reference_output_site().argument(),
            ranked_block: joined.gpu_write_site().block(),
            ranked_operation: joined.gpu_write_site().operation(),
            ranked_extent_argument,
            canonical_length_value: length.0,
            address_domain: format!("{:?}", joined.address_domain()),
            work: budget.work() - work,
        })
    })();
    assert_eq!(
        budget.storage(),
        floor,
        "shared phase storage must be restored"
    );
    assert!(budget.work_ledger_identity_v1() == ledger);
    result
}

fn check_extent_budget(
    binding: &ProductionConditionalOutputBindingV1<'_>,
    candidate: NativeRankedSourceCandidateV1<'_>,
) {
    let floor = binding.owner().retained_analysis_storage_v1();
    assert!(floor > 0);
    // Separate test measurements, not replacement ledgers in the production
    // phase. Each measurement carries its inherited work through both queries.
    let run = |work_limit, lose_reservation| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, floor);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let joined = binding
            .inspect_ranked_output_v1(candidate, &mut budget)
            .unwrap();
        if lose_reservation {
            budget.release_storage(1).unwrap();
        }
        let result = joined.rederive_output_extent_v1(&mut budget).map(|_| ());
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), floor - usize::from(lose_reservation));
        assert_eq!(budget.peak_storage(), floor);
        (result, budget.work())
    };
    let (baseline, exact_work) = run(1_000_000, false);
    baseline.unwrap();
    run(exact_work, false).0.unwrap();
    assert!(matches!(
        run(exact_work - 1, false).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        run(exact_work, true).0,
        Err(Error::Resource(Resource::Accounting))
    ));
}

#[test]
fn inactive_or_unreached_observer_cannot_supply_a_result() {
    assert!(!is_active());
    let (value, result) = observe(|| 19);
    assert_eq!(value, 19);
    assert!(result.is_err());
    assert!(!is_active());
}

#[test]
fn active_observer_refuses_unannotated_roots_before_ranked_compilation() {
    assert!(reject_unannotated().is_ok());
    let (result, observation) = observe(reject_unannotated);
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Incomplete(UNANNOTATED))
    ));
    assert!(observation.is_err());
    assert!(reject_unannotated().is_ok());
}

#[test]
fn observer_scope_restores_after_unwind_and_nesting() {
    let (_, result) = observe(|| {
        let panic = std::panic::catch_unwind(|| observe(|| panic!("test-only unwind")));
        assert!(panic.is_err());
        assert!(is_active());
    });
    assert!(result.is_err());
    assert!(!is_active());
}
