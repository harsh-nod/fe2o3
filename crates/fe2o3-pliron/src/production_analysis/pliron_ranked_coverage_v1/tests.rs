use super::*;

#[test]
fn live_query_observer_accumulates_and_preserves_denial_across_projection_panic() {
    use crate::production_analysis::invocation_receipt_v1::{
        InvocationReceiptFailureV1 as ReceiptFailure, InvocationReceiptV1 as Receipt,
    };
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
    };

    let f = fixture(GraphCase::Guarded);
    drop(f.inventory); // Do not retain an unpaid duplicate inventory.
    let hard = WrapperLimits::production_hard_ceiling();
    let capture = LivePlironStructuralIdentityProviderV1::new(&f.context, &f.function)
        .capture_with_resource_limits_v1(hard)
        .ok()
        .unwrap();
    let census = capture.input_census;
    let epoch = f.context.ir_mutation_attempt_epoch().unwrap().value();
    let mut am = Manager::new_with_resource_contract(
        &f.function,
        census,
        capture.resource_upper_bound,
        0,
        hard,
    )
    .unwrap();
    am.prepare_function_inventory(&f.context, &f.function);
    let inventory = am.function_inventory_handle().unwrap();
    let ownership = f.contract.get_operation();
    let view = f.view.result(&f.context);
    let owner = Phase::HierarchicalOwnership;
    let floor = am.resource_upper_bound();
    let q = live_preflight(census).unwrap();
    let twice = q.checked_then_retain(q, owner).unwrap();
    let total = floor.checked_then_retain(twice, owner).unwrap();
    let limits = WrapperLimits::new(total.work_upper_bound(), total.peak_storage_upper_bound());
    let mut receipt = Receipt::new(floor, limits).unwrap();
    let phase = receipt.phase(owner, 0).unwrap();
    let additional = Cell::new(Bound::default());
    let denial = Limit {
        phase: owner,
        resource: "work upper bound",
    };
    {
        let project = |bound| {
            let _actual_operation = ownership.deref(&f.context);
            Ok(bound)
        };
        let observer = phase.observer(&project);
        let mut query = || {
            check_conditional_ownership_live_rule_with_observation_v1(
                (&f.context, &f.function),
                inventory.as_ref(),
                census,
                epoch,
                (ownership, view),
                &mut am,
                Some((&observer, &additional)),
            )
        };
        let first = query().unwrap().unwrap();
        assert_eq!(additional.get(), q);
        assert_eq!(query(), Ok(Ok(first)));
        assert_eq!(additional.get(), twice);
        assert_eq!(query(), Err(denial));
        assert_eq!(additional.get(), twice);
        let held = ownership.deref_mut(&f.context);
        assert!(catch_unwind(AssertUnwindSafe(query)).is_err());
        drop(held);
    }
    assert_eq!(additional.get(), twice);
    assert_eq!(am.resource_upper_bound(), total);
    drop(phase); // No returned owner is being committed by this leaf test.
    let state = receipt.snapshot();
    assert_eq!(state.first_denial, Some(denial));
    assert!(state.caught_panic);
    assert_eq!(state.current, state.committed);
    assert_eq!(
        state.current,
        Bound::checked_phase(
            owner,
            twice.work_upper_bound(),
            twice.peak_storage_upper_bound(),
            0,
        )
        .unwrap()
    );
    assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(denial)));
    drop((receipt, inventory, am, capture));
}
use crate::production_analysis::{
    LivePlironStructuralIdentityProviderV1, PlironStructuralIdentityProviderV1,
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitsV1,
    pliron_function_inventory::BoundedPlironFunctionInventoryV1,
};
use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
use dialect_kernel::{
    BranchOp, DYNAMIC_EXTENT, IndexLessThanBranchOp, IndexType, IndexUnknownOp, InvocationIndexOp,
    MemorySpaceAttr, OwnershipContractOp, OwnershipCoverageAttr, OwnershipPartitionAttr,
    RankedAccessOp, RankedViewOp, RankedViewType, ReturnOp,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp, types::FunctionType},
    context::Context,
    dialect::DialectName,
    op::Op as _,
    operation::verify_operation,
    r#type::TypeHandle,
};

// Raw arena fixtures exercise the descriptive reader, not authenticated pending
// custody, source correspondence or a production theorem receipt.
struct Fixture {
    function: FuncOp,
    contract: OwnershipContractOp,
    view: RankedViewOp,
    inventory: BoundedPlironFunctionInventoryV1,
    census: ProductionAnalysisInputCensusV1,
    epoch: u64,
    context: Context,
}

