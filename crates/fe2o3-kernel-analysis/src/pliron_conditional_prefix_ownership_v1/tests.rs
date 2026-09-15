use super::*;
use dialect_gpu::ExecutionDomainAttr;
use dialect_kernel::{
    AtomicOrderingAttr, AtomicScopeAttr, IndexType, IndexValueAttr, RankedViewType,
    SemanticExceptionalValueAttr, SemanticIeeeRoundingAttr, SemanticNumericalContractV1,
    SemanticNumericalPolicyAttr, SemanticOverflowAttr, SemanticScalarKindAttr,
    SemanticTypedBinaryKindAttr, SemanticTypedBinaryOp, SemanticTypedCompareKindAttr,
    SemanticTypedCompareOp, SemanticTypedConstantOp, SemanticTypedExpressionRootOp,
    SemanticTypedExpressionV1, SemanticTypedScalarV1, SemanticTypedSymbolOp,
    SemanticTypedUnaryKindAttr, SemanticTypedUnaryOp, TrapOp,
};
use pliron::{
    basic_block::BasicBlock, builtin::types::FunctionType, context::Ptr, dialect::DialectName,
    r#type::TypeHandle,
};

#[path = "tests/production_values.rs"]
mod production_values;

#[path = "tests/contradictory_traps.rs"]
mod contradictory_traps;

struct Fixture {
    function: FuncOp,
    marker: IndexConstantOp,
    value: SemanticTypedBinaryOp,
}

#[derive(Clone, Copy, Debug)]
enum Mutation {
    None,
    WrongAxis,
    WrongBound,
    DuplicateWrite,
    DeadTrap,
    NonArgumentExtent,
    LayoutMismatch,
    FloatBinary(u16, SemanticTypedBinaryKindAttr),
    FloatConstantNegate,
    TypedIntegerDivision,
    TypedCheckedAdd,
    TypedMismatchedOperand,
    TypedUndefinedOperand,
    TypedForwardOperand,
    TypedCyclicOperand,
    TypedCrossBlockOperand,
    TypedExtraPredicate,
    TypedWrongPolicy,
    LiveTrap,
    HiddenAtomic,
}

fn fixture(context: &mut Context, mutation: Mutation) -> Fixture {
    dialect_kernel::register_dialect(
        context,
        &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    dialect_gpu::register_dialect(context).unwrap();
    let index: TypeHandle = IndexType::get(context).into();
    let function = FuncOp::new(
        context,
        "conditional_prefix".try_into().unwrap(),
        FunctionType::get(context, vec![index; 3], vec![]),
    );
    let entry = function.get_entry_block(context);
    let arguments = entry.deref(context).arguments().collect::<Vec<_>>();
    let mut blocks = vec![entry];
    for name in ["a_guard", "b_guard", "write", "exit"] {
        let block = BasicBlock::new(context, Some(name.try_into().unwrap()), vec![]);
        block.insert_at_back(function.get_region(context), context);
        blocks.push(block);
    }
    let layout = ExecutionLayoutOp::new_with_domain(
        context,
        41,
        [
            if matches!(mutation, Mutation::LayoutMismatch) {
                64
            } else {
                0
            },
            1,
            1,
        ],
        [64, 1, 1],
        64,
        ExecutionDomainAttr::FullPhysicalWorkgroups,
    );
    append(context, entry, &layout);
    let invocation = InvocationIndexOp::new(
        context,
        u32::from(matches!(mutation, Mutation::WrongAxis)),
        0,
    );
    append(context, entry, &invocation);
    let marker = IndexConstantOp::new(context, 0);
    append(context, entry, &marker);
    let mut views = Vec::new();
    let mut dimensions = Vec::new();
    for (i, argument) in arguments.iter().enumerate() {
        let ty = RankedViewType::new(context, 32, i == 0, vec![DYNAMIC_EXTENT]).unwrap();
        let extent = if i == 0 && matches!(mutation, Mutation::NonArgumentExtent) {
            marker.result(context)
        } else {
            *argument
        };
        let view = RankedViewOp::new_in_space_with_allocation_contract(
            context,
            ty,
            vec![extent],
            MemorySpaceAttr::Global,
            17 + i as u64,
            17 + i as u64,
        )
        .unwrap();
        append(context, entry, &view);
        let dim = DimensionOp::new(context, view.result(context), 0).unwrap();
        append(context, entry, &dim);
        dimensions.push(dim.result(context));
        views.push(view.result(context));
    }
    let ownership = OwnershipContractOp::new(
        context,
        views[0],
        OwnershipCoverageAttr::TotalView,
        OwnershipPartitionAttr::ExactSets,
    )
    .unwrap();
    append(context, entry, &ownership);
    for i in 0..3 {
        if i == 2 {
            let read = RankedAccessOp::new(
                context,
                AccessKindAttr::Read,
                views[1],
                vec![invocation.result(context)],
            )
            .unwrap();
            append(context, blocks[i], &read);
        }
        let bound = if i == 0 && matches!(mutation, Mutation::WrongBound) {
            marker.result(context)
        } else {
            dimensions[i]
        };
        let branch = IndexLessThanBranchOp::new(
            context,
            invocation.result(context),
            bound,
            blocks[i + 1],
            blocks[4],
        );
        append(context, blocks[i], &branch);
    }
    let read = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        views[2],
        vec![invocation.result(context)],
    )
    .unwrap();
    append(context, blocks[3], &read);
    let value = append_float_expression(context, entry, blocks[3], mutation);
    if matches!(mutation, Mutation::HiddenAtomic) {
        let atomic = RankedAccessOp::new_atomic(
            context,
            AccessKindAttr::AtomicReadModifyWrite,
            AtomicOrderingAttr::Relaxed,
            AtomicScopeAttr::Agent,
            views[0],
            vec![invocation.result(context)],
        )
        .unwrap();
        append(context, blocks[3], &atomic);
    }
    for _ in 0..if matches!(mutation, Mutation::DuplicateWrite) {
        2
    } else {
        1
    } {
        let write = RankedAccessOp::new(
            context,
            AccessKindAttr::Write,
            views[0],
            vec![invocation.result(context)],
        )
        .unwrap();
        append(context, blocks[3], &write);
    }
    let branch = BranchOp::new(context, blocks[4]);
    append(context, blocks[3], &branch);
    let ret = ReturnOp::new(context);
    append(context, blocks[4], &ret);
    if matches!(mutation, Mutation::DeadTrap) {
        let block = BasicBlock::new(context, Some("dead_trap".try_into().unwrap()), vec![]);
        block.insert_at_back(function.get_region(context), context);
        let trap = TrapOp::new(context);
        append(context, block, &trap);
    }
    Fixture {
        function,
        marker,
        value,
    }
}

