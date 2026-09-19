//! Explicit post-proof diagnostic stop. No pending owner can resume compilation.
use super::ProductionRankedProjectionErrorV1;
use crate::production_conditional_reference_output_v1::{
    Error as JoinError, with_conditional_reference_output_v1,
};
use crate::production_reference_effect_join_v2::CompilerOwnedBoundReferenceEffectV2;
use crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1;
use dialect_kernel::{OwnershipCoverageAttr, OwnershipPartitionAttr};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_lower_mir_kernel::{
    NativeRankedSourceCandidateV1, ProductionConditionalSourceTranslationErrorV1 as Error,
    ProductionMirPlironTranslationErrorV1, ProductionPreRankedKirOwnerV1,
    ProductionRankedAccessSourceV1, ProductionRankedExecutableEffectSourceV1,
    ProductionSemanticKirErrorV1,
};
use fe2o3_pliron::{
    HierarchicalOwnershipFindingV1, MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1,
    ProductionConditionalOwnershipBlockerV1 as Blocker,
    ProductionConditionalOwnershipCheckV1 as Check, ProductionConditionalOwnershipSiteV1 as Site,
    ProductionConditionalRankedAnalysisV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
    ProductionRankedOperationV1 as Op, ProductionRankedValueV1,
};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

pub(crate) const STOP: &str = "test-only post-bind source observation stopped with pending checks";
pub(crate) const UNANNOTATED: &str = "test-only post-bind observation refused an unannotated root";
const SITES: usize = MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1;
#[path = "conditional_reference_negative_v1_tests.rs"]
mod negative_reference;
const EMPTY_SITE: Site = Site {
    block: 0,
    operation: 0,
    view: ProductionRankedValueV1::Argument(0),
};

pub(crate) struct BoundSourceV1<'a> {
    pub(crate) root: u32,
    pub(crate) rank: u8,
    pub(crate) access: &'a [ProductionRankedAccessSourceV1],
    pub(crate) effects: &'a [ProductionRankedExecutableEffectSourceV1],
    pub(crate) references: &'a AuthenticatedReferenceEffectBindingsV1,
    pub(crate) logical_name: &'a str,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Observation {
    pub(crate) kernel: String,
    pub(crate) canonical_digest: [u8; 32],
    pub(crate) selected: usize,
    pub(crate) signed_receipts: usize,
    pub(crate) memory_effects: usize,
    pub(crate) value_expressions: usize,
    pub(crate) pending_checks: usize,
    pub(crate) work: usize,
    pub(crate) output: OutputObservation,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct OutputObservation {
    pub(crate) source_argument: u32,
    pub(crate) adjusted_argument: u32,
    pub(crate) physical_argument: u32,
    pub(crate) raw_reference_argument: u32,
    pub(crate) element_bytes: u64,
    pub(crate) address_domain: String,
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
            .unwrap_or_else(|| Err("proof-bound construction was not observed".into()))
    });
    drop(restore);
    (result, observation)
}

pub(super) fn observe_bound(
    owner: &ProductionPreRankedKirOwnerV1,
    bound: CompilerOwnedBoundReferenceEffectV2,
    source: BoundSourceV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let floor = budget.storage();
    let inherited_work = budget.work();
    let ledger = budget.work_ledger_identity_v1();
    let result = with_sites(budget, |sites, budget| {
        let count = select_sites(bound.kernel_for_test_v1(), sites, budget)?;
        check_selection_budget(bound.kernel_for_test_v1(), &sites[..count]);
        let signed_address = bound.signed_receipts_for_test_v1().as_ptr();
        let signed_bytes: Vec<_> = bound
            .signed_receipts_for_test_v1()
            .iter()
            .map(|receipt| (*receipt.wire(), *receipt.verifying_key()))
            .collect();
        bound
            .into_staged()
            .map_err(|e| format!("{e:?}"))?
            .observe_for_test_v1(&sites[..count], |pending, signed| {
                assert_eq!(signed.len(), count);
                assert!(std::ptr::eq(signed.as_ptr(), signed_address));
                assert_eq!(signed.len(), signed_bytes.len());
                for (actual, (wire, key)) in signed.iter().zip(&signed_bytes) {
                    assert_eq!(actual.wire(), wire);
                    assert_eq!(actual.verifying_key(), key);
                }
                let candidate = NativeRankedSourceCandidateV1::from_untrusted_parts(
                    source.root,
                    source.rank,
                    pending.kernel().map_err(|e| format!("{e:?}"))?,
                    source.access,
                    source.effects,
                    "post-bind diagnostic only",
                );
                let mut observation = inspect(owner, pending, candidate, &source, budget)?;
                observation.signed_receipts = signed.len();
                Ok(observation)
            })
    });
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    ACTIVE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let active = slot.as_mut().expect("explicit post-bind observation scope");
        assert!(
            active.result.is_none(),
            "one actual source root per observation"
        );
        active.result = Some(result.map(|mut result| {
            result.work = budget.work() - inherited_work;
            result
        }));
    });
    Err(ProductionRankedProjectionErrorV1::Incomplete(STOP))
}