#[derive(Clone, Copy)]
enum GraphCase {
    Guarded,
    GuardedWithRankEightView,
    Reversed,
    ExtraWrite,
    UnknownGuardWithExtraWrite,
}

fn fixture(case: GraphCase) -> Fixture {
    let mut context = Context::new();
    dialect_kernel::register_dialect(
        &mut context,
        &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    let index: TypeHandle = IndexType::get(&context).into();
    let argument_count = if matches!(case, GraphCase::GuardedWithRankEightView) {
        8
    } else {
        1
    };
    let signature = FunctionType::get(&context, vec![index; argument_count], vec![]);
    let function = FuncOp::new(
        &mut context,
        "shared_live_coverage".try_into().unwrap(),
        signature,
    );
    let entry = function.get_entry_block(&context);
    let extent = entry.deref(&context).get_argument(0);
    let body = BasicBlock::new(&mut context, Some("write".try_into().unwrap()), vec![]);
    body.insert_at_back(function.get_region(&context), &context);
    let exit = BasicBlock::new(&mut context, Some("exit".try_into().unwrap()), vec![]);
    exit.insert_at_back(function.get_region(&context), &context);
    let extra = matches!(
        case,
        GraphCase::ExtraWrite | GraphCase::UnknownGuardWithExtraWrite
    )
    .then(|| {
        let block = BasicBlock::new(&mut context, Some("extra".try_into().unwrap()), vec![]);
        block.insert_at_back(function.get_region(&context), &context);
        block
    });
    let layout = ExecutionLayoutOp::new_with_domain(
        &mut context,
        41,
        [0, 1, 1],
        [2, 1, 1],
        2,
        ExecutionDomainAttr::FullPhysicalWorkgroups,
    );
    let invocation = InvocationIndexOp::new(&mut context, 0, 0);
    let ty = RankedViewType::new(&context, 32, true, vec![DYNAMIC_EXTENT]).unwrap();
    let view = RankedViewOp::new_in_space_with_allocation_contract(
        &mut context,
        ty,
        vec![extent],
        MemorySpaceAttr::Global,
        17,
        17,
    )
    .unwrap();
    let view_value = view.result(&context);
    let contract = OwnershipContractOp::new(
        &mut context,
        view_value,
        OwnershipCoverageAttr::TotalView,
        OwnershipPartitionAttr::ExactSets,
    )
    .unwrap();
    for operation in [
        layout.get_operation(),
        invocation.get_operation(),
        view.get_operation(),
        contract.get_operation(),
    ] {
        operation.insert_at_back(entry, &context);
    }
    if matches!(case, GraphCase::GuardedWithRankEightView) {
        let ty = RankedViewType::new(&context, 32, false, vec![DYNAMIC_EXTENT; 8]).unwrap();
        let dimensions = (0..8)
            .map(|index| entry.deref(&context).get_argument(index))
            .collect();
        RankedViewOp::new_in_space_with_allocation_contract(
            &mut context,
            ty,
            dimensions,
            MemorySpaceAttr::Global,
            31,
            31,
        )
        .unwrap()
        .get_operation()
        .insert_at_back(entry, &context);
    }
    let invocation_value = invocation.result(&context);
    let guard_extent = if matches!(case, GraphCase::UnknownGuardWithExtraWrite) {
        let unknown = IndexUnknownOp::new(&mut context);
        unknown.get_operation().insert_at_back(entry, &context);
        unknown.result(&context)
    } else {
        extent
    };
    let reversed = matches!(case, GraphCase::Reversed);
    IndexLessThanBranchOp::new(
        &mut context,
        invocation_value,
        guard_extent,
        if reversed { exit } else { body },
        if reversed { body } else { exit },
    )
    .get_operation()
    .insert_at_back(entry, &context);
    let view_value = view.result(&context);
    RankedAccessOp::new(
        &mut context,
        AccessKindAttr::Write,
        view_value,
        vec![invocation_value],
    )
    .unwrap()
    .get_operation()
    .insert_at_back(body, &context);
    BranchOp::new(&mut context, extra.unwrap_or(exit))
        .get_operation()
        .insert_at_back(body, &context);
    ReturnOp::new(&mut context)
        .get_operation()
        .insert_at_back(exit, &context);
    if let Some(extra) = extra {
        RankedAccessOp::new(
            &mut context,
            AccessKindAttr::Write,
            view_value,
            vec![invocation_value],
        )
        .unwrap()
        .get_operation()
        .insert_at_back(extra, &context);
        BranchOp::new(&mut context, exit)
            .get_operation()
            .insert_at_back(extra, &context);
    }
    verify_operation(function.get_operation(), &context).unwrap();
    let census = {
        let mut provider = LivePlironStructuralIdentityProviderV1::new(&context, &function);
        provider
            .capture_with_resource_limits_v1(ProductionAnalysisResourceLimitsV1::new(
                usize::MAX,
                usize::MAX,
            ))
            .ok()
            .unwrap()
            .input_census
    };
    let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
    let epoch = context.ir_mutation_attempt_epoch().unwrap().value();
    Fixture {
        function,
        contract,
        view,
        inventory,
        census,
        epoch,
        context,
    }
}

fn query(
    fixture: &Fixture,
    work_limit: usize,
) -> (
    Result<[u32; 2], ProductionRankedRecipeCoverageErrorV1>,
    usize,
) {
    let temporary = 2 * fixture.census.type_nodes * std::mem::size_of::<TypeHandle>()
        + 2 * std::mem::size_of::<Vec<TypeHandle>>();
    let storage = 37 + temporary;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut work, storage);
    budget.reserve_storage(storage).unwrap();
    let result = (|| {
        let prepared = live_v1::prepare(
            &fixture.context,
            &fixture.function,
            &fixture.inventory,
            fixture.census,
            fixture.epoch,
            fixture.contract.get_operation(),
            fixture.view.result(&fixture.context),
            &mut budget,
        )?;
        assert_eq!(
            prepared.ownership,
            Site {
                block: 0,
                operation: 3
            }
        );
        assert_eq!(prepared.view, fixture.view.result(&fixture.context));
        let result = check_paths(&prepared.reader, prepared.selected, &mut budget)?;
        prepared.reader.check_epoch(&mut budget)?;
        Ok(result)
    })()
    .map_err(recipe_error);
    assert_eq!(
        (
            budget.storage(),
            budget.peak_storage(),
            budget.failed_storage()
        ),
        (storage, storage, None)
    );
    (result, budget.work())
}