fn append(context: &Context, block: Ptr<BasicBlock>, op: &impl Op) {
    op.get_operation().insert_at_back(block, context);
}

fn append_float_expression(
    context: &mut Context,
    entry: Ptr<BasicBlock>,
    block: Ptr<BasicBlock>,
    mutation: Mutation,
) -> SemanticTypedBinaryOp {
    let (kind, bits, operation) = match mutation {
        Mutation::FloatBinary(bits, operation) => (SemanticScalarKindAttr::Float, bits, operation),
        Mutation::TypedIntegerDivision => (
            SemanticScalarKindAttr::UnsignedInteger,
            32,
            SemanticTypedBinaryKindAttr::Divide,
        ),
        _ => (
            SemanticScalarKindAttr::Float,
            32,
            SemanticTypedBinaryKindAttr::Add,
        ),
    };
    let scalar = SemanticTypedScalarV1::new(kind, bits).unwrap();
    let lhs = SemanticTypedSymbolOp::new(context, 71, scalar);
    if matches!(mutation, Mutation::TypedCrossBlockOperand) {
        lhs.get_operation().insert_at_front(entry, context);
    } else if !matches!(
        mutation,
        Mutation::TypedUndefinedOperand | Mutation::TypedForwardOperand
    ) {
        append(context, block, &lhs);
    }
    let rhs_scalar = if matches!(mutation, Mutation::TypedMismatchedOperand) {
        SemanticTypedScalarV1::new(SemanticScalarKindAttr::Float, 64).unwrap()
    } else {
        scalar
    };
    let rhs = SemanticTypedSymbolOp::new(context, 72, rhs_scalar);
    append(context, block, &rhs);
    let mut rhs_value = rhs.result(context);
    let mut rhs_expression = SemanticTypedExpressionV1::Symbol {
        symbol: 72,
        scalar: rhs_scalar,
    };
    if matches!(mutation, Mutation::FloatConstantNegate) {
        let constant = SemanticTypedConstantOp::new(context, 0, scalar);
        append(context, block, &constant);
        let negate = SemanticTypedUnaryOp::new(
            context,
            SemanticTypedUnaryKindAttr::Negate,
            scalar,
            constant.result(context),
        );
        append(context, block, &negate);
        rhs_value = negate.result(context);
        rhs_expression = SemanticTypedExpressionV1::Unary {
            operation: SemanticTypedUnaryKindAttr::Negate,
            scalar,
            operand: Box::new(SemanticTypedExpressionV1::Constant { scalar, bits: 0 }),
        };
    }
    let overflow = if matches!(mutation, Mutation::TypedCheckedAdd) {
        SemanticOverflowAttr::Checked
    } else {
        SemanticOverflowAttr::Wrapping
    };
    let binary = SemanticTypedBinaryOp::new(
        context,
        operation,
        overflow,
        scalar,
        lhs.result(context),
        rhs_value,
    );
    append(context, block, &binary);
    if matches!(mutation, Mutation::TypedCyclicOperand) {
        Operation::replace_operand(binary.get_operation(), context, 0, binary.result(context));
    }
    if matches!(mutation, Mutation::TypedForwardOperand) {
        append(context, block, &lhs);
    }
    if matches!(mutation, Mutation::TypedExtraPredicate) {
        let compare = SemanticTypedCompareOp::new(
            context,
            SemanticTypedCompareKindAttr::LessThan,
            scalar,
            lhs.result(context),
            rhs.result(context),
        );
        append(context, block, &compare);
    }
    if matches!(mutation, Mutation::LiveTrap) {
        let trap = TrapOp::new(context);
        append(context, block, &trap);
    }
    let contract = SemanticNumericalContractV1 {
        policy: if matches!(mutation, Mutation::TypedWrongPolicy) {
            SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence
        } else {
            SemanticNumericalPolicyAttr::ExactIeeeNearestTiesToEvenPreserveBits
        },
        rounding: SemanticIeeeRoundingAttr::NearestTiesToEven,
        exceptional_values: SemanticExceptionalValueAttr::PreserveExactBits,
    };
    let expression = SemanticTypedExpressionV1::Binary {
        operation,
        scalar,
        overflow,
        lhs: Box::new(SemanticTypedExpressionV1::Symbol { symbol: 71, scalar }),
        rhs: Box::new(rhs_expression),
    };
    let digest = expression.canonical_transcript_sha256(contract);
    let mut commitment = [0_u64; 4];
    for (word, bytes) in commitment.iter_mut().zip(digest.chunks_exact(8)) {
        *word = u64::from_le_bytes(bytes.try_into().unwrap());
    }
    let root = SemanticTypedExpressionRootOp::new(
        context,
        binary.result(context),
        contract.policy,
        contract.rounding,
        contract.exceptional_values,
        commitment,
    );
    append(context, block, &root);
    binary
}