fn with_sites<R>(
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut [Site; SITES], &mut Budget<'_>) -> Result<R, String>,
) -> Result<R, String> {
    // Bounds selection payload only, not arenas, diagnostic snapshots or native stack usage.
    let scratch = std::mem::size_of::<[Site; SITES]>();
    budget.reserve_storage(scratch).map_err(|e| e.to_string())?;
    let result = catch_unwind(AssertUnwindSafe(|| run(&mut [EMPTY_SITE; SITES], budget)));
    let released = budget.release_storage(scratch).map_err(|e| e.to_string());
    match result {
        Ok(result) => {
            released?;
            result
        }
        Err(panic) => resume_unwind(panic),
    }
}

fn check_selection_budget(kernel: &ProductionRankedKernelV1, expected: &[Site]) {
    let storage = std::mem::size_of::<[Site; SITES]>() + 31;
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(31).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = with_sites(&mut budget, |sites, budget| {
            let count = select_sites(kernel, sites, budget)?;
            assert_eq!(&sites[..count], expected);
            Ok(())
        });
        assert_eq!(budget.storage(), 31);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (result, budget.work())
    };
    let (baseline, exact) = run(10_000_000, storage);
    baseline.unwrap();
    run(exact, storage).0.unwrap();
    assert!(run(exact - 1, storage).0.is_err());
    assert!(run(exact, storage - 1).0.is_err());
}

#[test]
fn selection_storage_is_restored_on_error_and_unwind() {
    let mut work = Work::new(10);
    let mut budget = Budget::new(&mut work, std::mem::size_of::<[Site; SITES]>() + 31);
    budget.reserve_storage(31).unwrap();
    assert!(with_sites::<()>(&mut budget, |_, _| Err("selection failure".into())).is_err());
    assert_eq!(budget.storage(), 31);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = with_sites::<()>(&mut budget, |_, _| panic!("selection panic"));
        }))
        .is_err()
    );
    assert_eq!(budget.storage(), 31);
}

#[test]
fn nested_observation_and_unwind_restore_the_prior_scope() {
    assert!(!is_active());
    reject_unannotated().unwrap();
    let (_, result) = observe(|| {
        assert!(matches!(
            reject_unannotated(),
            Err(ProductionRankedProjectionErrorV1::Incomplete(UNANNOTATED))
        ));
        assert!(observe(|| assert!(is_active())).1.is_err());
        assert!(is_active());
        assert!(catch_unwind(|| observe(|| panic!("observer panic"))).is_err());
        assert!(is_active());
    });
    assert!(result.is_err());
    assert!(!is_active());
}

fn select_sites(
    kernel: &ProductionRankedKernelV1,
    sites: &mut [Site; SITES],
    budget: &mut Budget<'_>,
) -> Result<usize, String> {
    let mut count = 0;
    for block in kernel.blocks() {
        budget.charge_work(1).map_err(|e| e.to_string())?;
        for operation in block.operations() {
            budget.charge_work(1).map_err(|e| e.to_string())?;
            let Op::RequireEffectRefinement { contract, .. } = operation else {
                continue;
            };
            if count == sites.len() {
                return Err("conditional selection limit exceeded".into());
            }
            sites[count] =
                select_ownership_site(kernel.blocks(), contract.view(), &sites[..count], budget)?;
            count += 1;
        }
    }
    if count == 0 {
        return Err("bound source has no refinement requests".into());
    }
    Ok(count)
}

fn select_ownership_site(
    blocks: &[ProductionRankedBlockV1],
    view: ProductionRankedValueV1,
    previous: &[Site],
    budget: &mut Budget<'_>,
) -> Result<Site, String> {
    let mut selected = None;
    for (b, block) in blocks.iter().enumerate() {
        budget.charge_work(1).map_err(|e| e.to_string())?;
        for (o, operation) in block.operations().iter().enumerate() {
            budget.charge_work(1).map_err(|e| e.to_string())?;
            let Op::OwnershipContract {
                view: actual,
                coverage,
                partition,
            } = operation
            else {
                continue;
            };
            if *actual != view {
                continue;
            }
            if selected.is_some()
                || *coverage != OwnershipCoverageAttr::TotalView
                || *partition != OwnershipPartitionAttr::ExactSets
            {
                return Err("refinement requires one exact total-view ownership occurrence".into());
            }
            selected = Some(Site {
                block: u32::try_from(b).map_err(|e| e.to_string())?,
                operation: u32::try_from(o).map_err(|e| e.to_string())?,
                view,
            });
        }
    }
    let selected = selected.ok_or("refinement has no ownership occurrence")?;
    for prior in previous {
        budget.charge_work(1).map_err(|e| e.to_string())?;
        if *prior == selected {
            return Err("duplicate conditional ownership selection".into());
        }
    }
    Ok(selected)
}

