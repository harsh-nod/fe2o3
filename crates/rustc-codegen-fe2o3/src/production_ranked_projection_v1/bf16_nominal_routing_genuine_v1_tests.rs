//! B4 genuine Identity/Swap01 hook, plus explicitly isolated accounting probes.
//! No source fixture, ranked owner, numerical result or normal admission is made.
use super::super::{CanonicalAssertionErrorV1, ProjectedAssertionFactsV1};
use super::*;
use crate::production_ranked_projection_v1::{
    ProductionRankedProjectionErrorV1 as ProjectionError, ProjectedViewsV1,
    bf16_nominal_call_projection_v1::{
        CheckedNominalCallProjectionV1, RequiredNominalProjectionV1,
    },
    bf16_nominal_call_routing_v1::{
        DefinedCallAccessRouteV1, NOMINAL_PENDING_V1, require_defined_call_access_ready_v1,
        resolve_defined_call_access_route_v1, with_nominal_summary_v1,
    },
};
use fe2o3_kernel_analysis::CanonicalKirCallEffectDecisionV1;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, TensorLayoutContractV1};
use fe2o3_lower_mir_kernel::CheckedBf16CallInstanceV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCallableDeclV1, SemanticCallableIdV1, SemanticOperandV1, SemanticSourceProvenanceV1,
    SemanticTerminatorKindV1,
};
use std::cell::Cell;

const PROBE_WORK: usize = 8 * 1024 * 1024;
const PROBE_SCRATCH: usize = 8 * 1024 * 1024;
const TEST_HEADERS: usize = 4096;

fn from_projection<T>(result: std::result::Result<T, ProjectionError>) -> Result<T> {
    result.map_err(|error| match error {
        ProjectionError::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(error)) => {
            QueryError::Resource(error)
        }
        ProjectionError::CanonicalAssertions(CanonicalAssertionErrorV1::NominalCall(error)) => {
            error
        }
        other => panic!("unexpected nominal observation diagnostic: {other:?}"),
    })
}

// Own only this test's fixed stack/header envelope. Callback-owned storage is
// never refunded here; all diagnostics/panic payloads are dropped first.
fn with_headers<'w, R: Copy + 'static>(
    budget: &mut Budget<'w>,
    bytes: usize,
    body: impl FnOnce(&mut Budget<'w>) -> Result<R>,
) -> Result<R> {
    let ledger = budget.work_ledger_identity_v1();
    budget.reserve_storage(bytes)?;
    let protected = budget.storage();
    let outcome = catch_unwind(AssertUnwindSafe(|| body(budget)));
    let result = match outcome {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(QueryError::CallbackPanicked)
        }
    };
    if budget.work_ledger_identity_v1() != ledger || budget.storage() < protected {
        return Err(Resource::Accounting.into());
    }
    budget.release_storage(bytes)?;
    result
}