#[test]
fn live_float_value_dag_preserves_conditional_not_numerical_authority() {
    for bits in [32, 64] {
        for operation in [
            SemanticTypedBinaryKindAttr::Add,
            SemanticTypedBinaryKindAttr::Subtract,
            SemanticTypedBinaryKindAttr::Multiply,
            SemanticTypedBinaryKindAttr::Divide,
            SemanticTypedBinaryKindAttr::Remainder,
        ] {
            let mut context = Context::new();
            let fixture = fixture(&mut context, Mutation::FloatBinary(bits, operation));
            let record =
                derive_pliron_conditional_prefix_coverage_v1(&context, &fixture.function).unwrap();
            assert_eq!(record.conditions().len(), 3);
            assert!(!record.grants_launch_authority());
            let report =
                crate::run_pliron_hierarchical_ownership_check_v1(&context, &fixture.function);
            assert!(report.conditional_prefix_coverage().is_some());
            assert_eq!(report.coverage_summary().total_view_proved(), 0);
        }
    }
    let mut context = Context::new();
    let fixture = fixture(&mut context, Mutation::FloatConstantNegate);
    derive_pliron_conditional_prefix_coverage_v1(&context, &fixture.function).unwrap();
    let mut context = Context::new();
    let fixture = self::fixture(&mut context, Mutation::TypedCrossBlockOperand);
    derive_pliron_conditional_prefix_coverage_v1(&context, &fixture.function).unwrap();
}

#[test]
fn live_value_slice_rejects_traps_effects_wrong_types_and_undefined_definitions() {
    for mutation in [
        Mutation::TypedIntegerDivision,
        Mutation::TypedCheckedAdd,
        Mutation::TypedMismatchedOperand,
        Mutation::TypedUndefinedOperand,
        Mutation::TypedForwardOperand,
        Mutation::TypedCyclicOperand,
        Mutation::TypedExtraPredicate,
        Mutation::TypedWrongPolicy,
        Mutation::LiveTrap,
        Mutation::HiddenAtomic,
        Mutation::FloatBinary(32, SemanticTypedBinaryKindAttr::BitXor),
    ] {
        let mut context = Context::new();
        let fixture = fixture(&mut context, mutation);
        assert!(
            derive_pliron_conditional_prefix_coverage_v1(&context, &fixture.function).is_err(),
            "{mutation:?}",
        );
    }
}

#[test]
fn value_only_change_stales_the_record_even_when_ownership_conditions_match() {
    let mut context = Context::new();
    let fixture = fixture(&mut context, Mutation::None);
    let record = derive_pliron_conditional_prefix_coverage_v1(&context, &fixture.function).unwrap();
    fixture.value.set_attr_kernel_semantic_typed_binary_kind(
        &context,
        SemanticTypedBinaryKindAttr::Subtract,
    );
    let changed =
        derive_pliron_conditional_prefix_coverage_v1(&context, &fixture.function).unwrap();
    assert_eq!(record.conditions(), changed.conditions());
    assert_eq!(
        record.host_binding_obligations(),
        changed.host_binding_obligations()
    );
    assert_ne!(record.canonical_bytes(), changed.canonical_bytes());
    assert_eq!(
        revalidate_pliron_conditional_prefix_coverage_v1(&context, &fixture.function, &record),
        Err(ConditionalPrefixDerivationErrorV1::StaleGraph),
    );
    // The old numerical commitment is not certified by the new ownership record.
    assert!(!changed.grants_launch_authority());
    assert!(!changed.proves_unconditional_total_view());
}

#[test]
fn ownership_report_retains_the_exact_conditional_rejection() {
    for (mutation, operation) in [
        (
            Mutation::TypedExtraPredicate,
            "kernel.semantic_typed_compare",
        ),
        (Mutation::LiveTrap, "kernel.trap"),
    ] {
        let mut context = Context::new();
        let f = fixture(&mut context, mutation);
        let error =
            derive_pliron_conditional_prefix_coverage_v1(&context, &f.function).unwrap_err();
        let mut sites = Vec::new();
        for (b, block) in f
            .function
            .get_region(&context)
            .deref(&context)
            .iter(&context)
            .enumerate()
        {
            for (i, pointer) in block.deref(&context).iter(&context).enumerate() {
                let op = Operation::get_op_dyn(pointer, &context);
                if op.downcast_ref::<TrapOp>().is_some()
                    || op.downcast_ref::<SemanticTypedCompareOp>().is_some()
                {
                    sites.push(ConditionalPrefixSiteV1 {
                        block: b as u32,
                        operation: i as u32,
                    });
                }
            }
        }
        let [site] = sites.as_slice() else {
            panic!("one inserted unsupported operation")
        };
        assert_eq!(
            error,
            ConditionalPrefixDerivationErrorV1::UnsupportedOperation {
                site: *site,
                operation
            }
        );
        let report = crate::run_pliron_hierarchical_ownership_check_v1(&context, &f.function);
        assert_eq!(report.conditional_prefix_failure(), Some(&error));
        assert!(report.conditional_prefix_coverage().is_none());
        assert!(!report.is_clean());
        assert!(!report.grants_artifact_or_launch_authority());
    }
    let mut context = Context::new();
    let f = fixture(&mut context, Mutation::None);
    let report = crate::run_pliron_hierarchical_ownership_check_v1(&context, &f.function);
    assert!(report.conditional_prefix_failure().is_none());
    assert!(report.conditional_prefix_coverage().is_some());
    assert!(!report.is_clean());
}