#[test]
fn ownership_selection_refuses_missing_duplicate_wrong_profile_and_repeated() {
    use OwnershipCoverageAttr::{ExactView, TotalView};
    use OwnershipPartitionAttr::{DenseRectangles, ExactSets};
    use fe2o3_pliron::{ProductionRankedTerminatorV1 as Term, ProductionRankedValueIdV1 as Id};
    let good = (TotalView, ExactSets);
    let malformed = "refinement requires one exact total-view ownership occurrence";
    for (profiles, repeat, expected) in [
        (vec![], false, "refinement has no ownership occurrence"),
        (vec![good, good], false, malformed),
        (vec![(ExactView, ExactSets)], false, malformed),
        (vec![(TotalView, DenseRectangles)], false, malformed),
        (
            vec![good],
            true,
            "duplicate conditional ownership selection",
        ),
    ] {
        // Inert constructor-valid roster, not an imported receipt or source transaction.
        let id = Id::new(0);
        let view = ProductionRankedValueV1::Local(id);
        let mut operations = vec![Op::View {
            result: id,
            element_width: 32,
            writable: true,
            shape: vec![1],
            dynamic_extents: vec![],
            allocation_origin: 1,
            noalias_class: 1,
        }];
        operations.extend(profiles.into_iter().map(|(coverage, partition)| {
            Op::OwnershipContract {
                view,
                coverage,
                partition,
            }
        }));
        let kernel = ProductionRankedKernelV1::new(
            "selection",
            0,
            vec![ProductionRankedBlockV1::new(operations, Term::Return)],
        )
        .unwrap();
        let mut work = Work::new(1_000);
        let mut budget = Budget::new(&mut work, 1_000);
        let mut prior = [EMPTY_SITE];
        let previous = if repeat {
            prior[0] = select_ownership_site(kernel.blocks(), view, &[], &mut budget).unwrap();
            assert_eq!(
                prior[0],
                Site {
                    block: 0,
                    operation: 1,
                    view
                }
            );
            &prior[..]
        } else {
            &[]
        };
        assert_eq!(
            select_ownership_site(kernel.blocks(), view, previous, &mut budget).unwrap_err(),
            expected
        );
    }
}

