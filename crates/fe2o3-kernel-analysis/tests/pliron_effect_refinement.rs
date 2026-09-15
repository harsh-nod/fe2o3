use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
use dialect_kernel::{
    AccessKindAttr, AtomicOrderingAttr, AtomicScopeAttr, BranchOp, DIALECT_NAME, DimensionOp,
    IndexConstantOp, IndexLessThanBranchOp, IndexType, InvocationIndexOp, MemorySpaceAttr,
    OwnershipContractOp, OwnershipCoverageAttr, OwnershipPartitionAttr, RankedAccessOp,
    RankedViewOp, RankedViewType, ReturnOp, SemanticBinaryKindAttr, SemanticBinaryOp,
    SemanticSymbolOp, register_dialect,
};
use dialect_proof::{
    CoveredBoundaryAttr, EvidenceRefOp, EvidenceStatusAttr, ObligationOp, ProofIdAttr,
    PropertyAttr, RequireEffectRefinementOp,
};
use fe2o3_kernel_analysis::{
    KernelCheckStatusV1, PlironEffectRefinementFindingV1, ProductionPlironPreloweringErrorV2,
    require_pliron_effect_refinement_before_lowering_v1,
    require_production_pliron_checks_before_lowering_v2, run_pliron_effect_refinement_check_v1,
    run_pliron_semantic_refinement_check_v1,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp, types::FunctionType},
    common_traits::Named,
    context::Context,
    dialect::DialectName,
    linked_list::ContainsLinkedList,
    op::Op,
    operation::Operation,
    r#type::TypeHandle,
    value::Value,
};

const OBLIGATION: [u64; 4] = [1, 2, 3, 4];

#[derive(Clone, Copy)]
enum FormulaCase {
    Equivalent,
    DomainMismatch,
    PreconditionMismatch,
    ValueMismatch,
}

#[derive(Clone, Copy)]
enum ExtentCase {
    Static,
    ConstantDynamic(Value),
    RuntimeDynamic,
}

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    dialect_proof::register_dialect(&mut context).unwrap();
    context
}

fn append(
    context: &Context,
    block: pliron::context::Ptr<pliron::basic_block::BasicBlock>,
    op: &dyn Op,
) {
    op.get_operation().insert_at_back(block, context);
}

fn semantic_pair(
    context: &mut Context,
    case: FormulaCase,
) -> (
    Vec<pliron::context::Ptr<pliron::operation::Operation>>,
    [Value; 6],
) {
    let a = SemanticSymbolOp::new(context, 10);
    let b = SemanticSymbolOp::new(context, 11);
    let c = SemanticSymbolOp::new(context, 12);
    let d = SemanticSymbolOp::new(context, 13);
    let domain_gpu = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Add,
        a.result(context),
        b.result(context),
    );
    let domain_reference = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Add,
        if matches!(case, FormulaCase::DomainMismatch) {
            a.result(context)
        } else {
            b.result(context)
        },
        if matches!(case, FormulaCase::DomainMismatch) {
            c.result(context)
        } else {
            a.result(context)
        },
    );
    let precondition_gpu = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Multiply,
        a.result(context),
        c.result(context),
    );
    let precondition_reference = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Multiply,
        if matches!(case, FormulaCase::PreconditionMismatch) {
            b.result(context)
        } else {
            c.result(context)
        },
        a.result(context),
    );
    let value_gpu = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Add,
        c.result(context),
        d.result(context),
    );
    let value_reference = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Add,
        if matches!(case, FormulaCase::ValueMismatch) {
            a.result(context)
        } else {
            d.result(context)
        },
        c.result(context),
    );
    let operations = vec![
        a.get_operation(),
        b.get_operation(),
        c.get_operation(),
        d.get_operation(),
        domain_gpu.get_operation(),
        domain_reference.get_operation(),
        precondition_gpu.get_operation(),
        precondition_reference.get_operation(),
        value_gpu.get_operation(),
        value_reference.get_operation(),
    ];
    (
        operations,
        [
            domain_gpu.result(context),
            domain_reference.result(context),
            precondition_gpu.result(context),
            precondition_reference.result(context),
            value_gpu.result(context),
            value_reference.result(context),
        ],
    )
}