#[test]
fn live_prefix_has_exact_conditions_and_remains_incomplete() {
    let mut context = Context::new();
    let fixture = fixture(&mut context, Mutation::None);
    let record = derive_pliron_conditional_prefix_coverage_v1(&context, &fixture.function).unwrap();
    assert_eq!(record.conditions.len(), 3);
    let [
        ConditionalPrefixConditionV1::OutputExtentAtMostInput {
            output: a_out,
            input: a,
        },
        ConditionalPrefixConditionV1::OutputExtentAtMostInput {
            output: b_out,
            input: b,
        },
        ConditionalPrefixConditionV1::OutputExtentAtMostActualWorkitems { output, launch },
    ] = record.conditions.as_slice()
    else {
        panic!("canonical conditions")
    };
    assert_eq!(*a_out, *output);
    assert_eq!(*b_out, *output);
    assert_eq!(
        output.source(),
        ConditionalPrefixExtentSourceV1::RankedEntryArgument(0)
    );
    assert_eq!(
        a.source(),
        ConditionalPrefixExtentSourceV1::RankedEntryArgument(1)
    );
    assert_eq!(
        b.source(),
        ConditionalPrefixExtentSourceV1::RankedEntryArgument(2)
    );
    assert_eq!(
        (
            output.allocation_origin(),
            a.allocation_origin(),
            b.allocation_origin()
        ),
        (17, 18, 19)
    );
    assert_eq!(launch.axis(), 0);
    assert_eq!(launch.declared_workitems(), 0);
    assert_eq!(
        record.host_binding_obligations(),
        &[
            ConditionalPrefixHostBindingObligationV1::RankedViewToPhysicalAllocationExtent {
                extent: *output,
            },
            ConditionalPrefixHostBindingObligationV1::RankedViewToPhysicalAllocationExtent {
                extent: *a,
            },
            ConditionalPrefixHostBindingObligationV1::RankedViewToPhysicalAllocationExtent {
                extent: *b,
            },
            ConditionalPrefixHostBindingObligationV1::RankedLaunchToActualDispatch {
                launch: *launch,
            },
        ]
    );
    assert_eq!(record.source_guard_dnf.len(), 1);
    assert_eq!(record.source_guard_dnf[0].len(), 3);
    assert!(
        record.source_guard_dnf[0]
            .iter()
            .all(|atom| atom.less_than())
    );
    assert!(!record.proves_unconditional_total_view());
    assert!(!record.grants_launch_authority());
    let report = crate::run_pliron_hierarchical_ownership_check_v1(&context, &fixture.function);
    assert_eq!(report.conditional_prefix_coverage(), Some(&record));
    assert_eq!(report.status(), crate::KernelCheckStatusV1::Incomplete);
    assert_eq!(report.coverage_summary().total_view_declared(), 1);
    assert_eq!(report.coverage_summary().total_view_proved(), 0);
    assert!(!report.all_total_view_contracts_are_proved());
    assert!(
        crate::require_pliron_hierarchical_ownership_before_lowering_v1(
            &context,
            &fixture.function
        )
        .is_err()
    );
}

#[test]
fn live_adapter_rejects_wrong_identity_hidden_traps_and_duplicate_writes() {
    for mutation in [
        Mutation::WrongAxis,
        Mutation::WrongBound,
        Mutation::DuplicateWrite,
        Mutation::DeadTrap,
        Mutation::NonArgumentExtent,
        Mutation::LayoutMismatch,
    ] {
        let mut context = Context::new();
        let fixture = fixture(&mut context, mutation);
        assert!(derive_pliron_conditional_prefix_coverage_v1(&context, &fixture.function).is_err());
        assert!(
            crate::run_pliron_hierarchical_ownership_check_v1(&context, &fixture.function)
                .conditional_prefix_coverage()
                .is_none()
        );
    }
}

#[test]
fn record_is_canonical_live_and_invalid_after_mutate_restore() {
    let mut context = Context::new();
    let fixture = fixture(&mut context, Mutation::None);
    let record = derive_pliron_conditional_prefix_coverage_v1(&context, &fixture.function).unwrap();
    let again = derive_pliron_conditional_prefix_coverage_v1(&context, &fixture.function).unwrap();
    assert_eq!(record, again);
    assert_eq!(record.canonical_bytes(), again.canonical_bytes());
    assert!(record.canonical_bytes().len() < 4096);
    revalidate_pliron_conditional_prefix_coverage_v1(&context, &fixture.function, &record).unwrap();
    for mutation in 0..9 {
        let mut forged = record.clone();
        match mutation {
            0 => {
                forged.conditions.pop();
            }
            1 => forged.source_guard_dnf.clear(),
            2 => forged.write.operation += 1,
            3 => forged.output.allocation_origin += 1,
            4 => forged.launch.grid_identity += 1,
            5 => forged.mutation_epoch += 1,
            6 => forged.source_guard_dnf[0][0].less_than = false,
            7 => forged.host_binding_obligations.clear(),
            8 => forged.host_binding_obligations[0] = record.host_binding_obligations[1],
            _ => unreachable!(),
        }
        assert_ne!(forged.canonical_bytes(), record.canonical_bytes());
        assert_eq!(
            revalidate_pliron_conditional_prefix_coverage_v1(&context, &fixture.function, &forged),
            Err(ConditionalPrefixDerivationErrorV1::StaleGraph)
        );
    }
    fixture
        .marker
        .set_attr_kernel_index_value(&context, IndexValueAttr(1));
    fixture
        .marker
        .set_attr_kernel_index_value(&context, IndexValueAttr(0));
    assert!(record.graph_identity.exactly_matches(
        &derive_pliron_ir_structural_identity_v1(&context, &fixture.function).unwrap()
    ));
    assert_eq!(
        revalidate_pliron_conditional_prefix_coverage_v1(&context, &fixture.function, &record),
        Err(ConditionalPrefixDerivationErrorV1::StaleGraph)
    );
}

