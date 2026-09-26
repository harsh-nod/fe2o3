//! Controls on the actual live source-emitted nominal owner. Unlike the
//! synthetic lowerer graph controls, these run for the genuine Identity/Swap01
//! frontend sessions. They do not alter the continued normal-compilation refusal.
use super::*;
use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
use fe2o3_lower_mir_kernel::{
    Bf16NominalCallQueryErrorV1 as QueryError, CheckedBf16CallInstanceV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1, SemanticCallDestinationV1, SemanticCallableIdV1, SemanticControlFlowEdgeV1,
    SemanticDirectCallV1, SemanticFunctionIdV1, SemanticOperandV1,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn query_error(error: QueryError) -> Error {
    match error {
        QueryError::Resource(error) => Error::Resource(error),
        QueryError::Unavailable(detail) => Error::Unavailable(detail),
        QueryError::CallbackPanicked => Error::CallbackPanicked,
    }
}

// These are isolated ACCOUNTING TEST ledgers, not the source/numerical phase
// ledger or replacement authority. The actual immutable owner/inventory stay
// reserved on the original phase budget throughout. This caller prepays the
// complete finite work ceiling and simultaneous scratch on that original budget.
const QUERY_BOUNDARY_PROBE_WORK_V1: usize = 8 * 1024 * 1024;
const QUERY_BOUNDARY_PROBE_SCRATCH_V1: usize = 64 * 1024;

fn inspect_retained_floor_boundaries(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    inventory_storage: usize,
    original: &mut Budget<'_>,
) -> Result<(), Error> {
    struct Probe {
        result: Result<(), QueryError>,
        entered: bool,
        work: usize,
        peak: usize,
        failed_work: Option<usize>,
        failed_storage: Option<usize>,
    }
    let occurrence = owner
        .semantic_ssa()
        .occurrence_storage()
        .expect("genuine materialization retains its preexisting occurrence receipt")
        .retained_storage();
    let source_floor = owner
        .retained_analysis_storage_v1()
        .checked_add(occurrence)
        .ok_or(Resource::Arithmetic)?;
    let complete_floor = source_floor
        .checked_add(inventory_storage)
        .ok_or(Resource::Arithmetic)?;
    let probe_storage_limit = complete_floor
        .checked_add(QUERY_BOUNDARY_PROBE_SCRATCH_V1)
        .ok_or(Resource::Arithmetic)?;
    // All six probes run sequentially; no second graph/inventory is allocated.
    // Include simultaneous test-result/header copies rather than charging them
    // to the already live source owner or the query's private reservation.
    let prepaid_storage = QUERY_BOUNDARY_PROBE_SCRATCH_V1
        + 8 * std::mem::size_of::<Probe>()
        + std::mem::size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1>()
        + std::mem::size_of::<Budget<'static>>()
        + std::mem::size_of::<Cell<bool>>();
    let before = original.storage();
    let ledger = original.work_ledger_identity_v1();
    original.reserve_storage(prepaid_storage)?;
    let protected = original.storage();
    let outcome = catch_unwind(AssertUnwindSafe(|| -> Result<(), Error> {
        original.charge_work(6 * QUERY_BOUNDARY_PROBE_WORK_V1 + 32)?;
        assert!(inventory_storage > 0);
        assert!(before >= complete_floor);
        assert!(inventory.belongs_to(owner.executable()));
        let probe = |incoming: usize, work_limit: usize, storage_limit: usize| {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget
                .reserve_storage(incoming)
                .expect("probe's explicit incoming floor fits");
            let identity = budget.work_ledger_identity_v1();
            let entered = Cell::new(false);
            let result = owner.with_checked_bf16_nominal_call_v1(
                inventory,
                source.root(),
                source.root(),
                source.call_block(),
                source.source_call(),
                &mut budget,
                |_, _| {
                    entered.set(true);
                    Ok(())
                },
            );
            assert!(budget.work_ledger_identity_v1() == identity);
            assert_eq!(
                budget.storage(),
                incoming,
                "public query refunds only its own scratch"
            );
            Probe {
                result,
                entered: entered.get(),
                work: budget.work(),
                peak: budget.peak_storage(),
                failed_work: budget.failed_work(),
                failed_storage: budget.failed_storage(),
            }
        };
        // These are the real public method and genuine borrowed source Call,
        // not the synthetic scope/component helper used by the unit controls.
        for incoming in [source_floor, complete_floor - 1] {
            let refused = probe(incoming, QUERY_BOUNDARY_PROBE_WORK_V1, probe_storage_limit);
            assert_eq!(
                refused.result,
                Err(QueryError::Resource(Resource::Accounting))
            );
            assert!(!refused.entered);
            assert_eq!(refused.failed_work, None);
            assert_eq!(refused.failed_storage, None);
        }
        let measured = probe(
            complete_floor,
            QUERY_BOUNDARY_PROBE_WORK_V1,
            probe_storage_limit,
        );
        assert_eq!(measured.result, Ok(()));
        assert!(measured.entered);
        assert!(measured.work > 0);
        assert!(measured.peak > complete_floor);
        assert_eq!(measured.failed_work, None);
        assert_eq!(measured.failed_storage, None);
        let exact = probe(complete_floor, measured.work, measured.peak);
        assert_eq!(exact.result, Ok(()));
        assert!(exact.entered);
        assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
        assert_eq!(exact.failed_work, None);
        assert_eq!(exact.failed_storage, None);
        let short_work = probe(complete_floor, measured.work - 1, measured.peak);
        assert!(matches!(
            short_work.result,
            Err(QueryError::Resource(Resource::Work(_)))
        ));
        assert!(!short_work.entered);
        assert!(short_work.failed_work.is_some());
        assert_eq!(short_work.failed_storage, None);
        let short_storage = probe(complete_floor, measured.work, measured.peak - 1);
        assert!(matches!(
            short_storage.result,
            Err(QueryError::Resource(Resource::Storage(_)))
        ));
        assert!(!short_storage.entered);
        assert_eq!(short_storage.failed_work, None);
        assert!(short_storage.failed_storage.is_some());
        Ok(())
    }));
    let result = match outcome {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::CallbackPanicked)
        }
    };
    if original.work_ledger_identity_v1() != ledger || original.storage() < protected {
        drop(result);
        return Err(Error::Resource(Resource::Accounting));
    }
    original.release_storage(prepaid_storage)?;
    assert_eq!(original.storage(), before);
    result
}