#[test]
fn live_dynamic_guard_has_both_normal_exits_and_exact_work_boundary() {
    let fixture = fixture(GraphCase::Guarded);
    let (result, exact) = query(&fixture, usize::MAX);
    assert_eq!(result, Ok([2, 2]));
    assert_eq!(query(&fixture, exact), (result, exact));
    let (result, prefix) = query(&fixture, exact - 1);
    let Err(ProductionRankedRecipeCoverageErrorV1::Resource(ResourceError::Work(error))) = result
    else {
        panic!("expected charged work refusal, got {result:?}");
    };
    assert_eq!((error.limit(), error.actual()), (exact - 1, exact));
    assert!(prefix < exact);
}

#[test]
fn original_subject_borrow_is_unavailable_after_the_first_pass_or_unwind() {
    use crate::KernelCheckPassKindV1;
    use crate::production_analysis::pliron_pass_contract::{
        PlironPassPreservationErrorV1,
        begin_production_pliron_pass_contract_session_with_resource_limits_v1,
    };

    let fixture = fixture(GraphCase::Guarded);
    for panic_in_pass in [false, true] {
        let provider =
            LivePlironStructuralIdentityProviderV1::new(&fixture.context, &fixture.function);
        let mut session = begin_production_pliron_pass_contract_session_with_resource_limits_v1(
            provider,
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .unwrap();
        let (snapshot, epoch) = session.initial_subject_v1().unwrap();
        let same_borrow = session.initial_subject_v1().unwrap().0;
        assert!(std::ptr::eq(snapshot, same_borrow));
        assert_eq!(epoch, fixture.epoch);
        let mut provider =
            LivePlironStructuralIdentityProviderV1::new(&fixture.context, &fixture.function);
        let fresh = provider
            .capture_with_resource_limits_v1(
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            )
            .ok()
            .unwrap();
        assert!(
            provider
                .require_exact_identity(snapshot, &fresh.snapshot)
                .is_ok()
        );
        let result = session.run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
            assert!(!panic_in_pass, "injected initial-subject test panic");
            Ok::<_, ()>(())
        });
        if panic_in_pass {
            assert!(matches!(
                result,
                Err(PlironPassPreservationErrorV1::AnalysisPanicked { .. })
            ));
        } else {
            assert_eq!(result, Ok(Ok(())));
        }
        assert!(matches!(
            session.initial_subject_v1(),
            Err(PlironPassPreservationErrorV1::InvalidSessionState { .. })
        ));
    }
}

