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
    ProductionConditionalRankedCoverageErrorV1 as CoverageError,
    ProductionConditionalRankedExtentV1, ProductionConditionalRankedOutputErrorV1 as Error,
    ProductionPreRankedKirOwnerV1,
};
use fe2o3_pliron::{ProductionRankedTerminatorV1, ProductionRankedValueV1};
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
    pub(crate) ranked_true_exit: u32,
    pub(crate) ranked_false_exit: u32,
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
        let facts = match derive_conditional_total_view_from_verified_v1(
            owner.executable().verified_module_ref_v1(),
            &kernel.id,
            budget,
        )
        .map_err(|e| e.to_string())?
        {
            ConditionalTotalViewAnalysisV1::Established(facts) => facts,
            ConditionalTotalViewAnalysisV1::Unsupported(reason) => {
                return Err(format!(
                    "actual canonical output does not establish conditional coverage: {reason:?}"
                ));
            }
        };
        let binding = owner
            .bind_conditional_output_v1(facts, budget)
            .map_err(|e| e.to_string())?;
        check_transported_extent(&binding, candidate);
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
        let coverage = joined
            .check_ranked_coverage_v1(budget)
            .map_err(|e| e.to_string())?;
        for exit in [coverage.true_exit_block(), coverage.false_exit_block()] {
            assert!(matches!(
                candidate.kernel().blocks()[exit as usize].terminator(),
                ProductionRankedTerminatorV1::Return
            ));
        }
        let joined = coverage.output();
        assert!(std::ptr::eq(joined.binding().owner(), owner));
        assert!(std::ptr::eq(
            joined.candidate().kernel(),
            candidate.kernel()
        ));
        check_coverage_budget(&binding, candidate);
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
            ranked_true_exit: coverage.true_exit_block(),
            ranked_false_exit: coverage.false_exit_block(),
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