fn model() -> PrefixModel {
    let site = |block, operation| ConditionalPrefixSiteV1 { block, operation };
    let views = (0..3)
        .map(|index| ConditionalPrefixExtentV1 {
            view: site(0, index + 2),
            allocation_origin: u64::from(index) + 17,
            noalias_class: u64::from(index) + 17,
            source: ConditionalPrefixExtentSourceV1::RankedEntryArgument(index),
        })
        .collect();
    PrefixModel {
        views,
        writable: vec![true, false, false],
        output: 0,
        contract: site(0, 5),
        invocation: site(0, 1),
        launch: ConditionalPrefixLaunchV1 {
            layout: site(0, 0),
            grid_identity: 41,
            declared_workitems: 0,
        },
        blocks: vec![
            PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::LessThan {
                    site: site(0, 6),
                    extent: 0,
                    yes: 1,
                    no: 4,
                },
            },
            PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::LessThan {
                    site: site(1, 0),
                    extent: 1,
                    yes: 2,
                    no: 4,
                },
            },
            PrefixBlock {
                accesses: vec![(site(2, 0), 1, false)],
                terminator: PrefixTerminator::LessThan {
                    site: site(2, 1),
                    extent: 2,
                    yes: 3,
                    no: 4,
                },
            },
            PrefixBlock {
                accesses: vec![(site(3, 0), 2, false), (site(3, 1), 0, true)],
                terminator: PrefixTerminator::Goto(4),
            },
            PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::Return,
            },
        ],
    }
}

#[test]
fn topological_walk_has_linear_work_even_with_reverse_block_storage() {
    for count in [1, 2, 16, 48, MAX_CONDITIONAL_PREFIX_BLOCKS_V1] {
        let expected = std::iter::once(0)
            .chain((1..count).rev())
            .collect::<Vec<_>>();
        let mut blocks = vec![
            PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::Return
            };
            count
        ];
        for pair in expected.windows(2) {
            blocks[pair[0]].terminator = PrefixTerminator::Goto(pair[1]);
        }
        let cost = 4 * count + 2 * (count - 1);
        let mut used = 0;
        assert_eq!(topological_blocks(&blocks, &mut used).unwrap(), expected);
        assert_eq!(used, cost);
        let mut exact = MAX_CONDITIONAL_PREFIX_WORK_V1 - cost;
        assert_eq!(topological_blocks(&blocks, &mut exact).unwrap(), expected);
        assert_eq!(exact, MAX_CONDITIONAL_PREFIX_WORK_V1);
        let mut short = MAX_CONDITIONAL_PREFIX_WORK_V1 - cost + 1;
        assert_eq!(
            topological_blocks(&blocks, &mut short),
            Err(ConditionalPrefixDerivationErrorV1::Limit("work"))
        );
    }
}

#[test]
fn pure_derivation_rejects_missing_extra_or_negated_guards_and_all_cycles() {
    assert!(derive_model(&model()).is_ok());
    for mutation in 0..10 {
        let mut model = model();
        match mutation {
            0 => model.blocks[0].terminator = PrefixTerminator::Goto(1),
            1 => model.blocks[1].terminator = PrefixTerminator::Goto(2),
            2 => {
                let PrefixTerminator::LessThan { yes, no, .. } = &mut model.blocks[0].terminator
                else {
                    unreachable!()
                };
                std::mem::swap(yes, no);
            }
            3 => model.blocks[4].terminator = PrefixTerminator::Goto(0),
            4 => model.blocks[4].terminator = PrefixTerminator::Goto(99),
            5 => model.blocks[4].accesses.push((
                ConditionalPrefixSiteV1 {
                    block: 4,
                    operation: 0,
                },
                0,
                true,
            )),
            6 => model.blocks.push(PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::Return,
            }),
            7 => model.views[1].allocation_origin = model.views[0].allocation_origin,
            8 => model.views[1].noalias_class = model.views[0].noalias_class,
            9 => {
                model.blocks[4].terminator = PrefixTerminator::LessThan {
                    site: ConditionalPrefixSiteV1 {
                        block: 4,
                        operation: 0,
                    },
                    extent: 0,
                    yes: 5,
                    no: 6,
                }
            }
            _ => unreachable!(),
        }
        if mutation == 9 {
            model.blocks.extend([
                PrefixBlock {
                    accesses: vec![],
                    terminator: PrefixTerminator::Return,
                },
                PrefixBlock {
                    accesses: vec![],
                    terminator: PrefixTerminator::Return,
                },
            ]);
        }
        assert!(derive_model(&model).is_err(), "mutation {mutation}");
    }
}

fn small_extent(binding: ConditionalPrefixExtentV1, lengths: &[u64; 3]) -> u64 {
    match binding.source {
        ConditionalPrefixExtentSourceV1::RankedEntryArgument(i) => lengths[i as usize],
        ConditionalPrefixExtentSourceV1::Constant(value) => value,
    }
}