/// SAME complete path for the original-ledger observation and every boundary
/// probe. No test reservation precedes the first summary/N1 retained-floor check.
fn observe_route(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let ledger = budget.work_ledger_identity_v1();
    with_nominal_summary_v1(
        owner,
        inventory,
        source.root(),
        source.root(),
        source.call_block(),
        source.source_call(),
        budget,
        |summaries, budget| {
            with_headers(budget, TEST_HEADERS, |budget| {
                let semantic = owner.semantic_ssa().source_semantic();
                // Finite iteration/observation work is prepaid, including both
                // detailed and resolver callbacks' fixed assertions below.
                budget.charge_work(
                    semantic
                        .functions()
                        .len()
                        .checked_add(64)
                        .ok_or(Resource::Arithmetic)?,
                )?;
                for index in 0..semantic.functions().len() {
                    let function = SemanticFunctionIdV1::from_index(
                        u32::try_from(index).map_err(|_| Resource::Arithmetic)?,
                    );
                    assert_eq!(
                        summaries.is_nominal_tensor_requires_call(function),
                        function == source.helper()
                    );
                    assert!(!summaries.is_exact_empty(function));
                    assert!(!summaries.is_exact_empty_deterministic_scalar(function));
                }
                let block = source.call_block().index() as usize;
                let provenance = semantic.functions()[source.root().index() as usize].blocks()
                    [block]
                    .terminator()
                    .source();
                with_nominal_canonical_facts_observation_v1(
                    owner,
                    inventory,
                    source.root(),
                    source.root(),
                    source.call_block(),
                    source.source_call(),
                    budget,
                    |facts| {
                        assert!(std::ptr::eq(facts.owner, owner));
                        assert!(facts.report.belongs_to(inventory));
                        assert!(facts.budget.work_ledger_identity_v1() == ledger);
                        // No locals are consumed by this dispatch-only path:
                        // zero avoids a dummy/unmetered local-state allocation.
                        let mut views = ProjectedViewsV1::new(0, Some(facts));
                        let visits = Cell::new(0usize);
                        let mut inspect =
                            |candidate: &CheckedNominalCallProjectionV1<'_>,
                             budget: &mut Budget<'_>| {
                                budget.charge_work(64)?;
                                assert!(budget.work_ledger_identity_v1() == ledger);
                                assert_eq!(visits.replace(visits.get() + 1), 0);
                                let call = candidate.call();
                                let emission = call.emission();
                                assert!(call.belongs_to(inventory));
                                assert!(std::ptr::eq(call.source_call(), source.source_call()));
                                assert!(std::ptr::eq(emission.owner(), owner));
                                assert_eq!(emission.root(), source.root());
                                assert_eq!(emission.helper(), source.helper());
                                assert_eq!(emission.call_argument_components(0), Some(&[][..]));
                                assert_eq!(emission.formal_components(0), Some(&[][..]));
                                for index in 1..=3 {
                                    assert_eq!(
                                        emission.call_argument_components(index).unwrap().len(),
                                        4
                                    );
                                    assert_eq!(emission.formal_components(index).unwrap().len(), 4);
                                }
                                assert_eq!(call.helper().function.signature.parameters.len(), 12);
                                assert_eq!(call.helper().function.signature.results.len(), 4);
                                assert_eq!(call.call().operation.results.len(), 4);
                                assert_eq!(call.matrix().operation.results.len(), 4);
                                assert_eq!(
                                    candidate.required_result_permutation(),
                                    source.return_permutation()
                                );
                                assert_eq!(
                                    candidate.physical_effect_decision(),
                                    CanonicalKirCallEffectDecisionV1::CompleteEmpty
                                );
                                assert_eq!(candidate.required_projection(),
                                RequiredNominalProjectionV1::CallerCapabilitiesTensorLayoutFullWaveAndExactResults);
                                assert_eq!(
                                    candidate.required_tensor_contract(),
                                    TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
                                        .with_zero_filled_predicate_inputs()
                                );
                                let matrix_block = &semantic.functions()
                                    [source.helper().index() as usize]
                                    .blocks()
                                    [emission.source_matrix_block().index() as usize];
                                let SemanticTerminatorKindV1::Call(matrix) =
                                    matrix_block.terminator().kind()
                                else {
                                    panic!("actual helper Matrix source call absent")
                                };
                                assert!(std::ptr::eq(candidate.source_matrix_call(), matrix));
                                let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } =
                                    &semantic.callables()[matrix.callee().index() as usize]
                                else {
                                    panic!("actual helper MFMA intrinsic absent")
                                };
                                assert!(std::ptr::eq(
                                    candidate.source_matrix_intrinsic(),
                                    operation
                                ));
                                Ok(())
                            };
                        from_projection(views.with_nominal_call_v1(
                            block,
                            source.source_call(),
                            provenance,
                            &mut inspect,
                        ))?;
                        assert_eq!(visits.get(), 1);
                        // The actual production resolver performs a SEPARATE
                        // once-only visitor on the still-live original ledger.
                        let route = from_projection(resolve_defined_call_access_route_v1(
                            semantic.callables(),
                            summaries,
                            source.helper(),
                            block,
                            source.source_call(),
                            provenance,
                            &mut views,
                        ))?;
                        assert_eq!(route, DefinedCallAccessRouteV1::NominalPending);
                        assert!(matches!(require_defined_call_access_ready_v1(route),
                            Err(ProjectionError::Incomplete(detail)) if detail == NOMINAL_PENDING_V1));
                        drop(views);
                        Ok(())
                    },
                )
            })
        },
    )
}

#[derive(Debug)]
struct Probe {
    result: Result<()>,
    completed: bool,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}