#[test]
fn live_reversed_guard_is_not_conditional_coverage() {
    assert_eq!(
        query(&fixture(GraphCase::Reversed), usize::MAX).0,
        Err(ProductionRankedRecipeCoverageErrorV1::WriteCount {
            block: 1,
            predicate: false
        })
    );
}

#[test]
fn live_reader_rejects_same_text_after_a_mutation_attempt() {
    let fixture = fixture(GraphCase::Guarded);
    assert_eq!(query(&fixture, usize::MAX).0, Ok([2, 2]));
    let operation = fixture.contract.get_operation();
    let attributes = operation.deref(&fixture.context).attributes.clone();
    operation.deref_mut(&fixture.context).attributes = attributes;
    assert_eq!(
        query(&fixture, usize::MAX),
        (Err(ProductionRankedRecipeCoverageErrorV1::Coordinate), 1)
    );
}

#[test]
fn live_preparation_leaves_second_write_refusal_to_the_shared_census() {
    assert_eq!(
        query(&fixture(GraphCase::ExtraWrite), usize::MAX).0,
        Err(
            ProductionRankedRecipeCoverageErrorV1::UnsupportedOperation {
                block: 3,
                operation: 0
            }
        )
    );
    assert_eq!(
        query(&fixture(GraphCase::UnknownGuardWithExtraWrite), usize::MAX).0,
        Err(ProductionRankedRecipeCoverageErrorV1::UnresolvedCondition { block: 0 })
    );
}

type WrapperLimits = ProductionAnalysisResourceLimitsV1;

fn wrapper_manager(f: &Fixture, limits: WrapperLimits) -> Manager {
    let mut am = Manager::new_with_resource_contract(
        &f.function,
        f.census,
        Bound::checked_phase(Phase::StructuralIdentity, 11, 37, 0).unwrap(),
        0,
        limits,
    )
    .unwrap();
    am.prepare_function_inventory(&f.context, &f.function);
    am
}
fn wrapper_query(f: &Fixture, census: Census, am: &mut Manager) -> LiveResultV1 {
    let inventory = am.function_inventory_handle().unwrap();
    assert!(std::ptr::eq(
        inventory.as_ref(),
        am.function_inventory().unwrap()
    ));
    check_conditional_ownership_live_rule_v1(
        &f.context,
        &f.function,
        inventory.as_ref(),
        census,
        f.epoch,
        f.contract.get_operation(),
        f.view.result(&f.context),
        am,
    )
}
fn wrapper_total(f: &Fixture) -> Bound {
    wrapper_manager(f, WrapperLimits::new(usize::MAX, usize::MAX))
        .resource_upper_bound()
        .checked_then_retain(
            live_preflight(f.census).unwrap(),
            Phase::HierarchicalOwnership,
        )
        .unwrap()
}

#[test]
fn live_wrapper_exact_work_and_storage() {
    let f = fixture(GraphCase::Guarded);
    let total = wrapper_total(&f);
    let mut am = wrapper_manager(
        &f,
        WrapperLimits::new(total.work_upper_bound(), total.peak_storage_upper_bound()),
    );
    let result = |operation| ValueKeyV1::Result {
        site: RuleSiteV1 {
            block: 0,
            operation,
        },
        index: 0,
    };
    assert_eq!(
        wrapper_query(&f, f.census, &mut am),
        Ok(Ok(RuleFactsV1 {
            view: result(2),
            index: result(1),
            extent: ValueKeyV1::Argument { block: 0, index: 0 },
            write: RuleSiteV1 {
                block: 1,
                operation: 0
            },
            normal_exits: [2, 2],
        }))
    );
    assert_eq!(am.resource_upper_bound(), total);
}