pub(super) fn inspect(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    let entry = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let (inventory, storage) = CanonicalKirInventoryV1::derive(owner.executable(), budget)
        .expect("genuine same-owner inventory derives");
    budget.reserve_storage(storage.retained_storage())?;
    let inventory_floor = budget.storage();
    let outcome = catch_unwind(AssertUnwindSafe(|| -> Result<(), Error> {
        let observed = owner
            .with_checked_bf16_nominal_call_v1(
                &inventory,
                source.root(),
                source.root(),
                source.call_block(),
                source.source_call(),
                budget,
                |view, budget| {
                    budget.charge_work(32)?;
                    assert!(view.belongs_to(&inventory));
                    assert!(std::ptr::eq(view.source_call(), source.source_call()));
                    assert!(std::ptr::eq(view.emission().owner(), owner));
                    assert_eq!(view.emission().root(), source.root());
                    assert_eq!(view.emission().helper(), source.helper());
                    assert_eq!(
                        view.emission().call_site().0,
                        owner
                            .bf16_call_instance_emission_v1()
                            .unwrap()
                            .call_site()
                            .0
                    );
                    assert_eq!(view.emission().call_argument_components(0), Some(&[][..]));
                    assert_eq!(view.emission().formal_components(0), Some(&[][..]));
                    for i in 1..=3 {
                        assert_eq!(
                            view.emission().call_argument_components(i).unwrap().len(),
                            4
                        );
                        assert_eq!(view.emission().formal_components(i).unwrap().len(), 4);
                    }
                    assert_eq!(view.helper().function.signature.parameters.len(), 12);
                    assert_eq!(view.helper().function.signature.results.len(), 4);
                    assert_eq!(view.call().operation.results.len(), 4);
                    assert_eq!(view.matrix().operation.results.len(), 4);
                    Ok(view.emission().return_permutation())
                },
            )
            .map_err(query_error)?;
        assert_eq!(observed, source.return_permutation());
        assert_eq!(budget.storage(), inventory_floor);
        inspect_retained_floor_boundaries(
            owner,
            source,
            &inventory,
            storage.retained_storage(),
            budget,
        )?;
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
            let result = owner.with_checked_bf16_nominal_call_v1(
                &inventory,
                root,
                caller,
                block,
                source.source_call(),
                budget,
                |_, _| {
                    entered.set(true);
                    Ok(())
                },
            );
            assert!(matches!(result, Err(QueryError::Unavailable(_))));
            assert!(!entered.get());
            assert_eq!(budget.storage(), inventory_floor);
        }

        // Actual fixture operands/destination are unprojected locals. Check
        // before allocating the small detached negative controls; no arbitrary
        // constant/projection payload is cloned under a guessed envelope.
        budget.charge_work(12)?;
        let actual = source.source_call();
        assert_eq!(actual.arguments().len(), 4);
        assert!(actual.variadic_argument_abis().is_empty());
        for argument in actual.arguments() {
            assert!(
                matches!(argument, SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p)
                if p.projections().is_empty())
            );
        }
        assert!(
            actual
                .destination()
                .unwrap()
                .place()
                .projections()
                .is_empty()
        );
        let clone_storage = 2 * std::mem::size_of::<SemanticDirectCallV1>()
            + 8 * std::mem::size_of::<SemanticOperandV1>();
        budget.reserve_storage(clone_storage)?;
        let clone_outcome = catch_unwind(AssertUnwindSafe(|| {
            let cloned = actual.clone();
            assert_eq!(cloned, *actual);
            let wrong_callee = SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(u32::MAX),
                actual.arguments().to_vec(),
                actual.destination().cloned(),
                actual.unwind(),
            )
            .unwrap();
            // Drop each second object before making another: two headers and
            // eight fixed operand payloads are the complete simultaneous bound.
            let reject = |candidate: &SemanticDirectCallV1, budget: &mut Budget<'_>| {
                let entered = Cell::new(false);
                let result = owner.with_checked_bf16_nominal_call_v1(
                    &inventory,
                    source.root(),
                    source.root(),
                    source.call_block(),
                    candidate,
                    budget,
                    |_, _| {
                        entered.set(true);
                        Ok(())
                    },
                );
                assert!(matches!(result, Err(QueryError::Unavailable(_))));
                assert!(!entered.get());
            };
            reject(&cloned, budget);
            reject(&wrong_callee, budget);
            drop(wrong_callee);
            let destination = actual.destination().unwrap();
            let wrong_continuation = SemanticDirectCallV1::new_callable(
                actual.callee(),
                actual.arguments().to_vec(),
                Some(SemanticCallDestinationV1::new(
                    destination.place().clone(),
                    SemanticControlFlowEdgeV1::new(
                        destination.edge().role(),
                        SemanticBlockIdV1::from_index(u32::MAX),
                    ),
                )),
                actual.unwind(),
            )
            .unwrap();
            reject(&wrong_continuation, budget);
        }));
        let clone_failed = match clone_outcome {
            Ok(()) => false,
            Err(payload) => {
                drop(payload);
                true
            }
        };
        budget.release_storage(clone_storage)?;
        if clone_failed {
            return Err(Error::CallbackPanicked);
        }

        // Equality of canonical bytes is deliberately insufficient: the
        // second executable is solely a detached negative-control owner.
        let (foreign, foreign_storage) =
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                owner.executable().module(), budget,
            ).expect("identical negative-control canonical owner verifies");
        budget.reserve_storage(foreign_storage.retained_storage())?;
        let (foreign_inventory, foreign_inventory_storage) =
            CanonicalKirInventoryV1::derive(&foreign, budget).expect("foreign inventory derives");
        budget.reserve_storage(foreign_inventory_storage.retained_storage())?;
        let foreign_outcome = catch_unwind(AssertUnwindSafe(|| {
            assert_eq!(
                foreign.canonical().identity(),
                owner.executable().canonical().identity()
            );
            let entered = Cell::new(false);
            let result = owner.with_checked_bf16_nominal_call_v1(
                &foreign_inventory,
                source.root(),
                source.root(),
                source.call_block(),
                source.source_call(),
                budget,
                |_, _| {
                    entered.set(true);
                    Ok(())
                },
            );
            assert_eq!(
                result,
                Err(QueryError::Unavailable("foreign canonical inventory"))
            );
            assert!(!entered.get());
        }));
        let foreign_failed = match foreign_outcome {
            Ok(()) => false,
            Err(payload) => {
                drop(payload);
                true
            }
        };
        drop(foreign_inventory);
        budget.release_storage(foreign_inventory_storage.retained_storage())?;
        drop(foreign);
        budget.release_storage(foreign_storage.retained_storage())?;
        if foreign_failed {
            return Err(Error::CallbackPanicked);
        }

        // Error/panic/success postflights preserve callback-owned charges on the
        // same actual owner and cumulative Work ledger.
        for case in 0..3 {
            let before = budget.storage();
            let result = owner.with_checked_bf16_nominal_call_v1(
                &inventory,
                source.root(),
                source.root(),
                source.call_block(),
                source.source_call(),
                budget,
                |_, budget| {
                    budget.reserve_storage(23)?;
                    budget.charge_work(17)?;
                    match case {
                        0 => Ok(()),
                        1 => Err(QueryError::Unavailable("genuine query callback control")),
                        _ => panic!("genuine nominal query callback panic control"),
                    }
                },
            );
            match case {
                0 => assert!(result.is_ok()),
                1 => assert_eq!(
                    result,
                    Err(QueryError::Unavailable("genuine query callback control"))
                ),
                _ => assert_eq!(result, Err(QueryError::CallbackPanicked)),
            }
            assert_eq!(budget.storage(), before + 23);
            budget.release_storage(23)?;
        }
        let entered = Cell::new(false);
        assert!(
            owner
                .with_checked_canonical_calls_v1(&inventory, budget, |_, _| {
                    entered.set(true);
                    Ok(())
                })
                .is_err()
        );
        assert!(!entered.get());
        Ok(())
    }));
    let result = match outcome {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::CallbackPanicked)
        }
    };
    drop(inventory);
    if budget.work_ledger_identity_v1() != ledger || budget.storage() < inventory_floor {
        drop(result);
        return Err(Error::Resource(Resource::Accounting));
    }
    budget.release_storage(storage.retained_storage())?;
    assert_eq!(budget.storage(), entry);
    result
}
