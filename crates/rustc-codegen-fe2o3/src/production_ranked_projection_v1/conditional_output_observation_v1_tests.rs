//! Test-only observation of the same live canonical owner and prepared request.
//! An active observer always stops before proof execution, never admits a root.
use super::ProductionRankedProjectionErrorV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, ConditionalTotalViewAnalysisV1,
    derive_conditional_total_view_from_verified_v1,
};
use fe2o3_lower_mir_kernel::{NativeRankedSourceCandidateV1, ProductionPreRankedKirOwnerV1};
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
    pub(crate) dynamic_extent: String,
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
            .map_err(|e| e.to_string())?;
        assert!(std::ptr::eq(joined.binding().owner(), owner));
        assert!(std::ptr::eq(
            joined.candidate().kernel(),
            candidate.kernel()
        ));
        Ok(Observation {
            kernel: kernel.id.as_str().to_owned(),
            canonical_digest: *owner.executable().canonical().identity().digest(),
            root: candidate.semantic_root(),
            source_argument: binding.source_argument(),
            physical_parameter: binding.coverage().output_parameter_index(),
            reference_argument: joined.contract().reference_output_site().argument(),
            ranked_block: joined.gpu_write_site().block(),
            ranked_operation: joined.gpu_write_site().operation(),
            dynamic_extent: format!("{:?}", joined.dynamic_extent()),
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
