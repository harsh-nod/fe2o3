use super::*;
use fe2o3_kernel_ir::{AddressSpace, Constant, ValueId};
use fe2o3_lower_mir_kernel::{
    InertCanonicalMirToKirCorrespondenceEvidenceV6,
    ProductionRankedSemanticProjectionModuleReceiptV1, ProductionRankedSemanticProjectionRootV1,
};

pub(super) fn observed_return_tail(
    local: u32,
    ty: SemanticTypeIdV1,
    value: u128,
    first: u32,
    tag: u8,
) -> Vec<SemanticBasicBlockV1> {
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    vec![
        block(
            tag,
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(local_place(local, ty)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        value,
                        edge(SemanticEdgeRoleV1::SwitchValue, first + 1),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, first + 2),
                )
                .unwrap(),
            },
        ),
        block(tag + 1, vec![], SemanticTerminatorKindV1::Abort),
        block(tag + 2, vec![], SemanticTerminatorKindV1::Return),
    ]
}

pub(super) fn observed_helper(mode: DefinedHelperFixtureV1) -> ProductionSemanticKirOwnerV1 {
    let source = defined_helper_request_with_observer_v1(mode, true)
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    ProductionSemanticKirOwnerV1::try_lower(
        ProductionSemanticMirOwnerV1::try_new(source, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap()
}

pub(super) fn execution_block(
    owner: &ProductionSemanticKirOwnerV1,
    function: u32,
    source_block: u32,
) -> BlockId {
    let execution_block = execution_semantic_block(owner, function, source_block);
    let root = SemanticFunctionIdV1::from_index(0);
    let matching = owner
        .correspondence()
        .blocks()
        .iter()
        .filter(|mapping| {
            mapping.correspondence_owner() == root
                && mapping.semantic_function() == root
                && mapping.semantic_block() == execution_block
        })
        .collect::<Vec<_>>();
    let [mapping] = matching.as_slice() else {
        panic!("one exact execution block mapping: {matching:?}");
    };
    mapping.kernel_ir_block()
}

pub(super) fn execution_semantic_block(
    owner: &ProductionSemanticKirOwnerV1,
    function: u32,
    source_block: u32,
) -> SemanticBlockIdV1 {
    let view = owner
        .semantic_ssa()
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let matching = view
        .block_origins()
        .iter()
        .enumerate()
        .filter(|(_, origin)| {
            origin.function().index() == function && origin.block().index() == source_block
        })
        .map(|(block, _)| SemanticBlockIdV1::from_index(block as u32))
        .collect::<Vec<_>>();
    let [block] = matching.as_slice() else {
        panic!("one exact source block: {matching:?}");
    };
    *block
}

pub(super) fn assert_expansion(owner: &ProductionSemanticKirOwnerV1) {
    owner.verify_equivalence().unwrap();
    owner.semantic_ssa().verify_replay().unwrap();
    assert!(owner.has_expanded_calls());
    let defined = owner
        .module()
        .functions
        .iter()
        .filter(|function| function.body.is_some())
        .collect::<Vec<_>>();
    let [entry] = defined.as_slice() else {
        panic!("one physical root");
    };
    assert_eq!(entry.role, FunctionRole::KernelEntry);
    assert!(
        entry
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .all(|operation| match &operation.kind {
                OperationKind::Call { callee, arguments } => matches!(
                    AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments),
                    Some(AmdGpuDiagnosticOperation::Trap)
                ),
                _ => true,
            })
    );
    let view = owner
        .semantic_ssa()
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    assert_eq!(
        view.instances()
            .iter()
            .map(|instance| instance.function().index())
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(view.instances()[1].parent().unwrap().index(), 0);
    assert_eq!(owner.correspondence().function_count(), 2);
}

pub(super) fn observed_selector(
    owner: &ProductionSemanticKirOwnerV1,
    source_block: u32,
) -> ValueId {
    let body = owner.module().functions[0].body.as_ref().unwrap();
    let at = execution_block(owner, 0, source_block);
    let aborted = execution_block(owner, 0, source_block + 1);
    let returned = execution_block(owner, 0, source_block + 2);
    let block = body.blocks.iter().find(|block| block.id == at).unwrap();
    let view = owner
        .semantic_ssa()
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let source = execution_semantic_block(owner, 0, source_block);
    let SemanticTerminatorKindV1::SwitchInt { targets, .. } = view.body().blocks()
        [source.index() as usize]
        .terminator()
        .kind()
    else {
        panic!("source observer switch");
    };
    assert_eq!(targets.values().len(), 1);
    let expected_literal = targets.values()[0].value();
    let selector = match block.terminator.as_ref().unwrap() {
        Terminator::Switch {
            selector,
            cases,
            default_target,
            default_arguments,
        } => {
            assert_eq!(cases.len(), 1);
            assert_eq!(u128::from(cases[0].value), expected_literal);
            assert_eq!(cases[0].target, aborted);
            assert!(cases[0].arguments.is_empty());
            assert_eq!(*default_target, returned);
            assert!(default_arguments.is_empty());
            *selector
        }
        Terminator::IntegerSwitch {
            selector,
            cases,
            default_target,
            default_arguments,
        } => {
            assert_eq!(cases.len(), 1);
            let value = match cases[0].value {
                Constant::U32(value) => u128::from(value),
                Constant::U64(value) => u128::from(value),
                ref other => panic!("exact unsigned observer literal: {other:?}"),
            };
            assert_eq!(value, expected_literal);
            assert_eq!(cases[0].target, aborted);
            assert!(cases[0].arguments.is_empty());
            assert_eq!(*default_target, returned);
            assert!(default_arguments.is_empty());
            *selector
        }
        Terminator::ConditionalBranch {
            condition,
            then_target,
            then_arguments,
            else_target,
            else_arguments,
        } => {
            assert_eq!(expected_literal, 1);
            assert_eq!((*then_target, *else_target), (aborted, returned));
            assert!(then_arguments.is_empty() && else_arguments.is_empty());
            *condition
        }
        other => panic!("observable helper result: {other:?}"),
    };
    let abort_block = body
        .blocks
        .iter()
        .find(|block| block.id == aborted)
        .unwrap();
    assert!(abort_block.operations.is_empty());
    let spans = owner
        .correspondence()
        .synthetic_operation_spans()
        .iter()
        .filter(|span| {
            span.correspondence_owner().index() == 0
                && span.semantic_function().index() == 0
                && span.rule() == SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap
        })
        .collect::<Vec<_>>();
    let [span] = spans.as_slice() else {
        panic!("one exact synthetic observer trap: {spans:?}");
    };
    assert_eq!(span.first_operation_ordinal(), 0);
    assert_eq!(span.operation_count(), 1);
    assert_eq!(
        abort_block.terminator,
        Some(Terminator::Branch {
            target: span.kernel_ir_block(),
            arguments: vec![],
        })
    );
    let trap = body
        .blocks
        .iter()
        .find(|block| block.id == span.kernel_ir_block())
        .unwrap();
    assert_eq!(
        trap.operations,
        [AmdGpuDiagnosticOperation::Trap.operation(None)]
    );
    assert_eq!(trap.terminator, Some(Terminator::Unreachable));
    assert_eq!(
        body.blocks
            .iter()
            .find(|block| block.id == returned)
            .unwrap()
            .terminator,
        Some(Terminator::Return { values: vec![] })
    );
    selector
}

pub(super) fn assert_carrier_comparison(owner: &ProductionSemanticKirOwnerV1, input: ValueId) {
    let body = owner.module().functions[0].body.as_ref().unwrap();
    let block = body
        .blocks
        .iter()
        .find(|block| block.id == execution_block(owner, 1, 0))
        .unwrap();
    let compares = block
        .operations
        .iter()
        .filter(|operation| matches!(operation.kind, OperationKind::Compare { .. }))
        .collect::<Vec<_>>();
    let [compare] = compares.as_slice() else {
        panic!("one carrier comparison: {compares:?}");
    };
    let OperationKind::Compare {
        predicate,
        lhs,
        rhs,
    } = compare.kind
    else {
        unreachable!()
    };
    assert_eq!(predicate, fe2o3_kernel_ir::ComparePredicate::NotEqual);
    assert_eq!(lhs, input);
    let literal = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.results.iter().any(|result| result.id == rhs))
        .unwrap();
    assert_eq!(
        literal.kind,
        OperationKind::Constant(fe2o3_kernel_ir::Constant::U16(0x7f80))
    );
    assert_eq!(compare.results.len(), 1);
    assert_eq!(compare.results[0].ty, Type::BOOL);
    assert_eq!(observed_selector(owner, 1), compare.results[0].id);
}

pub(super) fn assert_barrier(
    owner: &ProductionSemanticKirOwnerV1,
    function: u32,
    source_block: u32,
) {
    use fe2o3_kernel_ir::{Convergence, MemoryEffect, MemoryOrdering, SynchronizationScope};
    use std::collections::BTreeSet;
    let body = owner.module().functions[0].body.as_ref().unwrap();
    let at = execution_block(owner, function, source_block);
    let barriers = body
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .operations
                .iter()
                .enumerate()
                .filter_map(move |(ordinal, operation)| {
                    if let OperationKind::WorkgroupBarrier(barrier) = &operation.kind {
                        Some((block.id, ordinal, operation, barrier))
                    } else {
                        None
                    }
                })
        })
        .collect::<Vec<_>>();
    let [(block, ordinal, operation, barrier)] = barriers.as_slice() else {
        panic!("one exact barrier: {barriers:?}");
    };
    assert_eq!(*block, at);
    assert!(operation.results.is_empty());
    assert_eq!(barrier.memory_scope, SynchronizationScope::Workgroup);
    assert_eq!(barrier.semantics.ordering, MemoryOrdering::AcquireRelease);
    assert_eq!(
        barrier.semantics.address_spaces,
        BTreeSet::from([AddressSpace::Workgroup])
    );
    assert_eq!(
        barrier.convergence,
        Convergence::uniform(SynchronizationScope::Workgroup)
    );
    let expected = MemoryEffect::Synchronize {
        execution_scope: SynchronizationScope::Workgroup,
        memory_scope: SynchronizationScope::Workgroup,
        address_spaces: BTreeSet::from([AddressSpace::Workgroup]),
    };
    assert_eq!(operation.memory_effects(), [expected.clone()]);
    assert!(operation.has_complete_effect_summary());
    let effects = analyze_interprocedural_effects_v1(owner.module()).unwrap();
    let summary = effects.function(&owner.module().functions[0].id).unwrap();
    assert!(summary.is_complete());
    assert!(!summary.is_complete_and_pure());
    assert_eq!(summary.summary().effects(), &BTreeSet::from([expected]));
    let spans = owner
        .correspondence()
        .terminator_operation_spans()
        .iter()
        .filter(|span| {
            span.correspondence_owner().index() == 0
                && span.semantic_function().index() == 0
                && span.kernel_ir_block() == at
        })
        .collect::<Vec<_>>();
    let [span] = spans.as_slice() else {
        panic!("one barrier source span: {spans:?}");
    };
    assert_eq!(span.semantic_function().index(), 0);
    assert_eq!(
        span.semantic_block(),
        execution_semantic_block(owner, function, source_block)
    );
    assert_eq!(span.first_operation_ordinal(), *ordinal as u32);
    assert_eq!(span.operation_count(), 1);
}