fn effect_function(
    context: &mut Context,
    case: FormulaCase,
    ownership: bool,
    policy_checked_staging: bool,
    extent: ExtentCase,
    orphan: bool,
    unmodeled_write: bool,
) -> FuncOp {
    effect_function_with_coverage(
        context,
        case,
        ownership,
        policy_checked_staging,
        extent,
        orphan,
        unmodeled_write,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn effect_function_with_coverage(
    context: &mut Context,
    case: FormulaCase,
    ownership: bool,
    policy_checked_staging: bool,
    extent: ExtentCase,
    orphan: bool,
    unmodeled_write: bool,
    coverage_override: Option<OwnershipCoverageAttr>,
) -> FuncOp {
    let argument_types: Vec<TypeHandle> = if matches!(extent, ExtentCase::RuntimeDynamic) {
        vec![IndexType::get(context).into()]
    } else {
        vec![]
    };
    let function = FuncOp::new(
        context,
        "effect_kernel".try_into().unwrap(),
        FunctionType::get(context, argument_types, vec![]),
    );
    let entry = function.get_entry_block(context);
    let guarded = matches!(extent, ExtentCase::RuntimeDynamic);
    let body = guarded.then(|| {
        let block = BasicBlock::new(context, Some("write".try_into().unwrap()), vec![]);
        block.insert_at_back(function.get_region(context), context);
        block
    });
    let exit = guarded.then(|| {
        let block = BasicBlock::new(context, Some("exit".try_into().unwrap()), vec![]);
        block.insert_at_back(function.get_region(context), context);
        block
    });
    let write_block = body.unwrap_or(entry);
    let layout = ExecutionLayoutOp::new_with_domain(
        context,
        41,
        [4, 1, 1],
        [2, 1, 1],
        2,
        ExecutionDomainAttr::FullPhysicalWorkgroups,
    );
    let invocation = InvocationIndexOp::new(context, 0, 4);
    let shape = if matches!(extent, ExtentCase::Static) {
        vec![4]
    } else {
        vec![0]
    };
    let extent_values = match extent {
        ExtentCase::Static => vec![],
        ExtentCase::ConstantDynamic(value) => vec![value],
        ExtentCase::RuntimeDynamic => vec![entry.deref(context).arguments().next().unwrap()],
    };
    let view_type = RankedViewType::new(context, 32, true, shape).unwrap();
    let view = RankedViewOp::new_in_space_with_allocation_contract(
        context,
        view_type,
        extent_values,
        MemorySpaceAttr::Global,
        17,
        17,
    )
    .unwrap();
    append(context, entry, &layout);
    append(context, entry, &invocation);
    append(context, entry, &view);
    if ownership {
        let contract = OwnershipContractOp::new(
            context,
            view.result(context),
            coverage_override.unwrap_or(if guarded {
                OwnershipCoverageAttr::ExactEffectDomain
            } else {
                OwnershipCoverageAttr::ExactView
            }),
            OwnershipPartitionAttr::ExactSets,
        )
        .unwrap();
        append(context, entry, &contract);
    }
    let (semantic_operations, expressions) = semantic_pair(context, case);
    for operation in semantic_operations {
        operation.insert_at_back(entry, context);
    }
    let obligation = ObligationOp::new(
        context,
        ProofIdAttr::new(OBLIGATION),
        ProofIdAttr::new([5, 6, 7, 8]),
        ProofIdAttr::new([9, 10, 11, 12]),
        PropertyAttr::FunctionalRefinement,
    );
    append(context, entry, &obligation);
    if policy_checked_staging {
        let evidence = EvidenceRefOp::new(
            context,
            ProofIdAttr::new([13, 14, 15, 16]),
            ProofIdAttr::new(OBLIGATION),
            PropertyAttr::FunctionalRefinement,
            EvidenceStatusAttr::Checked,
            CoveredBoundaryAttr::Mir,
        );
        append(context, entry, &evidence);
    }
    let store = RankedAccessOp::new_value(
        context,
        AccessKindAttr::Write,
        view.result(context),
        vec![invocation.result(context)],
        expressions[4],
    )
    .unwrap();
    append(context, write_block, &store);
    let contract_index = if orphan {
        let zero = IndexConstantOp::new(context, 0);
        append(context, write_block, &zero);
        zero.result(context)
    } else {
        invocation.result(context)
    };
    let effect = RequireEffectRefinementOp::new(
        context,
        ProofIdAttr::new(OBLIGATION),
        view.result(context),
        vec![contract_index],
        vec![expressions[0]],
        vec![expressions[0]],
        expressions[0],
        expressions[1],
        expressions[2],
        expressions[3],
        expressions[4],
        expressions[5],
    );
    append(context, write_block, &effect);
    if unmodeled_write {
        let second_type = RankedViewType::new(context, 32, true, vec![4]).unwrap();
        let second_view = RankedViewOp::new_in_space_with_allocation_contract(
            context,
            second_type,
            vec![],
            MemorySpaceAttr::Global,
            18,
            18,
        )
        .unwrap();
        append(context, entry, &second_view);
        let second = RankedAccessOp::new(
            context,
            AccessKindAttr::Write,
            second_view.result(context),
            vec![invocation.result(context)],
        )
        .unwrap();
        append(context, write_block, &second);
    }
    if let (Some(body), Some(exit)) = (body, exit) {
        let dimension = DimensionOp::new(context, view.result(context), 0).unwrap();
        let guard = IndexLessThanBranchOp::new(
            context,
            invocation.result(context),
            dimension.result(context),
            body,
            exit,
        );
        append(context, entry, &dimension);
        append(context, entry, &guard);
        let to_exit = BranchOp::new(context, exit);
        let ret = ReturnOp::new(context);
        append(context, body, &to_exit);
        append(context, exit, &ret);
    } else {
        let ret = ReturnOp::new(context);
        append(context, entry, &ret);
    }
    function
}

#[test]
fn normalized_effects_prove_every_coordinate_across_the_hierarchy() {
    let context = &mut setup();
    let function = effect_function(
        context,
        FormulaCase::Equivalent,
        true,
        true,
        ExtentCase::Static,
        false,
        false,
    );
    let report = run_pliron_effect_refinement_check_v1(context, &function);
    assert!(report.is_clean(), "{:#?}", report.findings());
    assert_eq!(report.contract_count(), 1);
    assert_eq!(report.proved_contract_count(), 1);
    assert!(report.all_declared_effects_are_proved());
    assert!(!report.grants_compiler_refinement_authority());
}

#[test]
fn effect_claims_must_name_the_actual_stored_ssa_even_when_formulas_are_equivalent() {
    for substitute_equivalent in [false, true] {
        let context = &mut setup();
        let function = effect_function(
            context,
            FormulaCase::Equivalent,
            true,
            true,
            ExtentCase::Static,
            false,
            false,
        );
        assert!(run_pliron_effect_refinement_check_v1(context, &function).is_clean());
        let operations: Vec<_> = function
            .get_entry_block(context)
            .deref(context)
            .iter(context)
            .collect();
        let store = operations
            .iter()
            .copied()
            .find(|op| Operation::is_op::<RankedAccessOp>(*op, context))
            .unwrap();
        let contract = operations
            .iter()
            .copied()
            .find(|op| Operation::is_op::<RequireEffectRefinementOp>(*op, context))
            .unwrap();
        let contract = RequireEffectRefinementOp::from_operation(contract);
        let original = contract.gpu_value(context);
        let replacement = if substitute_equivalent {
            contract.reference_value(context)
        } else {
            contract.gpu_domain(context)
        };
        assert_ne!(original, replacement);
        Operation::replace_operand(store, context, 2, replacement);
        let report = run_pliron_effect_refinement_check_v1(context, &function);
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        assert_eq!(report.proved_contract_count(), 0);
        assert!(matches!(
            report.findings(),
            [
                PlironEffectRefinementFindingV1::StoredValueBindingMismatch {
                    actual: Some(_),
                    ..
                }
            ]
        ));
        Operation::replace_operand(store, context, 2, original);
        assert!(run_pliron_effect_refinement_check_v1(context, &function).is_clean());
        Operation::remove_operand(store, context, 2);
        pliron::op::verify_op(&RankedAccessOp::from_operation(store), context).unwrap();
        let report = run_pliron_effect_refinement_check_v1(context, &function);
        assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
        assert_eq!(report.proved_contract_count(), 0);
        assert!(matches!(
            report.findings(),
            [PlironEffectRefinementFindingV1::StoredValueBindingMismatch { actual: None, .. }]
        ));
        assert!(
            report.findings()[0]
                .to_string()
                .contains("FE2O3-EFFECT-010")
        );
    }
}

#[test]
fn atomic_rmw_input_does_not_receive_final_stored_value_refinement_credit() {
    let context = &mut setup();
    let function = effect_function(
        context,
        FormulaCase::Equivalent,
        true,
        true,
        ExtentCase::Static,
        false,
        false,
    );
    let store = function
        .get_entry_block(context)
        .deref(context)
        .iter(context)
        .find(|op| Operation::is_op::<RankedAccessOp>(*op, context))
        .unwrap();
    let store = RankedAccessOp::from_operation(store);
    store.set_attr_kernel_access_kind(context, AccessKindAttr::AtomicReadModifyWrite);
    store.set_attr_kernel_atomic_ordering(context, AtomicOrderingAttr::Relaxed);
    store.set_attr_kernel_atomic_scope(context, AtomicScopeAttr::Device);
    pliron::op::verify_op(&store, context).unwrap();
    let report = run_pliron_effect_refinement_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert_eq!(report.proved_contract_count(), 0);
    assert!(matches!(
        report.findings(),
        [PlironEffectRefinementFindingV1::UnsupportedWriteSemantics {
            kind: AccessKindAttr::AtomicReadModifyWrite,
            ..
        }]
    ));
    assert!(
        report.findings()[0]
            .to_string()
            .contains("FE2O3-EFFECT-011")
    );
}

#[test]
fn total_output_coverage_composes_with_per_coordinate_effect_refinement() {
    let context = &mut setup();
    let function = effect_function_with_coverage(
        context,
        FormulaCase::Equivalent,
        true,
        true,
        ExtentCase::Static,
        false,
        false,
        Some(OwnershipCoverageAttr::TotalView),
    );
    let report = run_pliron_effect_refinement_check_v1(context, &function);
    assert!(report.is_clean(), "{:#?}", report.findings());
    assert!(report.all_declared_effects_are_proved());
}

#[test]
fn mismatches_are_classified_without_fabricating_dynamic_witnesses() {
    for (case, component) in [
        (FormulaCase::DomainMismatch, "domain"),
        (FormulaCase::PreconditionMismatch, "precondition"),
        (FormulaCase::ValueMismatch, "value"),
    ] {
        let context = &mut setup();
        let function = effect_function(context, case, true, true, ExtentCase::Static, false, false);
        let report = run_pliron_effect_refinement_check_v1(context, &function);
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        let finding = &report.findings()[0];
        let witness = match finding {
            PlironEffectRefinementFindingV1::DomainMismatch { witness, .. }
                if component == "domain" =>
            {
                witness
            }
            PlironEffectRefinementFindingV1::PreconditionMismatch { witness, .. }
                if component == "precondition" =>
            {
                witness
            }
            PlironEffectRefinementFindingV1::ValueMismatch { witness, .. }
                if component == "value" =>
            {
                witness
            }
            other => panic!("unexpected {component} finding: {other:?}"),
        };
        assert!(witness.is_none());
        assert!(
            finding
                .to_string()
                .contains(&format!("{component} mismatch"))
        );
    }
}

#[test]
fn missing_or_ambiguous_write_models_fail_closed() {
    let context = &mut setup();
    let missing = effect_function(
        context,
        FormulaCase::Equivalent,
        true,
        true,
        ExtentCase::Static,
        false,
        true,
    );
    let report = run_pliron_effect_refinement_check_v1(context, &missing);
    assert!(matches!(
        report.findings(),
        [PlironEffectRefinementFindingV1::UnmodeledWriteSite { .. }]
    ));
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);

    let context = &mut setup();
    let orphan = effect_function(
        context,
        FormulaCase::Equivalent,
        true,
        true,
        ExtentCase::Static,
        true,
        false,
    );
    assert!(matches!(
        run_pliron_effect_refinement_check_v1(context, &orphan).findings(),
        [PlironEffectRefinementFindingV1::OrphanEffectContract { .. }]
    ));
}

fn function_without_effect_contracts(
    context: &mut Context,
    space: Option<MemorySpaceAttr>,
    accesses: &[AccessKindAttr],
) -> (FuncOp, Value) {
    let view_type = RankedViewType::new(context, 32, true, vec![1]).unwrap();
    let arguments = if space.is_none() {
        vec![view_type.into()]
    } else {
        vec![]
    };
    let function = FuncOp::new(
        context,
        "empty_effect_roster".try_into().unwrap(),
        FunctionType::get(context, arguments, vec![]),
    );
    let entry = function.get_entry_block(context);
    let view = if let Some(space) = space {
        let view = RankedViewOp::new_in_space_with_allocation_contract(
            context,
            view_type,
            vec![],
            space,
            17,
            17,
        )
        .unwrap();
        append(context, entry, &view);
        view.result(context)
    } else {
        // A block argument has no defining view operation or known memory space.
        entry.deref(context).arguments().next().unwrap()
    };
    let zero = IndexConstantOp::new(context, 0);
    append(context, entry, &zero);
    for &kind in accesses {
        let access = if kind.is_atomic() {
            RankedAccessOp::new_atomic(
                context,
                kind,
                AtomicOrderingAttr::Relaxed,
                AtomicScopeAttr::System,
                view,
                vec![zero.result(context)],
            )
        } else {
            RankedAccessOp::new(context, kind, view, vec![zero.result(context)])
        }
        .unwrap();
        append(context, entry, &access);
    }
    let ret = ReturnOp::new(context);
    append(context, entry, &ret);
    (function, view)
}

#[test]
fn empty_effect_roster_rejects_every_global_or_unknown_write_site() {
    for space in [Some(MemorySpaceAttr::Global), None] {
        let context = &mut setup();
        let (function, view) = function_without_effect_contracts(
            context,
            space,
            &[
                AccessKindAttr::Write,
                AccessKindAttr::Read,
                AccessKindAttr::AtomicWrite,
                AccessKindAttr::AtomicRead,
                AccessKindAttr::AtomicReadModifyWrite,
            ],
        );
        let report = run_pliron_effect_refinement_check_v1(context, &function);
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected, "{space:?}");
        assert_eq!(report.contract_count(), 0);
        assert_eq!(report.proved_contract_count(), 0);
        assert!(!report.all_declared_effects_are_proved());
        assert!(!report.grants_compiler_refinement_authority());
        assert!(!report.grants_artifact_or_launch_authority());
        assert_eq!(report.findings().len(), 3);
        for (finding, offset) in report.findings().iter().zip([0, 2, 4]) {
            let PlironEffectRefinementFindingV1::UnmodeledWriteSite {
                view: found,
                location,
            } = finding
            else {
                panic!("expected an exact unmodeled write site, got {finding:?}");
            };
            assert_eq!(found, &view.unique_name(context).to_string());
            assert_eq!(location.block(), 0);
            assert_eq!(
                location.operation(),
                1 + usize::from(space.is_some()) + offset
            );
            assert!(finding.to_string().contains("FE2O3-EFFECT-008"));
        }
        let error = require_pliron_effect_refinement_before_lowering_v1(context, &function)
            .expect_err("missing effect contracts must block lowering");
        assert_eq!(error.report(), &report);
        let semantics = run_pliron_semantic_refinement_check_v1(context, &function);
        assert_eq!(semantics.status(), KernelCheckStatusV1::Rejected);
        assert_eq!(semantics.effect_refinement(), &report);
    }
}