fn inspect(
    owner: &ProductionPreRankedKirOwnerV1,
    pending: &ProductionConditionalRankedAnalysisV1,
    candidate: NativeRankedSourceCandidateV1<'_>,
    source: &BoundSourceV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<Observation, String> {
    let [kernel] = owner.executable().module().kernels.as_slice() else {
        return Err("source fixture requires exactly one actual kernel".into());
    };
    let legacy = pending.legacy_report().clone();
    let rows = format!("{:?}", pending.rows());
    let bounds = pending.mandatory_bounds_failure().map(str::to_owned);
    let checks = pending.pending_pipeline_checks();
    let (memory_effects, value_expressions, output) = with_conditional_reference_output_v1(
        owner,
        pending,
        candidate,
        source.references,
        source.logical_name,
        budget,
        |joined| {
            let report = joined.translation();
            let output = joined.coverage().output();
            let binding = output.binding();
            assert!(std::ptr::eq(report.source(), owner));
            assert!(std::ptr::eq(report.pending(), pending));
            assert!(std::ptr::eq(
                output.candidate().kernel(),
                candidate.kernel()
            ));
            assert!(std::ptr::eq(joined.ownership(), &pending.selections()[0]));
            assert!(
                source
                    .references
                    .as_slice()
                    .iter()
                    .any(|reference| { std::ptr::eq(reference, joined.reference_binding()) })
            );
            assert_eq!(joined.reference_write().argument, binding.source_argument());
            assert!(report.memory_effects() > 0 && report.value_expressions() > 0);
            Ok((
                report.memory_effects(),
                report.value_expressions(),
                OutputObservation {
                    source_argument: binding.source_argument(),
                    adjusted_argument: binding.adjusted_argument(),
                    physical_argument: binding.coverage().output_parameter_index(),
                    raw_reference_argument: joined.raw_reference_argument(),
                    element_bytes: binding.coverage().element_bytes(),
                    address_domain: format!("{:?}", output.address_domain()),
                },
            ))
        },
    )
    .map_err(|error| error.to_string())?;
    assert!(!legacy.is_clean());
    assert!(legacy.findings().iter().any(|finding| matches!(
        finding,
        HierarchicalOwnershipFindingV1::TraceIncomplete { .. }
    )));
    assert_eq!(legacy.coverage_summary().total_view_declared(), 1);
    assert_eq!(legacy.coverage_summary().total_view_proved(), 0);
    assert_eq!(pending.selections().len(), 1);
    assert!(
        pending
            .rows()
            .iter()
            .filter(|row| row.selected())
            .all(|row| row.coverage() == Check::Blocked(Blocker::CanonicalCoverageAndSourceReplay))
    );
    assert!(!checks.is_empty());
    for (root, rank) in [
        (u32::MAX, candidate.launch_rank()),
        (candidate.semantic_root(), 2),
    ] {
        let changed = NativeRankedSourceCandidateV1::from_untrusted_parts(
            root,
            rank,
            candidate.kernel(),
            candidate.access_sources(),
            candidate.executable_effect_sources(),
            "",
        );
        assert!(matches!(
            owner.check_conditional_source_translation_v1(pending, changed, budget),
            Err(Error::SourceAssociation)
        ));
    }
    check_budget(owner, pending, candidate);
    check_join_budget(owner, pending, candidate, source);
    negative_reference::check(owner, pending, candidate, source);
    assert_eq!(*pending.legacy_report(), legacy);
    assert_eq!(format!("{:?}", pending.rows()), rows);
    assert_eq!(pending.mandatory_bounds_failure(), bounds.as_deref());
    assert_eq!(pending.pending_pipeline_checks(), checks);
    Ok(Observation {
        kernel: kernel.id.as_str().to_owned(),
        canonical_digest: *owner.executable().canonical().identity().digest(),
        selected: pending.selections().len(),
        signed_receipts: 0,
        memory_effects,
        value_expressions,
        pending_checks: checks.len(),
        work: 0,
        output,
    })
}

fn check_join_budget(
    owner: &ProductionPreRankedKirOwnerV1,
    pending: &ProductionConditionalRankedAnalysisV1,
    candidate: NativeRankedSourceCandidateV1<'_>,
    source: &BoundSourceV1<'_>,
) {
    let floor = owner.retained_analysis_storage_v1();
    let run = |limit, missing_floor| {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, floor + 1024 * 1024);
        let retained = floor - usize::from(missing_floor);
        budget.reserve_storage(retained).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let mut callbacks = 0;
        let result = with_conditional_reference_output_v1(
            owner,
            pending,
            candidate,
            source.references,
            source.logical_name,
            &mut budget,
            |_| {
                callbacks += 1;
                Ok(())
            },
        );
        assert_eq!(callbacks, usize::from(result.is_ok()));
        assert_eq!(budget.storage(), retained);
        assert!(budget.work_ledger_identity_v1() == ledger);
        let used = budget.work();
        (result, used, work.failed_work())
    };
    let (baseline, exact, failed) = run(10_000_000, false);
    baseline.unwrap();
    assert_eq!(failed, None);
    run(exact, false).0.unwrap();
    let (short, _, failed) = run(exact - 1, false);
    assert!(
        matches!(short, Err(JoinError::Resource(Resource::Work(_)))),
        "{short:?}"
    );
    assert_eq!(failed, Some(exact));
    assert!(matches!(run(exact, true).0, Err(JoinError::Source(_))));
}

fn check_budget(
    owner: &ProductionPreRankedKirOwnerV1,
    pending: &ProductionConditionalRankedAnalysisV1,
    candidate: NativeRankedSourceCandidateV1<'_>,
) {
    let floor = owner.retained_analysis_storage_v1();
    assert!(floor > 0);
    // Independent calibration probes carry prior work; the live ledger is never reset.
    let run = |limit, missing_floor| {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, floor + 1024 * 1024);
        let retained = floor - usize::from(missing_floor);
        budget.reserve_storage(retained).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = owner
            .check_conditional_source_translation_v1(pending, candidate, &mut budget)
            .map(|_| ());
        assert_eq!(budget.storage(), retained);
        assert!(budget.work_ledger_identity_v1() == ledger);
        let used = budget.work();
        (result, used, work.failed_work())
    };
    let (baseline, exact, failed) = run(10_000_000, false);
    baseline.unwrap();
    assert_eq!(failed, None);
    run(exact, false).0.unwrap();
    let (short, _, failed) = run(exact - 1, false);
    assert!(
        matches!(
            short,
            Err(Error::Correspondence(
                ProductionSemanticKirErrorV1::MirPlironTranslation(
                    ProductionMirPlironTranslationErrorV1::ResourceLimit
                )
            ))
        ),
        "{short:?}"
    );
    assert_eq!(failed, Some(exact));
    assert!(matches!(
        run(exact, true).0,
        Err(Error::Correspondence(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(Resource::Accounting)
        ))
    ));
}