// Execute actual terminators and accesses, independently of proof paths/DNF.
// The small environment and per-invocation visited set bound the oracle itself.
fn execute_small_cfg(
    model: &PrefixModel,
    lengths: &[u64; 3],
    workitems: u64,
) -> Result<Vec<usize>, &'static str> {
    assert!(model.blocks.len() <= MAX_CONDITIONAL_PREFIX_BLOCKS_V1);
    assert!(workitems <= 8);
    assert!(
        model
            .views
            .iter()
            .all(|&view| small_extent(view, lengths) <= 8)
    );
    let mut writes = vec![0; small_extent(model.views[model.output], lengths) as usize];
    for i in 0..workitems {
        let mut block = 0;
        let mut visited = vec![false; model.blocks.len()];
        loop {
            let current = model.blocks.get(block).ok_or("invalid target")?;
            if std::mem::replace(&mut visited[block], true) {
                return Err("cycle");
            }
            for &(_, view, write) in &current.accesses {
                if i >= small_extent(model.views[view], lengths) {
                    return Err("out-of-bounds access");
                }
                if write {
                    if view != model.output {
                        return Err("unexpected output");
                    }
                    writes[i as usize] += 1;
                }
            }
            block = match current.terminator {
                PrefixTerminator::Return => break,
                PrefixTerminator::Trap(_) => return Err("reachable trap"),
                PrefixTerminator::Goto(target) => target,
                PrefixTerminator::LessThan {
                    extent, yes, no, ..
                } => {
                    if i < small_extent(model.views[extent], lengths) {
                        yes
                    } else {
                        no
                    }
                }
            };
        }
    }
    Ok(writes)
}

fn check_small_theorem(model: &PrefixModel) {
    let proof = derive_model(model).unwrap();
    for n in 0..=6_u64 {
        for a in 0..=6_u64 {
            for b in 0..=6_u64 {
                for workitems in 0..=8_u64 {
                    let lengths = [n, a, b];
                    let holds = proof.conditions.iter().all(|condition| match *condition {
                        ConditionalPrefixConditionV1::OutputExtentAtMostInput { output, input } => {
                            small_extent(output, &lengths) <= small_extent(input, &lengths)
                        }
                        ConditionalPrefixConditionV1::OutputExtentAtMostActualWorkitems {
                            output,
                            ..
                        } => small_extent(output, &lengths) <= workitems,
                    });
                    let observed = match execute_small_cfg(model, &lengths, workitems) {
                        Ok(writes) => writes,
                        Err("reachable trap") if !holds => continue,
                        other => panic!("unexpected execution with conditions={holds}: {other:?}"),
                    };
                    let covers = observed.iter().all(|&count| count == 1);
                    assert_eq!(holds, covers, "n={n}, a={a}, b={b}, workitems={workitems}");
                }
            }
        }
    }
}

#[test]
fn pure_conditions_are_exact_for_short_inputs_padded_launches_and_zero_output() {
    check_small_theorem(&model());
}

fn permuted_guards(order: [usize; 3]) -> PrefixModel {
    let mut model = model();
    model.blocks[2].accesses.clear();
    model.blocks[3].accesses = [(1, false), (2, false), (0, true)]
        .into_iter()
        .enumerate()
        .map(|(operation, (view, writes))| {
            (
                ConditionalPrefixSiteV1 {
                    block: 3,
                    operation: operation as u32,
                },
                view,
                writes,
            )
        })
        .collect();
    for (block, bound) in order.into_iter().enumerate() {
        let PrefixTerminator::LessThan { extent, .. } = &mut model.blocks[block].terminator else {
            unreachable!()
        };
        *extent = bound;
    }
    model
}

#[test]
fn independent_cfg_oracle_checks_guard_permutations_and_reconvergent_exits() {
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut model = permuted_guards(order);
        check_small_theorem(&model);
        // Distinct early-exit blocks reconverge at the common return. A true
        // edge also takes a forward goto to an earlier-listed guard block.
        for guard in 0..3 {
            let exit = model.blocks.len();
            model.blocks.push(PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::Goto(4),
            });
            let PrefixTerminator::LessThan { no, .. } = &mut model.blocks[guard].terminator else {
                unreachable!()
            };
            *no = exit;
        }
        let bridge = model.blocks.len();
        model.blocks.push(PrefixBlock {
            accesses: vec![],
            terminator: PrefixTerminator::Goto(2),
        });
        let PrefixTerminator::LessThan { yes, .. } = &mut model.blocks[1].terminator else {
            unreachable!()
        };
        *yes = bridge;
        check_small_theorem(&model);
    }
}

#[test]
fn independent_cfg_oracle_detects_early_exit_duplicate_writes_and_unguarded_reads() {
    let mut early = permuted_guards([0, 1, 2]);
    let PrefixTerminator::LessThan { yes, no, .. } = &mut early.blocks[2].terminator else {
        unreachable!()
    };
    std::mem::swap(yes, no);
    // Every stated inequality holds, but this CFG returns before the write.
    assert_eq!(execute_small_cfg(&early, &[2, 2, 2], 2), Ok(vec![0, 0]));
    assert!(derive_model(&early).is_err());

    let mut duplicate = permuted_guards([0, 1, 2]);
    let mut write = *duplicate.blocks[3].accesses.last().unwrap();
    write.0.operation += 1;
    duplicate.blocks[3].accesses.push(write);
    assert_eq!(execute_small_cfg(&duplicate, &[2, 2, 2], 2), Ok(vec![2, 2]));
    assert_eq!(
        derive_model(&duplicate).err(),
        Some(ConditionalPrefixDerivationErrorV1::DuplicateWrite)
    );

    let mut unguarded = permuted_guards([0, 1, 2]);
    let mut read = unguarded.blocks[3].accesses.remove(0);
    read.0 = ConditionalPrefixSiteV1 {
        block: 0,
        operation: 0,
    };
    unguarded.blocks[0].accesses.push(read);
    assert_eq!(
        execute_small_cfg(&unguarded, &[2, 1, 2], 2),
        Err("out-of-bounds access")
    );
    assert_eq!(
        derive_model(&unguarded).err(),
        Some(ConditionalPrefixDerivationErrorV1::UnguardedRead)
    );

    let mut cyclic = permuted_guards([0, 1, 2]);
    cyclic.blocks[4].terminator = PrefixTerminator::Goto(0);
    assert_eq!(execute_small_cfg(&cyclic, &[2, 2, 2], 2), Err("cycle"));
    assert_eq!(
        derive_model(&cyclic).err(),
        Some(ConditionalPrefixDerivationErrorV1::CyclicControlFlow)
    );
}

