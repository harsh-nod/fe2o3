use super::*;
use crate::{ProductionFormalMemoryErrorV1, analyze_checked_output_formal_memory_v1};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, AmdGpuDiagnosticOperation, BasicBlock, ExplicitLaunchExtent,
    FormalIndexWidth, FormalMemoryIncompleteReason, FormalMemoryObligationAnalysis, Function,
    FunctionId, Kernel, LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind,
    Signature, Terminator, Type, ValueDef, ValueId, derive_kernel_memory_obligations_for_launch,
};
use fe2o3_pliron::{CheckedNeutralKernelIrOwnerV1, KirPlironGraphV12};

fn optimize(
    input: &VerifiedCanonicalKernelIrModuleV12,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> CheckedNeutralKernelIrOwnerV1 {
    let (mut graph, storage) = KirPlironGraphV12::import(input, budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let observed = graph
        .execute_production_neutral_optimization_v1(budget)
        .unwrap()
        .extract()
        .unwrap();
    assert_eq!(observed.report().passes().len(), 7);
    budget
        .reserve_storage(observed.storage().retained_storage())
        .unwrap();
    let graph_storage = graph.retained_storage();
    drop(graph);
    budget.release_storage(graph_storage).unwrap();
    let checked = observed.try_check_and_finish_v1(budget).unwrap();
    budget
        .reserve_storage(checked.storage().retained_storage())
        .unwrap();
    checked
}

fn block_count(module: &Module) -> usize {
    module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .map(|body| body.blocks.len())
        .sum()
}

fn traps(module: &Module) -> usize {
    module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(&operation.kind, OperationKind::Call { callee, arguments }
                if AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments)
                    == Some(AmdGpuDiagnosticOperation::Trap))
        })
        .count()
}

#[test]
fn admitted_literal_source_is_changed_before_actual_output_memory_analysis() {
    for expected in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let source = materialize(Fixture::Literal(expected), false, &mut budget);
        let source_storage = retained(&source);
        budget.reserve_storage(source_storage).unwrap();
        let checked = optimize(source.executable(), &mut budget);
        assert_ne!(
            source.executable().canonical().identity(),
            checked.owner().canonical().identity()
        );
        assert!(block_count(checked.owner().module()) < block_count(source.executable().module()));
        assert_eq!(traps(checked.owner().module()), 0);
        let report = analyze_checked_output_formal_memory_v1(&checked).unwrap();
        assert!(std::ptr::eq(report.output(), checked.owner()));
        let [kernel] = report.output().module().kernels.as_slice() else {
            panic!("one actual source kernel expected")
        };
        let [obligations] = report.kernels() else {
            panic!("one actual output analysis expected")
        };
        assert_eq!(obligations.kernel(), &kernel.id);
        assert_eq!(obligations.entry(), &kernel.entry);
        assert_eq!(obligations.index_width(), FormalIndexWidth::Bits64);
        let invocations = obligations.invocations().unwrap();
        assert_eq!(invocations.end_exclusive() - invocations.start(), 64);
        assert!(obligations.accesses().is_empty());
        let fresh = derive_kernel_memory_obligations_for_launch(
            report.output().module(),
            &kernel.id,
            ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [64, 1, 1],
            },
            FormalIndexWidth::Bits64,
        )
        .unwrap();
        assert!(matches!(fresh, FormalMemoryObligationAnalysis::Complete(_)));
        assert_eq!(obligations, fresh.obligations());
        drop(report);
        let checked_storage = checked.storage().retained_storage();
        drop(checked);
        budget.release_storage(checked_storage).unwrap();
        drop(source);
        budget.release_storage(source_storage).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn admitted_selected_failure_still_traps_despite_complete_memory_analysis() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (ssa, launch) = fixture_with_blocks(Fixture::Literal(true), false, |_, _| {
        let mut assertion = literal_terminator(true, 1);
        let SemanticTerminatorKindV1::Assert { condition, .. } = &mut assertion else {
            unreachable!()
        };
        *condition = constant(BOOL, 0, 1);
        vec![
            block(31, vec![], assertion),
            block(32, vec![], SemanticTerminatorKindV1::Return),
        ]
    });
    let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    let source_storage = retained(&source);
    budget.reserve_storage(source_storage).unwrap();
    let checked = optimize(source.executable(), &mut budget);
    assert_ne!(
        source.executable().canonical().identity(),
        checked.owner().canonical().identity()
    );
    let report = analyze_checked_output_formal_memory_v1(&checked).unwrap();
    assert_eq!(traps(report.output().module()), 1);
    assert!(report.output().module().functions.iter().any(|function| {
        function.body.as_ref().is_some_and(|body| {
            body.blocks
                .iter()
                .any(|block| matches!(block.terminator, Some(Terminator::Unreachable)))
        })
    }));
    assert!(report.kernels()[0].accesses().is_empty());
    drop(report);
    let checked_storage = checked.storage().retained_storage();
    drop(checked);
    budget.release_storage(checked_storage).unwrap();
    drop(source);
    budget.release_storage(source_storage).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