#[test]
fn empty_effect_roster_without_global_writes_is_clean_without_proof() {
    for (space, accesses) in [
        (Some(MemorySpaceAttr::Global), vec![]),
        (
            Some(MemorySpaceAttr::Global),
            vec![AccessKindAttr::Read, AccessKindAttr::AtomicRead],
        ),
        (None, vec![AccessKindAttr::Read, AccessKindAttr::AtomicRead]),
        (Some(MemorySpaceAttr::Private), vec![AccessKindAttr::Write]),
        (
            Some(MemorySpaceAttr::Workgroup),
            vec![AccessKindAttr::Write],
        ),
    ] {
        let context = &mut setup();
        let (function, _) = function_without_effect_contracts(context, space, &accesses);
        let report = require_pliron_effect_refinement_before_lowering_v1(context, &function)
            .expect("no observable global writes require an effect contract");
        assert!(report.is_clean());
        assert!(report.findings().is_empty());
        assert_eq!(report.contract_count(), 0);
        assert_eq!(report.proved_contract_count(), 0);
        assert!(!report.all_declared_effects_are_proved());
        assert!(!report.grants_compiler_refinement_authority());
        assert!(!report.grants_artifact_or_launch_authority());
    }
}

#[test]
fn ownership_and_mir_evidence_are_mandatory_prerequisites() {
    let context = &mut setup();
    let no_ownership = effect_function(
        context,
        FormulaCase::Equivalent,
        false,
        true,
        ExtentCase::Static,
        false,
        false,
    );
    assert!(matches!(
        run_pliron_effect_refinement_check_v1(context, &no_ownership).findings(),
        [PlironEffectRefinementFindingV1::MissingOwnershipContract { .. }]
    ));

    let context = &mut setup();
    let no_evidence = effect_function(
        context,
        FormulaCase::Equivalent,
        true,
        false,
        ExtentCase::Static,
        false,
        false,
    );
    assert!(matches!(
        run_pliron_effect_refinement_check_v1(context, &no_evidence).findings(),
        [PlironEffectRefinementFindingV1::ReferenceProofIncomplete { .. }]
    ));
}