fn boundary_probes(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    inventory_storage: usize,
    original: &mut Budget<'_>,
) -> Result<()> {
    let occurrence = owner
        .semantic_ssa()
        .occurrence_storage()
        .expect("genuine owner retains occurrence receipt")
        .retained_storage();
    let source_floor = owner
        .retained_analysis_storage_v1()
        .checked_add(occurrence)
        .ok_or(Resource::Arithmetic)?;
    let floor = source_floor
        .checked_add(inventory_storage)
        .ok_or(Resource::Arithmetic)?;
    let storage_limit = floor
        .checked_add(PROBE_SCRATCH)
        .ok_or(Resource::Arithmetic)?;
    assert!(original.storage() >= floor);
    // Sequential accounting-only probes. The genuine owners remain charged to
    // ORIGINAL throughout. No probe can authorize a source/ranked continuation.
    let envelope = PROBE_SCRATCH
        .checked_add(TEST_HEADERS)
        .and_then(|n| n.checked_add(8 * size_of::<Probe>()))
        .ok_or(Resource::Arithmetic)?;
    with_headers(original, envelope, |original| {
        original.charge_work(6 * PROBE_WORK + 64)?;
        let probe = |incoming: usize, work_limit: usize, storage_limit: usize| {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(incoming).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = observe_route(owner, source, inventory, &mut budget);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(
                budget.storage(),
                incoming,
                "route refunds only its owned scratch"
            );
            Probe {
                completed: result.is_ok(),
                result,
                work: budget.work(),
                peak: budget.peak_storage(),
                failed_work: budget.failed_work(),
                failed_storage: budget.failed_storage(),
            }
        };
        assert!(inventory_storage > 0);
        for incoming in [source_floor, floor - 1] {
            let refused = probe(incoming, PROBE_WORK, storage_limit);
            assert_eq!(refused.result, Err(Resource::Accounting.into()));
            assert!(!refused.completed);
            assert_eq!((refused.failed_work, refused.failed_storage), (None, None));
        }
        let measured = probe(floor, PROBE_WORK, storage_limit);
        assert_eq!(measured.result, Ok(()));
        assert!(measured.completed && measured.work > 0 && measured.peak > floor);
        assert_eq!(
            (measured.failed_work, measured.failed_storage),
            (None, None)
        );
        let exact = probe(floor, measured.work, measured.peak);
        assert_eq!(exact.result, Ok(()));
        assert!(exact.completed);
        assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
        assert_eq!((exact.failed_work, exact.failed_storage), (None, None));
        let short_work = probe(floor, measured.work - 1, measured.peak);
        assert!(matches!(
            short_work.result,
            Err(QueryError::Resource(Resource::Work(_)))
        ));
        assert!(!short_work.completed);
        assert!(short_work.failed_work.is_some());
        assert_eq!(short_work.failed_storage, None);
        let short_storage = probe(floor, measured.work, measured.peak - 1);
        assert!(matches!(
            short_storage.result,
            Err(QueryError::Resource(Resource::Storage(_)))
        ));
        assert!(!short_storage.completed);
        assert_eq!(short_storage.failed_work, None);
        assert!(short_storage.failed_storage.is_some());
        Ok(())
    })
}