#[test]
fn pure_work_and_shape_budgets_fail_closed() {
    let mut work = 0;
    charge(&mut work, MAX_CONDITIONAL_PREFIX_WORK_V1).unwrap();
    assert!(charge(&mut work, 1).is_err());
    let mut overflowed_work = usize::MAX;
    assert!(charge(&mut overflowed_work, 1).is_err());
    let mut model = model();
    model.blocks.resize(
        MAX_CONDITIONAL_PREFIX_BLOCKS_V1 + 1,
        model.blocks[4].clone(),
    );
    assert_eq!(
        derive_model(&model).err(),
        Some(ConditionalPrefixDerivationErrorV1::Limit("blocks/views"))
    );
}

#[test]
fn pure_dnf_term_and_guard_depth_budgets_fail_closed() {
    let mut branching = model();
    let original = branching.blocks.clone();
    branching.blocks.clear();
    for stage in 0..6 {
        let index = 3 * stage;
        branching.blocks.extend([
            PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::LessThan {
                    site: ConditionalPrefixSiteV1 {
                        block: index as u32,
                        operation: 0,
                    },
                    extent: 0,
                    yes: index + 1,
                    no: index + 2,
                },
            },
            PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::Goto(index + 3),
            },
            PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::Goto(index + 3),
            },
        ]);
    }
    for mut block in original {
        for (site, _, _) in &mut block.accesses {
            site.block += 18;
        }
        match &mut block.terminator {
            PrefixTerminator::Goto(target) => *target += 18,
            PrefixTerminator::LessThan { site, yes, no, .. } => {
                site.block += 18;
                *yes += 18;
                *no += 18;
            }
            PrefixTerminator::Return | PrefixTerminator::Trap(_) => {}
        }
        branching.blocks.push(block);
    }
    assert!(matches!(
        derive_model(&branching).err(),
        Some(ConditionalPrefixDerivationErrorV1::Limit(
            "DNF terms" | "work"
        ))
    ));

    let mut deep = model();
    let write = deep.blocks[3].clone();
    deep.blocks[3].accesses.clear();
    for _ in 0..MAX_CONDITIONAL_PREFIX_GUARD_ATOMS_V1 {
        let index = deep.blocks.len();
        let previous = if index == 5 { 3 } else { index - 1 };
        deep.blocks[previous].terminator = PrefixTerminator::LessThan {
            site: ConditionalPrefixSiteV1 {
                block: previous as u32,
                operation: 0,
            },
            extent: 0,
            yes: index,
            no: 4,
        };
        deep.blocks.push(PrefixBlock {
            accesses: vec![],
            terminator: PrefixTerminator::Return,
        });
    }
    *deep.blocks.last_mut().unwrap() = write;
    assert_eq!(
        derive_model(&deep).err(),
        Some(ConditionalPrefixDerivationErrorV1::Limit("guard atoms"))
    );
}

#[test]
fn live_adapter_operation_budget_is_independent_of_launch_extent() {
    let mut context = Context::new();
    let fixture = fixture(&mut context, Mutation::None);
    let entry = fixture.function.get_entry_block(&context);
    for _ in 0..MAX_CONDITIONAL_PREFIX_OPERATIONS_V1 {
        let extra = IndexConstantOp::new(&mut context, 0);
        append(&context, entry, &extra);
    }
    assert_eq!(
        derive_pliron_conditional_prefix_coverage_v1(&context, &fixture.function).err(),
        Some(ConditionalPrefixDerivationErrorV1::Limit("operations"))
    );
}

fn fill_attributes(attributes: &mut AttributeDict, count: usize) {
    for index in attributes.0.len()..count {
        attributes.set(
            format!("prefix_padding_{index}").try_into().unwrap(),
            IndexValueAttr(0),
        );
    }
    assert_eq!(attributes.0.len(), count);
}