// The remaining cases admit and optimize actual KIR modules, not semantic MIR
// source owners. They isolate the complete-only analysis policy.
fn component_module(parameters: Vec<Type>, operations: Vec<Operation>) -> Module {
    let values = (0..parameters.len())
        .map(|index| ValueId(u32::try_from(index).unwrap()))
        .collect();
    let mut block = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("checked-output-formal-component");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(parameters, vec![]),
        values,
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn with_component(module: Module, check: impl FnOnce(&CheckedNeutralKernelIrOwnerV1)) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (input, input_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &module,
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(input_storage.retained_storage())
        .unwrap();
    drop(module);
    let checked = optimize(&input, &mut budget);
    check(&checked);
    let checked_storage = checked.storage().retained_storage();
    drop(checked);
    budget.release_storage(checked_storage).unwrap();
    drop(input);
    budget
        .release_storage(input_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn actual_checked_module_without_kernels_is_not_an_analysis_roster() {
    with_component(Module::new("empty"), |checked| {
        assert!(matches!(
            analyze_checked_output_formal_memory_v1(checked),
            Err(ProductionFormalMemoryErrorV1::KernelCount { actual: 0 })
        ));
    });
}

#[test]
fn actual_checked_global_read_has_fresh_nonempty_obligations() {
    let module = component_module(
        vec![Type::pointer(
            Type::F32,
            AddressSpace::Global,
            AccessMode::ReadOnly,
        )],
        vec![Operation::effect_free(
            ValueDef::new(ValueId(1), Type::F32),
            OperationKind::Load {
                pointer: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        )],
    );
    with_component(module, |checked| {
        let report = analyze_checked_output_formal_memory_v1(checked).unwrap();
        let [obligations] = report.kernels() else {
            panic!("one kernel expected")
        };
        assert_eq!(obligations.accesses().len(), 1);
        assert!(obligations.inter_invocation_conflicts().is_empty());
        let invocations = obligations.invocations().unwrap();
        assert_eq!(invocations.end_exclusive() - invocations.start(), 2);
    });
}

#[test]
fn actual_checked_external_call_cannot_import_an_old_discharge() {
    let mut module = component_module(
        vec![],
        vec![Operation::new(
            vec![],
            OperationKind::Call {
                callee: FunctionId::new("external"),
                arguments: vec![],
            },
        )],
    );
    module.functions.push(Function::declaration(
        "external",
        Signature::new(vec![], vec![]),
    ));
    with_component(module, |checked| {
        let Err(ProductionFormalMemoryErrorV1::Incomplete { reasons }) =
            analyze_checked_output_formal_memory_v1(checked)
        else {
            panic!("incomplete call must reject without a discharge path")
        };
        assert!(matches!(reasons.as_ref(),
            [FormalMemoryIncompleteReason::CallEffectsUnavailable { location, callee }]
            if location.block == fe2o3_kernel_ir::BlockId(0)
                && location.operation_index == 0 && callee.as_str() == "external"
        ));
    });
}

#[test]
fn actual_checked_constant_address_write_retains_the_witness_race() {
    let module = component_module(
        vec![
            Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite),
            Type::F32,
        ],
        vec![Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        )],
    );
    with_component(module, |checked| {
        let Err(ProductionFormalMemoryErrorV1::InterInvocationConflicts { conflicts }) =
            analyze_checked_output_formal_memory_v1(checked)
        else {
            panic!("two witness invocations write the same location")
        };
        assert_eq!(conflicts.len(), 1);
    });
}

#[test]
fn fresh_obligation_rows_release_the_borrow_before_same_checked_owner_moves() {
    fn move_owner_and_rows(
        checked: CheckedNeutralKernelIrOwnerV1,
        rows: Box<[fe2o3_kernel_ir::FormalMemoryObligations]>,
    ) -> (
        CheckedNeutralKernelIrOwnerV1,
        Box<[fe2o3_kernel_ir::FormalMemoryObligations]>,
    ) {
        (checked, rows)
    }
    for expected in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let source = materialize(Fixture::Literal(expected), false, &mut budget);
        let source_storage = retained(&source);
        budget.reserve_storage(source_storage).unwrap();
        let checked = optimize(source.executable(), &mut budget);
        assert_ne!(
            source.executable().canonical().identity(),
            checked.owner().canonical().identity()
        );
        let executable_rows = checked.owner().module().functions.as_ptr();
        let report = analyze_checked_output_formal_memory_v1(&checked).unwrap();
        let obligation_rows = report.kernels().as_ptr();
        let rows = report.into_kernel_obligations();
        assert_eq!(rows.as_ptr(), obligation_rows);
        let (checked, rows) = move_owner_and_rows(checked, rows);
        assert_eq!(checked.owner().module().functions.as_ptr(), executable_rows);
        assert_eq!(rows.as_ptr(), obligation_rows);
        assert_eq!(rows.len(), checked.owner().module().kernels.len());
        assert_eq!(rows[0].kernel(), &checked.owner().module().kernels[0].id);
        assert_eq!(rows[0].entry(), &checked.owner().module().kernels[0].entry);
        assert!(rows[0].inter_invocation_conflicts().is_empty());
        assert!(!checked.grants_authority());
        drop(rows);
        let checked_storage = checked.storage().retained_storage();
        drop(checked);
        budget.release_storage(checked_storage).unwrap();
        drop(source);
        budget.release_storage(source_storage).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn detached_nonempty_obligations_are_owned_data_not_an_executable_owner() {
    let module = component_module(
        vec![Type::pointer(
            Type::F32,
            AddressSpace::Global,
            AccessMode::ReadOnly,
        )],
        vec![Operation::effect_free(
            ValueDef::new(ValueId(1), Type::F32),
            OperationKind::Load {
                pointer: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        )],
    );
    let mut detached = None;
    with_component(module, |checked| {
        let report = analyze_checked_output_formal_memory_v1(checked).unwrap();
        let rows = report.kernels().as_ptr();
        let owned = report.into_kernel_obligations();
        assert_eq!(owned.as_ptr(), rows);
        detached = Some(owned);
    });
    let detached = detached.unwrap();
    assert_eq!(detached.len(), 1);
    assert_eq!(detached[0].kernel().as_str(), "kernel");
    assert_eq!(detached[0].entry().as_str(), "entry");
    assert_eq!(detached[0].accesses().len(), 1);
    assert!(detached[0].inter_invocation_conflicts().is_empty());
}

#[test]
fn actual_checked_selected_zero_store_has_guarded_complete_rows_not_wire_authority() {
    use fe2o3_kernel_ir::{
        ComparePredicate, Constant, FormalAccessDomainV1, FormalBoundsKindV1,
        FormalMemoryReceiptErrorV1, InertCanonicalFormalMemoryObligationReceiptV1,
        IntrinsicOperation, ScalarType,
    };
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let scalar = |id, ty, kind| Operation::effect_free(ValueDef::new(ValueId(id), ty), kind);
    let module = component_module(
        vec![
            Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            ),
            Type::Scalar(ScalarType::U32),
        ],
        vec![
            scalar(
                2,
                Type::INDEX,
                OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
            ),
            scalar(
                3,
                Type::INDEX,
                OperationKind::SliceLength { slice: ValueId(0) },
            ),
            scalar(
                4,
                Type::BOOL,
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(2),
                    rhs: ValueId(3),
                },
            ),
            scalar(5, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
            scalar(
                6,
                Type::INDEX,
                OperationKind::Select {
                    condition: ValueId(4),
                    true_value: ValueId(2),
                    false_value: ValueId(5),
                },
            ),
            scalar(
                7,
                pointer.clone(),
                OperationKind::SliceData { slice: ValueId(0) },
            ),
            scalar(
                8,
                pointer,
                OperationKind::GetElementPointer {
                    base: ValueId(7),
                    offset: ValueId(6),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::GuardedStore {
                    pointer: ValueId(8),
                    value: ValueId(1),
                    predicate: ValueId(4),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ],
    );
    with_component(module, |checked| {
        let report = analyze_checked_output_formal_memory_v1(checked).unwrap();
        assert!(std::ptr::eq(report.output(), checked.owner()));
        let [obligations] = report.kernels() else {
            panic!("one checked kernel")
        };
        let [access] = obligations.accesses() else {
            panic!("one retained actual write")
        };
        let FormalAccessDomainV1::SliceBounded(domain) = access.domain() else {
            panic!("guarded domain")
        };
        assert_eq!(
            obligations.bounds_requirements()[0].minimum_byte_len(),
            None
        );
        assert_eq!(
            obligations.bounds_requirements()[0].kind(),
            FormalBoundsKindV1::SliceElementAtGuardedIndex(domain)
        );
        assert!(!domain.may_access_untrusted_index(0, 0, 2));
        assert!(domain.may_access_untrusted_index(0, 1, 2));
        assert!(!domain.may_access_untrusted_index(1, 1, 2));
        assert!(obligations.inter_invocation_conflicts().is_empty());
        assert_eq!(
            InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(obligations),
            Err(FormalMemoryReceiptErrorV1::UnsupportedGuardedRepresentation)
        );
        assert!(!checked.grants_authority());
    });
}