fn source_rejections(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    for (root, caller, block) in [
        (
            SemanticFunctionIdV1::from_index(u32::MAX),
            source.root(),
            source.call_block(),
        ),
        (source.root(), source.helper(), source.call_block()),
        (
            source.root(),
            source.root(),
            SemanticBlockIdV1::from_index(u32::MAX),
        ),
    ] {
        let entered = Cell::new(false);
        let result = with_nominal_canonical_facts_observation_v1(
            owner,
            inventory,
            root,
            caller,
            block,
            source.source_call(),
            budget,
            |_| {
                entered.set(true);
                Ok(())
            },
        );
        assert!(matches!(result, Err(QueryError::Unavailable(_))));
        assert!(!entered.get());
    }
    with_nominal_canonical_facts_observation_v1(
        owner,
        inventory,
        source.root(),
        source.root(),
        source.call_block(),
        source.source_call(),
        budget,
        |facts| {
            facts.budget.charge_work(128)?;
            let semantic = owner.semantic_ssa().source_semantic();
            let block = source.call_block().index() as usize;
            let provenance = semantic.functions()[source.root().index() as usize].blocks()[block]
                .terminator()
                .source();
            let mut never = |_: &CheckedNominalCallProjectionV1<'_>, _: &mut Budget<'_>| {
                panic!("invalid genuine query must not visit")
            };
            if usize::BITS > 32 {
                assert!(matches!(
                    facts.with_nominal_call_v1(
                        usize::MAX,
                        source.source_call(),
                        provenance,
                        &mut never
                    ),
                    Err(ProjectionError::CanonicalAssertions(
                        CanonicalAssertionErrorV1::Resource(Resource::Arithmetic)
                    ))
                ));
            }
            let missing = SemanticSourceProvenanceV1::unavailable();
            assert_ne!(missing, provenance, "fixture must retain actual provenance");
            assert!(matches!(
                facts.with_nominal_call_v1(block, source.source_call(), missing, &mut never),
                Err(ProjectionError::CanonicalAssertions(
                    CanonicalAssertionErrorV1::NominalCall(QueryError::Unavailable(
                        "nominal facts source provenance differs"
                    ))
                ))
            ));
            // The real UnitLocal consumer must still refuse this nominal owner.
            assert!(
                facts
                    .require_unit_local_call(block, source.source_call(), provenance)
                    .is_err()
            );
            Ok(())
        },
    )?;
    // Clone only the fixture's already checked fixed unprojected operands.
    let call = source.source_call();
    budget.charge_work(16)?;
    assert_eq!(call.arguments().len(), 4);
    assert!(call.variadic_argument_abis().is_empty());
    for operand in call.arguments() {
        assert!(
            matches!(operand, SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p)
            if p.projections().is_empty())
        );
    }
    assert!(call.destination().unwrap().place().projections().is_empty());
    let clone_bytes = 2 * size_of::<SemanticDirectCallV1>() + 8 * size_of::<SemanticOperandV1>();
    with_headers(budget, clone_bytes, |budget| {
        let clone = call.clone();
        assert_eq!(clone, *call);
        let wrong = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(u32::MAX),
            call.arguments().to_vec(),
            call.destination().cloned(),
            call.unwind(),
        )
        .unwrap();
        for detached in [&clone, &wrong] {
            let entered = Cell::new(false);
            let result = with_nominal_canonical_facts_observation_v1(
                owner,
                inventory,
                source.root(),
                source.root(),
                source.call_block(),
                detached,
                budget,
                |_| {
                    entered.set(true);
                    Ok(())
                },
            );
            assert!(matches!(result, Err(QueryError::Unavailable(_))));
            assert!(!entered.get());
        }
        drop(wrong);
        drop(clone);
        Ok(())
    })?;
    Ok(())
}

fn callback_charge_controls(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    for outcome in 0..3 {
        let before = budget.storage();
        let work_before = budget.work();
        let ledger = budget.work_ledger_identity_v1();
        let result = with_nominal_canonical_facts_observation_v1(
            owner,
            inventory,
            source.root(),
            source.root(),
            source.call_block(),
            source.source_call(),
            budget,
            |facts| {
                facts.budget.reserve_storage(23)?;
                facts.budget.charge_work(17)?;
                match outcome {
                    0 => Ok(()),
                    1 => Err(QueryError::Unavailable("genuine facts callback refusal")),
                    _ => panic!("controlled genuine facts callback unwind"),
                }
            },
        );
        assert_eq!(
            result,
            match outcome {
                0 => Ok(()),
                1 => Err(QueryError::Unavailable("genuine facts callback refusal")),
                _ => Err(QueryError::CallbackPanicked),
            }
        );
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), before + 23);
        assert!(budget.work() >= work_before + 17);
        budget.release_storage(23)?;
    }
    Ok(())
}