#[test]
fn live_wrapper_admits_full_rank_dynamic_metadata_with_the_original_census() {
    let f = fixture(GraphCase::GuardedWithRankEightView);
    let total = wrapper_total(&f);
    let mut am = wrapper_manager(
        &f,
        WrapperLimits::new(total.work_upper_bound(), total.peak_storage_upper_bound()),
    );
    let facts = wrapper_query(&f, f.census, &mut am).unwrap().unwrap();
    assert_eq!(
        facts.write,
        RuleSiteV1 {
            block: 1,
            operation: 0
        }
    );
    assert_eq!(facts.normal_exits, [2, 2]);
    assert_eq!(f.census.block_arguments, 8);
    assert_eq!(am.resource_upper_bound(), total);
}
#[test]
fn live_wrapper_one_short_admission_preserves_prefix() {
    let f = fixture(GraphCase::Guarded);
    let total = wrapper_total(&f);
    for (dw, ds, resource) in [
        (1, 0, "work upper bound"),
        (0, 1, "peak storage upper bound"),
    ] {
        let mut am = wrapper_manager(
            &f,
            WrapperLimits::new(
                total.work_upper_bound() - dw,
                total.peak_storage_upper_bound() - ds,
            ),
        );
        let prefix = am.resource_upper_bound();
        assert_eq!(
            wrapper_query(&f, f.census, &mut am),
            Err(Limit {
                phase: Phase::HierarchicalOwnership,
                resource
            })
        );
        assert_eq!(am.resource_upper_bound(), prefix);
    }
}
#[test]
fn live_wrapper_retained_epoch_refusal_keeps_reservation() {
    let f = fixture(GraphCase::Guarded);
    let total = wrapper_total(&f);
    let mut am = wrapper_manager(
        &f,
        WrapperLimits::new(total.work_upper_bound(), total.peak_storage_upper_bound()),
    );
    drop(f.contract.get_operation().deref_mut(&f.context));
    assert_ne!(
        f.context.ir_mutation_attempt_epoch().unwrap().value(),
        f.epoch
    );
    assert_eq!(
        wrapper_query(&f, f.census, &mut am),
        Ok(Err(RuleRefusalV1::Coordinate))
    );
    assert_eq!(am.resource_upper_bound(), total);
    assert_eq!(
        wrapper_query(&f, f.census, &mut am),
        Err(live_limit("work upper bound"))
    );
    assert_eq!(am.resource_upper_bound(), total);
}
#[test]
fn live_wrapper_requires_whole_census_equality() {
    let f = fixture(GraphCase::Guarded);
    let total = wrapper_total(&f);
    let mut changed = f.census;
    changed.native_switch_verification_scratch += 1;
    assert_eq!(live_preflight(changed), live_preflight(f.census));
    let mut am = wrapper_manager(
        &f,
        WrapperLimits::new(total.work_upper_bound(), total.peak_storage_upper_bound()),
    );
    assert_eq!(
        wrapper_query(&f, changed, &mut am),
        Ok(Err(RuleRefusalV1::Coordinate))
    );
    assert_eq!(am.input_census(), Some(f.census));
    assert_eq!(am.resource_upper_bound(), total);
}

#[test]
fn live_wrapper_requires_the_managers_actual_inventory_not_an_equal_roster() {
    let f = fixture(GraphCase::Guarded);
    let total = wrapper_total(&f);
    let mut am = wrapper_manager(
        &f,
        WrapperLimits::new(total.work_upper_bound(), total.peak_storage_upper_bound()),
    );
    assert!(!std::ptr::eq(
        &f.inventory,
        am.function_inventory().unwrap()
    ));
    let result = check_conditional_ownership_live_rule_v1(
        &f.context,
        &f.function,
        &f.inventory,
        f.census,
        f.epoch,
        f.contract.get_operation(),
        f.view.result(&f.context),
        &mut am,
    );
    assert_eq!(result, Ok(Err(RuleRefusalV1::Coordinate)));
    assert_eq!(am.resource_upper_bound(), total);
}

#[test]
fn live_preflight_rejects_counter_overflow_without_admission() {
    let f = fixture(GraphCase::Guarded);
    let mut am = wrapper_manager(&f, WrapperLimits::new(usize::MAX, usize::MAX));
    let prefix = am.resource_upper_bound();
    for field in 0..7 {
        let mut census = f.census;
        let target = match field {
            0 => &mut census.blocks,
            1 => &mut census.operations,
            2 => &mut census.results,
            3 => &mut census.block_arguments,
            4 => &mut census.attributes,
            5 => &mut census.identifier_bytes,
            _ => &mut census.type_nodes,
        };
        *target = usize::MAX;
        assert_eq!(
            wrapper_query(&f, census, &mut am),
            Err(live_limit("conditional coverage arithmetic"))
        );
        assert_eq!(am.resource_upper_bound(), prefix);
    }
}

#[test]
fn reserved_meter_exhaustion_is_sticky_even_for_zero() {
    let mut meter = ReservedMeter {
        remaining: Some(3),
        phase: Phase::HierarchicalOwnership,
    };
    assert!(meter.charge(3).is_ok() && meter.charge(0).is_ok());
    assert_eq!(meter.remaining, Some(0));
    for charge in [1, 0, 1, usize::MAX] {
        assert!(
            matches!(meter.charge(charge), Err(Failure::Resource(e)) if e == live_limit("conditional coverage admitted work"))
        );
        assert_eq!(meter.remaining, None);
    }
}