pub(super) fn v6_round_trip(owner: &ProductionSemanticKirOwnerV1) {
    let limits = fe2o3_mir_model::SemanticU32InductionAnalysisLimitsV1::default();
    let root = SemanticFunctionIdV1::from_index(0);
    let report =
        fe2o3_mir_model::analyze_expanded_semantic_u32_induction_no_overflow_with_limits_v1(
            owner.semantic().semantic(),
            owner.semantic_ssa().execution_expansion(),
            root,
            limits,
        )
        .unwrap();
    let evidence = InertCanonicalMirToKirCorrespondenceEvidenceV6::from_live_owner(
        owner,
        &[(root, &report)],
        limits,
    )
    .unwrap();
    let decoded =
        InertCanonicalMirToKirCorrespondenceEvidenceV6::decode(evidence.canonical_bytes()).unwrap();
    assert_eq!(decoded, evidence);
    assert_eq!(decoded.execution_correspondence(), owner.correspondence());
    assert!(
        !decoded
            .verify_replay(owner, limits)
            .unwrap()
            .grants_authority()
    );
}

pub(super) fn call_free_roster() -> ProductionSemanticKirOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let source = SemanticSourceProvenanceV1::unavailable();
    let exports = ["roster_first125", "roster_second125"];
    let functions = exports
        .iter()
        .enumerate()
        .map(|(index, export)| {
            let tag = 20 + index as u8;
            let abi = SemanticFunctionAbiV1::from_rustc(
                SemanticAbiIdentityV1::from_sha256(bytes(tag)),
                SemanticLayoutIdentityV1::from_sha256(bytes(250)),
                SemanticCanonAbiV1::GpuKernel,
                SemanticExternAbiV1::GpuKernel,
                false,
                false,
                0,
                vec![],
                SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
            )
            .unwrap();
            let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
            SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
                SemanticFunctionRoleV1::KernelRoot,
                SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag)),
                SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag)),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag)),
                SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag)),
                source,
                abi,
                vec![return_local()],
                SemanticBlockIdV1::from_index(0),
                vec![block(tag, vec![], SemanticTerminatorKindV1::Return)],
            )
            .unwrap()
            .with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(export.as_bytes().to_vec()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256(bytes(tag)),
                SemanticKernelSourceContractV1::new(
                    Some(
                        SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                            .unwrap(),
                    ),
                    None,
                    None,
                )
                .unwrap(),
            ))
        })
        .collect();
    let source = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![unit_type()],
        vec![],
        vec![],
        vec![],
        functions,
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(1),
        ],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let roots = exports
        .iter()
        .enumerate()
        .map(|(index, export)| {
            let kernel = ProductionRankedKernelV1::new(
                *export,
                0,
                vec![ProductionRankedBlockV1::new(
                    vec![ProductionRankedOperationV1::ExecutionLayout {
                        grid_identity: 1,
                        global_extents: [64, 1, 1],
                        workgroup_extents: [64, 1, 1],
                        subgroup_size: 64,
                        full_physical_workgroups: true,
                    }],
                    ProductionRankedTerminatorV1::Return,
                )],
            )
            .unwrap();
            let input = compile_ranked_kernel_for_lowering_v1(
                ProductionConstructionV1::ranked_kernel(*export, kernel).unwrap(),
                ProductionSessionLimitsV1::default(),
            )
            .unwrap();
            ProductionRankedSemanticProjectionRootV1::new(
                SemanticFunctionIdV1::from_index(index as u32),
                1,
                input,
                format!("func @{export} {{ kernel.return }}"),
                vec![],
                vec![],
            )
        })
        .collect();
    let receipt = ProductionRankedSemanticProjectionModuleReceiptV1::from_unvalidated_projection_roster_candidate(
        ProductionSemanticMirOwnerV1::try_new(source, ProductionSemanticMirLimitsV1::default()).unwrap(), roots,
    ).unwrap();
    let owner = ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks(
        receipt,
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    assert!(!owner.has_expanded_calls());
    assert_eq!(owner.module().kernels.len(), 2);
    owner
}