fn custody_accounting_probes(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    inventory_storage: usize,
    original: &mut Budget<'_>,
) -> Result<()> {
    let occurrence = owner
        .semantic_ssa()
        .occurrence_storage()
        .unwrap()
        .retained_storage();
    let floor = owner
        .retained_analysis_storage_v1()
        .checked_add(occurrence)
        .and_then(|n| n.checked_add(inventory_storage))
        .ok_or(Resource::Arithmetic)?;
    let limit = floor
        .checked_add(PROBE_SCRATCH)
        .ok_or(Resource::Arithmetic)?;
    with_headers(original, PROBE_SCRATCH + TEST_HEADERS, |original| {
        original.charge_work(4 * PROBE_WORK + 64)?;
        for mode in 0..4 {
            let mut foreign_work = Work::new(PROBE_WORK);
            let mut work = Work::new(PROBE_WORK);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(floor)?;
            let ledger = budget.work_ledger_identity_v1();
            let protected = Cell::new(0);
            let entered = Cell::new(false);
            let mut replacement = Some(Budget::new(&mut foreign_work, limit));
            let result = with_nominal_canonical_facts_observation_v1(
                owner,
                inventory,
                source.root(),
                source.root(),
                source.call_block(),
                source.source_call(),
                &mut budget,
                |facts| {
                    entered.set(true);
                    protected.set(facts.budget.storage());
                    match mode {
                        0 => {
                            let _ = facts.budget.charge_work(PROBE_WORK + 1);
                        }
                        1 => {
                            let _ = facts.budget.reserve_storage(limit + 1);
                        }
                        2 => {
                            facts.budget.release_storage(1)?;
                        }
                        _ => {
                            *facts.budget = replacement.take().unwrap();
                        }
                    }
                    Ok(())
                },
            );
            assert!(entered.get());
            assert_eq!(result, Err(Resource::Accounting.into()));
            if mode == 3 {
                assert!(budget.work_ledger_identity_v1() != ledger);
                assert_eq!(budget.storage(), 0, "foreign account is not repaired");
                assert_eq!(budget.work(), 0, "foreign account is not charged");
                assert_eq!(
                    (budget.failed_work(), budget.failed_storage()),
                    (None, None)
                );
            } else {
                assert!(budget.work_ledger_identity_v1() == ledger);
                if mode < 2 {
                    assert_eq!(budget.storage(), floor);
                    assert_eq!(budget.failed_work().is_some(), mode == 0);
                    assert_eq!(budget.failed_storage().is_some(), mode == 1);
                } else {
                    // Scope itself leaves its damaged floor intact. Outer N1
                    // may refund only its OWN reservation; no floor repair.
                    assert!(budget.storage() >= floor);
                    assert!(budget.storage() < protected.get());
                    assert_eq!(
                        (budget.failed_work(), budget.failed_storage()),
                        (None, None)
                    );
                }
            }
        }
        Ok(())
    })
}

pub(crate) fn inspect_nominal_routing_genuine_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    inventory_storage: usize,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let before = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    observe_route(owner, source, inventory, budget)?;
    assert_eq!(budget.storage(), before);
    // Negative/control bookkeeping is separately prepaid on the original
    // phase after the first genuine whole route has completed.
    with_headers(budget, TEST_HEADERS, |budget| {
        budget.charge_work(256)?;
        source_rejections(owner, source, inventory, budget)?;
        callback_charge_controls(owner, source, inventory, budget)?;
        boundary_probes(owner, source, inventory, inventory_storage, budget)?;
        custody_accounting_probes(owner, source, inventory, inventory_storage, budget)?;
        Ok(())
    })?;
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.storage(), before);
    assert_eq!(
        (budget.failed_work(), budget.failed_storage()),
        (None, None)
    );
    Ok(())
}

#[test]
fn sparse_resource_mapping_is_typed_not_synthetic_allocation_success() {
    for resource in [
        Resource::Allocation,
        Resource::Arithmetic,
        Resource::Accounting,
    ] {
        assert_eq!(
            sparse_error(CanonicalKirSparseErrorV1::Resource(resource)),
            QueryError::Resource(resource)
        );
    }
    assert_eq!(
        sparse_error(CanonicalKirSparseErrorV1::InconsistentInventory),
        QueryError::Unavailable("nominal facts sparse inventory inconsistent")
    );
}

/** Uses the EXISTING pipeline negative-control executable and its real receipt.
 * Equal graph identity never authorizes the detached owner's inventory. */
pub(crate) fn inspect_foreign_nominal_facts_refusal_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    foreign_inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    with_headers(budget, TEST_HEADERS, |budget| {
        budget.charge_work(32)?;
        let entered = Cell::new(false);
        let result = with_nominal_canonical_facts_observation_v1(
            owner,
            foreign_inventory,
            source.root(),
            source.root(),
            source.call_block(),
            source.source_call(),
            budget,
            |_| {
                entered.set(true);
                Ok(())
            },
        );
        assert_eq!(
            result,
            Err(QueryError::Unavailable("foreign canonical inventory"))
        );
        assert!(!entered.get());
        Ok(())
    })
}