#[test]
fn live_attribute_count_boundary_covers_function_block_and_typed_operation() {
    use ConditionalPrefixDerivationErrorV1 as Error;
    for block_attributes in [false, true] {
        let mut context = Context::new();
        let fixture = fixture(&mut context, Mutation::None);
        let entry = fixture.function.get_entry_block(&context);
        let root = fixture.function.get_operation();
        for count in [
            MAX_CONDITIONAL_PREFIX_ATTRIBUTES_PER_ENTITY_V1,
            MAX_CONDITIONAL_PREFIX_ATTRIBUTES_PER_ENTITY_V1 + 1,
        ] {
            if block_attributes {
                fill_attributes(&mut entry.deref_mut(&context).attributes, count);
            } else {
                fill_attributes(&mut root.deref_mut(&context).attributes, count);
            }
            let result = derive_pliron_conditional_prefix_coverage_v1(&context, &fixture.function);
            if count == MAX_CONDITIONAL_PREFIX_ATTRIBUTES_PER_ENTITY_V1 {
                let record = result.unwrap();
                assert!(!record.proves_unconditional_total_view());
                assert!(!record.grants_launch_authority());
            } else {
                assert_eq!(result.err(), Some(Error::Limit("attributes per entity")));
            }
        }
    }

    let mut context = Context::new();
    let fixture = fixture(&mut context, Mutation::None);
    let value = fixture.value.get_operation();
    fill_attributes(
        &mut value.deref_mut(&context).attributes,
        MAX_CONDITIONAL_PREFIX_ATTRIBUTES_PER_ENTITY_V1,
    );
    // Passing the budget never excuses an invalid closed typed schema.
    assert_eq!(
        derive_pliron_conditional_prefix_coverage_v1(&context, &fixture.function).err(),
        Some(Error::Malformed("typed value")),
    );
    fill_attributes(
        &mut value.deref_mut(&context).attributes,
        MAX_CONDITIONAL_PREFIX_ATTRIBUTES_PER_ENTITY_V1 + 1,
    );
    assert_eq!(
        derive_pliron_conditional_prefix_coverage_v1(&context, &fixture.function).err(),
        Some(Error::Limit("attributes per entity")),
    );
}

#[test]
fn all_attribute_preflight_finishes_before_any_typed_verifier() {
    let mut context = Context::new();
    let fixture = fixture(&mut context, Mutation::TypedIntegerDivision);
    // The earlier integer symbol would fail domain admission, but the later
    // dictionary must hit its budget before semantic collection begins.
    fill_attributes(
        &mut fixture.value.get_operation().deref_mut(&context).attributes,
        MAX_CONDITIONAL_PREFIX_ATTRIBUTES_PER_ENTITY_V1 + 1,
    );
    assert_eq!(
        derive_pliron_conditional_prefix_coverage_v1(&context, &fixture.function).err(),
        Some(ConditionalPrefixDerivationErrorV1::Limit(
            "attributes per entity"
        )),
    );
}

#[test]
fn live_attribute_text_boundaries_are_checked_without_printing() {
    use ConditionalPrefixDerivationErrorV1 as Error;
    for payload in 0..3 {
        for bytes in [
            MAX_CONDITIONAL_PREFIX_ATTRIBUTE_TEXT_BYTES_V1,
            MAX_CONDITIONAL_PREFIX_ATTRIBUTE_TEXT_BYTES_V1 + 1,
        ] {
            let mut context = Context::new();
            let fixture = fixture(&mut context, Mutation::None);
            let root = fixture.function.get_operation();
            let mut raw = root.deref_mut(&context);
            match payload {
                0 => raw
                    .attributes
                    .set("a".repeat(bytes).try_into().unwrap(), IndexValueAttr(0)),
                1 => raw.attributes.set(
                    "prefix_text".try_into().unwrap(),
                    StringAttr::new("a".repeat(bytes)),
                ),
                2 => raw.attributes.set(
                    "prefix_identifier".try_into().unwrap(),
                    IdentifierAttr::new("a".repeat(bytes).try_into().unwrap()),
                ),
                _ => unreachable!(),
            }
            drop(raw);
            let result = derive_pliron_conditional_prefix_coverage_v1(&context, &fixture.function);
            if bytes == MAX_CONDITIONAL_PREFIX_ATTRIBUTE_TEXT_BYTES_V1 {
                let record = result.unwrap();
                assert!(!record.grants_launch_authority());
            } else {
                assert_eq!(
                    result.err(),
                    Some(Error::Limit(if payload == 0 {
                        "attribute key bytes"
                    } else {
                        "attribute text bytes"
                    }))
                );
            }
        }
    }
}

#[test]
fn live_attribute_work_budget_is_shared_across_entity_dictionaries() {
    let mut context = Context::new();
    let fixture = fixture(&mut context, Mutation::None);
    let root = fixture.function.get_operation();
    let entry = fixture.function.get_entry_block(&context);
    for index in 0..8 {
        let key: pliron::identifier::Identifier =
            format!("prefix_text_{index}").try_into().unwrap();
        let text = StringAttr::new("a".repeat(MAX_CONDITIONAL_PREFIX_ATTRIBUTE_TEXT_BYTES_V1));
        root.deref_mut(&context)
            .attributes
            .set(key.clone(), text.clone());
        entry.deref_mut(&context).attributes.set(key, text);
    }
    assert_eq!(
        derive_pliron_conditional_prefix_coverage_v1(&context, &fixture.function).err(),
        Some(ConditionalPrefixDerivationErrorV1::Limit("work")),
    );
}

#[test]
fn attribute_preflight_and_cfg_derivation_share_the_same_work_budget() {
    let model = model();
    let mut proof_work = 0;
    derive_model_with_work(&model, &mut proof_work).unwrap();
    assert!(proof_work > 0);
    let mut exact = MAX_CONDITIONAL_PREFIX_WORK_V1 - proof_work;
    derive_model_with_work(&model, &mut exact).unwrap();
    assert_eq!(exact, MAX_CONDITIONAL_PREFIX_WORK_V1);

    let mut attributes = AttributeDict::default();
    attributes.set("a".try_into().unwrap(), IndexValueAttr(0));
    let mut shared = MAX_CONDITIONAL_PREFIX_WORK_V1 - proof_work;
    preflight_attributes(&attributes, &mut shared).unwrap();
    assert_eq!(
        derive_model_with_work(&model, &mut shared).err(),
        Some(ConditionalPrefixDerivationErrorV1::Limit("work")),
    );
}
