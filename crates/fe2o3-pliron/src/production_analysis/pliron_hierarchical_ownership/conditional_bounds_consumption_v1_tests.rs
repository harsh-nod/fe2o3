use super::*;
use crate::production_analysis::{
    LivePlironStructuralIdentityProviderV1, PlironStructuralIdentityProviderV1,
    ProductionAnalysisResourceLimitsV1 as Limits,
};
use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
use dialect_kernel::{
    BranchOp, IndexLessThanBranchOp, IndexType, InvocationIndexOp, RankedViewType, ReturnOp, TrapOp,
};
use fe2o3_kernel_ir::ConditionalTotalViewAddressDomainV1 as Domain;
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, types::FunctionType},
    dialect::DialectName,
    op::Op,
    operation::verify_operation,
    r#type::TypeHandle,
};

struct Fixture {
    context: Context,
    function: FuncOp,
    selection: (Ptr<Operation>, Value),
    reads: Vec<LiveReadBoundV1>,
    census: Census,
    epoch: u64,
}

// This arena exercises the consumer's descriptive live query. It cannot mint a
// source subject, a bounds dependency, or a validated production report.
fn fixture(reads: usize, guard_argument: Option<usize>) -> Fixture {
    let mut context = Context::new();
    dialect_kernel::register_dialect(
        &mut context,
        &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    let index: TypeHandle = IndexType::get(&context).into();
    let signature = FunctionType::get(&context, vec![index; 3], vec![]);
    let function = FuncOp::new(
        &mut context,
        "bounds_consumer".try_into().unwrap(),
        signature,
    );
    let entry = function.get_entry_block(&context);
    let [guard, body, exit, trap] = ["guard", "body", "exit", "trap"].map(|name| {
        let block = BasicBlock::new(&mut context, Some(name.try_into().unwrap()), vec![]);
        block.insert_at_back(function.get_region(&context), &context);
        block
    });
    ExecutionLayoutOp::new_with_domain(
        &mut context,
        17,
        [0, 1, 1],
        [2, 1, 1],
        2,
        ExecutionDomainAttr::FullPhysicalWorkgroups,
    )
    .get_operation()
    .insert_at_back(entry, &context);
    let invocation = InvocationIndexOp::new(&mut context, 0, 0);
    invocation.get_operation().insert_at_back(entry, &context);
    let point = invocation.result(&context);
    let [output, input] = [0, 1].map(|argument| {
        let extent = entry.deref(&context).get_argument(argument);
        let ty = RankedViewType::new(&context, 32, argument == 0, vec![DYNAMIC_EXTENT]).unwrap();
        let view = RankedViewOp::new_in_space_with_allocation_contract(
            &mut context,
            ty,
            vec![extent],
            MemorySpaceAttr::Global,
            argument as u64 + 1,
            argument as u64 + 1,
        )
        .unwrap();
        view.get_operation().insert_at_back(entry, &context);
        view.result(&context)
    });
    let ownership = OwnershipContractOp::new(
        &mut context,
        output,
        OwnershipCoverageAttr::TotalView,
        OwnershipPartitionAttr::ExactSets,
    )
    .unwrap();
    ownership.get_operation().insert_at_back(entry, &context);
    let output_extent = entry.deref(&context).get_argument(0);
    IndexLessThanBranchOp::new(&mut context, point, output_extent, guard, exit)
        .get_operation()
        .insert_at_back(entry, &context);
    match guard_argument {
        Some(argument) => {
            let extent = entry.deref(&context).get_argument(argument);
            IndexLessThanBranchOp::new(&mut context, point, extent, body, trap)
                .get_operation()
                .insert_at_back(guard, &context);
        }
        None => BranchOp::new(&mut context, body)
            .get_operation()
            .insert_at_back(guard, &context),
    }
    let reads = (0..reads)
        .map(|_| {
            let read = RankedAccessOp::new(&mut context, AccessKindAttr::Read, input, vec![point])
                .unwrap();
            read.get_operation().insert_at_back(body, &context);
            LiveReadBoundV1 {
                operation: read.get_operation(),
                view: input,
                index: point,
                extent: entry.deref(&context).get_argument(1),
                domain: Domain::GuardedOutput,
            }
        })
        .collect();
    RankedAccessOp::new(&mut context, AccessKindAttr::Write, output, vec![point])
        .unwrap()
        .get_operation()
        .insert_at_back(body, &context);
    BranchOp::new(&mut context, exit)
        .get_operation()
        .insert_at_back(body, &context);
    ReturnOp::new(&mut context)
        .get_operation()
        .insert_at_back(exit, &context);
    TrapOp::new(&mut context)
        .get_operation()
        .insert_at_back(trap, &context);
    verify_operation(function.get_operation(), &context).unwrap();
    let census = LivePlironStructuralIdentityProviderV1::new(&context, &function)
        .capture_with_resource_limits_v1(Limits::production_hard_ceiling())
        .ok()
        .unwrap()
        .input_census;
    let epoch = context.ir_mutation_attempt_epoch().unwrap().value();
    Fixture {
        context,
        function,
        selection: (ownership.get_operation(), output),
        reads,
        census,
        epoch,
    }
}

fn query(
    f: &Fixture,
    reads: Option<&[LiveReadBoundV1]>,
    limits: Limits,
) -> (
    Result<Result<RuleFactsV1, RuleRefusalV1>, Limit>,
    Bound,
    Bound,
) {
    let initial = Bound::checked_phase(Phase::StructuralIdentity, 11, 37, 0).unwrap();
    let mut analyses =
        Manager::new_with_resource_contract(&f.function, f.census, initial, 0, limits).unwrap();
    analyses.prepare_function_inventory(&f.context, &f.function);
    let inventory = analyses.function_inventory_handle().unwrap();
    let before = analyses.resource_upper_bound();
    let result = check_selected_live_rule_v1(
        (&f.context, &f.function),
        &inventory,
        (f.census, f.epoch),
        f.selection,
        reads,
        &mut analyses,
        None,
    );
    (result, before, analyses.resource_upper_bound())
}

#[test]
fn consumer_requires_complete_read_roster_including_explicit_empty() {
    let f = fixture(2, None);
    let hard = Limits::production_hard_ceiling();
    assert!(query(&f, None, hard).0.unwrap().is_ok());
    assert!(query(&f, Some(&f.reads), hard).0.unwrap().is_ok());
    for reads in [&f.reads[..0], &f.reads[..1]] {
        assert_eq!(
            query(&f, Some(reads), hard).0,
            Ok(Err(RuleRefusalV1::Coordinate))
        );
    }
    let duplicates = [f.reads[0], f.reads[0]];
    assert_eq!(
        query(&f, Some(&duplicates), hard).0,
        Ok(Err(RuleRefusalV1::Coordinate))
    );
}

#[test]
fn consumer_checks_read_guards_against_the_authenticated_read_domain() {
    let hard = Limits::production_hard_ceiling();
    let f = fixture(1, Some(1));
    assert!(query(&f, None, hard).0.unwrap().is_err());
    assert!(query(&f, Some(&f.reads), hard).0.unwrap().is_ok());
    let unrelated_guard = fixture(1, Some(2));
    assert!(
        query(&unrelated_guard, Some(&unrelated_guard.reads), hard)
            .0
            .unwrap()
            .is_err()
    );
}

#[test]
fn consumer_rejects_live_mutation_after_roster_capture() {
    let f = fixture(1, None);
    drop(f.selection.0.deref_mut(&f.context));
    assert_eq!(
        query(&f, Some(&f.reads), Limits::production_hard_ceiling()).0,
        Ok(Err(RuleRefusalV1::Coordinate))
    );
}

#[test]
fn consumer_charges_read_coverage_for_empty_rosters_before_querying() {
    let f = fixture(0, None);
    let hard = Limits::production_hard_ceiling();
    let (ordinary, _, legacy) = query(&f, None, hard);
    let (source, _, explicit) = query(&f, Some(&[]), hard);
    assert_eq!(source, ordinary);
    assert!(source.unwrap().is_ok());
    assert!(explicit.work_upper_bound() > legacy.work_upper_bound());
    assert!(explicit.peak_storage_upper_bound() > legacy.peak_storage_upper_bound());
    let exact = Limits::new(
        explicit.work_upper_bound(),
        explicit.peak_storage_upper_bound(),
    );
    assert_eq!(query(&f, Some(&[]), exact).2, explicit);
    for (work_short, storage_short) in [(1, 0), (0, 1)] {
        let limits = Limits::new(
            explicit.work_upper_bound() - work_short,
            explicit.peak_storage_upper_bound() - storage_short,
        );
        let (result, before, after) = query(&f, Some(&[]), limits);
        assert!(result.is_err());
        assert_eq!(after, before);
    }
}

#[test]
fn dependency_scan_preflights_refuse_overflow_in_each_consuming_phase() {
    use crate::production_analysis::{
        conditional_semantic_v1 as semantic, pliron_effect_refinement::conditional_v1 as effect,
    };
    for census in [
        Census {
            operands: usize::MAX,
            ..Census::default()
        },
        Census {
            operations: usize::MAX,
            ..Census::default()
        },
        Census {
            ownership_contracts: usize::MAX,
            ..Census::default()
        },
    ] {
        for (phase, result) in [
            (Phase::HierarchicalOwnership, overhead(census, 1)),
            (Phase::EffectRefinement, effect::preflight_v1(census, 1)),
            (Phase::SemanticRefinement, semantic::preflight_v1(census, 1)),
        ] {
            assert_eq!(result.unwrap_err().phase, phase);
        }
    }
}