fn check_transported_extent(
    binding: &ProductionConditionalOutputBindingV1<'_>,
    candidate: NativeRankedSourceCandidateV1<'_>,
) {
    use fe2o3_lower_mir_kernel::{
        ProductionRankedAccessSourceV1 as Access, ProductionRankedOutputExtentSourceV1 as Extent,
        decode_production_ranked_source_rows_v1, encode_production_ranked_source_rows_v1,
    };
    let floor = binding.owner().retained_analysis_storage_v1();
    for mutation in 0..6 {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, floor + 1_000_000);
        budget.reserve_storage(floor).unwrap();
        let joined = binding
            .inspect_ranked_output_v1(candidate, &mut budget)
            .unwrap();
        assert!(matches!(
            joined.dynamic_extent(),
            ProductionConditionalRankedExtentV1::Unbound(_)
        ));
        let site = joined.gpu_write_site();
        let mut rows = candidate.access_sources().to_vec();
        let row = rows
            .iter_mut()
            .find(|row| {
                row.ranked_block() == site.block() && row.ranked_operation() == site.operation()
            })
            .unwrap();
        let proposal = row.output_extent().unwrap();
        let base = Access::new(
            row.semantic_block(),
            row.semantic_statement(),
            row.semantic_access_ordinal(),
            row.ranked_block(),
            row.ranked_operation(),
        );
        *row = match mutation {
            0 => *row,
            1 => base,
            _ => base.with_output_extent(Extent::new(
                if mutation == 2 {
                    proposal.source_argument().wrapping_add(1)
                } else {
                    proposal.source_argument()
                },
                if mutation == 3 {
                    ProductionRankedValueV1::Argument(u32::MAX)
                } else {
                    proposal.view()
                },
                if mutation == 4 {
                    ProductionRankedValueV1::Argument(u32::MAX)
                } else {
                    proposal.extent()
                },
                if mutation == 5 {
                    ProductionRankedValueV1::Argument(u32::MAX)
                } else {
                    proposal.index()
                },
            )),
        };
        let (bytes, encoded_storage) = encode_production_ranked_source_rows_v1(
            &rows,
            candidate.executable_effect_sources(),
            &mut budget,
        )
        .unwrap();
        budget
            .reserve_storage(encoded_storage.retained_storage())
            .unwrap();
        drop(rows);
        let (decoded, storage) =
            decode_production_ranked_source_rows_v1(&bytes, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let recovered = NativeRankedSourceCandidateV1::from_untrusted_parts(
            candidate.semantic_root(),
            candidate.launch_rank(),
            candidate.kernel(),
            decoded.access_sources(),
            decoded.executable_effect_sources(),
            candidate.ranked_ir(),
        );
        let joined = binding
            .inspect_ranked_output_v1(recovered, &mut budget)
            .unwrap();
        assert!(matches!(
            joined.dynamic_extent(),
            ProductionConditionalRankedExtentV1::Unbound(_)
        ));
        let result = joined.rederive_output_extent_v1(&mut budget);
        match mutation {
            0 => {
                let joined = result.unwrap();
                assert_eq!(
                    joined.dynamic_extent(),
                    ProductionConditionalRankedExtentV1::CanonicalOutputLength {
                        operand: proposal.extent(),
                        length: binding.coverage().length()
                    }
                );
                assert!(std::ptr::eq(joined.binding().owner(), binding.owner()));
                check_extent_budget(binding, recovered);
            }
            1 => assert!(matches!(result, Err(Error::MissingExtentSource))),
            _ => assert!(matches!(result, Err(Error::ExtentSource))),
        }
        drop(decoded);
        budget.release_storage(storage.retained_storage()).unwrap();
        drop(bytes);
        budget
            .release_storage(encoded_storage.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

fn check_extent_budget(
    binding: &ProductionConditionalOutputBindingV1<'_>,
    candidate: NativeRankedSourceCandidateV1<'_>,
) {
    let floor = binding.owner().retained_analysis_storage_v1();
    assert!(floor > 0);
    // Separate test measurements, not replacement ledgers in the production
    // phase. Each measurement carries its inherited work through both queries.
    let run = |work_limit, storage_limit, lose_reservation| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let joined = binding.inspect_ranked_output_v1(candidate, &mut budget);
        if lose_reservation {
            budget.release_storage(1).unwrap();
        }
        let result = joined
            .and_then(|joined| joined.rederive_output_extent_v1(&mut budget))
            .map(|_| ());
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), floor - usize::from(lose_reservation));
        (result, budget.work(), budget.peak_storage())
    };
    let (baseline, exact_work, exact_storage) = run(1_000_000, usize::MAX, false);
    baseline.unwrap();
    run(exact_work, exact_storage, false).0.unwrap();
    assert!(matches!(
        run(exact_work - 1, exact_storage, false).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        run(exact_work, exact_storage, true).0,
        Err(Error::Resource(Resource::Accounting))
    ));
    if exact_storage > floor {
        assert!(matches!(
            run(exact_work, exact_storage - 1, false).0,
            Err(Error::Resource(Resource::Storage(_)))
        ));
    }
}

fn check_coverage_budget(
    binding: &ProductionConditionalOutputBindingV1<'_>,
    candidate: NativeRankedSourceCandidateV1<'_>,
) {
    let floor = binding.owner().retained_analysis_storage_v1();
    let run = |work_limit, storage_limit, lose_reservation| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let joined = binding
            .inspect_ranked_output_v1(candidate, &mut budget)
            .map_err(CoverageError::from);
        if lose_reservation {
            budget.release_storage(1).unwrap();
        }
        let result = joined
            .and_then(|joined| joined.check_ranked_coverage_v1(&mut budget))
            .map(|_| ());
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), floor - usize::from(lose_reservation));
        (result, budget.work(), budget.peak_storage())
    };
    let (baseline, exact_work, exact_storage) = run(1_000_000, usize::MAX, false);
    baseline.unwrap();
    run(exact_work, exact_storage, false).0.unwrap();
    assert!(matches!(
        run(exact_work - 1, exact_storage, false).0,
        Err(CoverageError::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        run(exact_work, exact_storage, true).0,
        Err(CoverageError::Resource(Resource::Accounting))
    ));
    if exact_storage > floor {
        assert!(matches!(
            run(exact_work, exact_storage - 1, false).0,
            Err(CoverageError::Resource(Resource::Storage(_)))
        ));
    }
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