#[test]
fn staticized_and_guarded_runtime_dynamic_effect_domains_are_proved() {
    let context = &mut setup();
    let four = IndexConstantOp::new(context, 4);
    let function = effect_function(
        context,
        FormulaCase::Equivalent,
        true,
        true,
        ExtentCase::ConstantDynamic(four.result(context)),
        false,
        false,
    );
    four.get_operation()
        .insert_at_front(function.get_entry_block(context), context);
    assert!(
        run_pliron_effect_refinement_check_v1(context, &function).is_clean(),
        "dynamic shape reduced by sparse facts must be exact"
    );

    let context = &mut setup();
    let runtime = effect_function(
        context,
        FormulaCase::Equivalent,
        true,
        true,
        ExtentCase::RuntimeDynamic,
        false,
        false,
    );
    let report = run_pliron_effect_refinement_check_v1(context, &runtime);
    assert!(report.is_clean(), "{:#?}", report.findings());
}

#[test]
fn production_pipeline_runs_effect_refinement_inside_the_semantic_stage() {
    let context = &mut setup();
    let valid = effect_function(
        context,
        FormulaCase::Equivalent,
        true,
        true,
        ExtentCase::Static,
        false,
        false,
    );
    let report = require_production_pliron_checks_before_lowering_v2(context, &valid).unwrap();
    assert!(
        report
            .semantics()
            .effect_refinement()
            .all_declared_effects_are_proved()
    );

    let context = &mut setup();
    let invalid = effect_function(
        context,
        FormulaCase::ValueMismatch,
        true,
        true,
        ExtentCase::Static,
        false,
        false,
    );
    let error = require_production_pliron_checks_before_lowering_v2(context, &invalid).unwrap_err();
    assert!(matches!(
        error,
        ProductionPlironPreloweringErrorV2::Semantic(_)
    ));
    assert!(error.to_string().contains("value mismatch"));
}
