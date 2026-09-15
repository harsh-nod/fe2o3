use super::*;
use dialect_kernel::{
    SEMANTIC_TYPED_READ_SYMBOL_BASE_V1, SemanticReadOrderingAttr, SemanticReadVolatilityAttr,
    SemanticTypedReadOp, SemanticTypedSelectOp,
};
use dialect_proof::{ProofIdAttr, RequireEffectRefinementOp};

pub(super) struct ProductionValues {
    pub(super) base: Fixture,
    reads: Vec<SemanticTypedReadOp>,
    metadata: RequireEffectRefinementOp,
    coordinate: SemanticTypedExpressionRootOp,
    predicate: SemanticTypedExpressionRootOp,
}

fn entry_root(
    context: &mut Context,
    before: Ptr<Operation>,
    kind: SemanticScalarKindAttr,
    bits: u16,
) -> SemanticTypedExpressionRootOp {
    let scalar = SemanticTypedScalarV1::new(kind, bits).unwrap();
    let (value, expression) = if scalar.is_bool() {
        let constant = SemanticTypedConstantOp::new(context, 1, scalar);
        constant.get_operation().insert_before(context, before);
        (
            constant.result(context),
            SemanticTypedExpressionV1::Constant { scalar, bits: 1 },
        )
    } else {
        let symbol = SemanticTypedSymbolOp::new(context, 0, scalar);
        symbol.get_operation().insert_before(context, before);
        (
            symbol.result(context),
            SemanticTypedExpressionV1::Symbol { scalar, symbol: 0 },
        )
    };
    let contract = SemanticNumericalContractV1 {
        policy: SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence,
        rounding: SemanticIeeeRoundingAttr::NearestTiesToEven,
        exceptional_values: SemanticExceptionalValueAttr::PreserveExactBits,
    };
    let digest = expression.canonical_transcript_sha256(contract);
    let mut commitment = [0; 4];
    for (word, bytes) in commitment.iter_mut().zip(digest.chunks_exact(8)) {
        *word = u64::from_le_bytes(bytes.try_into().unwrap());
    }
    let root = SemanticTypedExpressionRootOp::new(
        context,
        value,
        contract.policy,
        contract.rounding,
        contract.exceptional_values,
        commitment,
    );
    root.get_operation().insert_before(context, before);
    root
}

pub(super) fn production_values(context: &mut Context) -> ProductionValues {
    let base = fixture(context, Mutation::None);
    dialect_proof::register_dialect(context).unwrap();
    let predicate = entry_root(
        context,
        base.marker.get_operation(),
        SemanticScalarKindAttr::Bool,
        1,
    );
    let coordinate = entry_root(
        context,
        base.marker.get_operation(),
        SemanticScalarKindAttr::UnsignedInteger,
        64,
    );
    let operations = base
        .function
        .get_region(context)
        .deref(context)
        .iter(context)
        .flat_map(|block| block.deref(context).iter(context).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let scalar = SemanticTypedScalarV1::new(SemanticScalarKindAttr::Float, 32).unwrap();
    let mut reads = Vec::new();
    let mut write = None;
    for pointer in operations {
        let op = Operation::get_op_dyn(pointer, context);
        let Some(access) = op.downcast_ref::<RankedAccessOp>() else {
            continue;
        };
        if access.kind(context) == Some(AccessKindAttr::Write) {
            write = Some(pointer);
            continue;
        }
        let read = SemanticTypedReadOp::new(
            context,
            SEMANTIC_TYPED_READ_SYMBOL_BASE_V1 + reads.len() as u32,
            scalar,
            MemorySpaceAttr::Global,
            SemanticReadVolatilityAttr::NonVolatile,
            SemanticReadOrderingAttr::Unordered,
            access.view(context),
            access.indices(context),
            None,
        )
        .unwrap();
        read.get_operation().insert_after(context, pointer);
        reads.push(read);
    }
    assert_eq!(reads.len(), 2);
    for (slot, read) in reads.iter().enumerate() {
        let fallback = SemanticTypedConstantOp::new(context, 0, scalar);
        fallback
            .get_operation()
            .insert_before(context, base.value.get_operation());
        let select = SemanticTypedSelectOp::new(
            context,
            scalar,
            predicate.result(context),
            read.result(context),
            fallback.result(context),
        );
        select
            .get_operation()
            .insert_before(context, base.value.get_operation());
        Operation::replace_operand(
            base.value.get_operation(),
            context,
            slot,
            select.result(context),
        );
    }
    let write = write.unwrap();
    Operation::insert_operand(write, context, 2, base.value.result(context));
    let view = write.deref(context).get_operand(0);
    let index = write.deref(context).get_operand(1);
    let metadata = RequireEffectRefinementOp::new(
        context,
        ProofIdAttr::new([1, 2, 3, 4]),
        view,
        vec![index],
        vec![coordinate.result(context)],
        vec![coordinate.result(context)],
        predicate.result(context),
        predicate.result(context),
        predicate.result(context),
        predicate.result(context),
        base.value.result(context),
        base.value.result(context),
    );
    metadata.get_operation().insert_after(context, write);
    ProductionValues {
        base,
        reads,
        metadata,
        coordinate,
        predicate,
    }
}

#[test]
fn paired_cross_block_reads_and_ten_operand_metadata_preserve_conditional_coverage() {
    let mut context = Context::new();
    let f = production_values(&mut context);
    let model = collect_model(&context, &f.base.function, &mut 0).unwrap();
    assert_eq!(
        model.blocks.iter().map(|b| b.accesses.len()).sum::<usize>(),
        3
    );
    let record = derive_pliron_conditional_prefix_coverage_v1(&context, &f.base.function).unwrap();
    assert_eq!(record.conditions().len(), 3);
    assert_eq!(record.source_guard_dnf().len(), 1);
    assert_eq!(record.source_guard_dnf()[0].len(), 3);
    assert!(!record.grants_launch_authority());
    assert!(!record.proves_unconditional_total_view());
    let report = crate::run_pliron_hierarchical_ownership_check_v1(&context, &f.base.function);
    assert!(report.conditional_prefix_coverage().is_some());
    assert_eq!(report.coverage_summary().total_view_proved(), 0);
}

#[test]
fn paired_reads_reject_view_index_identity_ordering_and_adjacency_mutations() {
    for mutation in 0..6 {
        let mut context = Context::new();
        let f = production_values(&mut context);
        let read = &f.reads[0];
        match mutation {
            0 => Operation::replace_operand(
                read.get_operation(),
                &context,
                0,
                f.reads[1].view(&context),
            ),
            1 => Operation::replace_operand(
                read.get_operation(),
                &context,
                1,
                f.base.marker.result(&context),
            ),
            2 => read.set_attr_kernel_semantic_read_ordering(
                &context,
                SemanticReadOrderingAttr::Acquire,
            ),
            3 => f.reads[1].set_attr_kernel_semantic_read_id(
                &context,
                dialect_kernel::SemanticSymbolAttr(SEMANTIC_TYPED_READ_SYMBOL_BASE_V1),
            ),
            4 => {
                let scalar = read.scalar(&context).unwrap();
                let constant = SemanticTypedConstantOp::new(&mut context, 0, scalar);
                constant
                    .get_operation()
                    .insert_before(&context, read.get_operation());
            }
            5 => {
                let scalar = read.scalar(&context).unwrap();
                let symbol = SemanticTypedSymbolOp::new(
                    &mut context,
                    SEMANTIC_TYPED_READ_SYMBOL_BASE_V1,
                    scalar,
                );
                symbol
                    .get_operation()
                    .insert_before(&context, read.get_operation());
            }
            _ => unreachable!(),
        }
        assert!(
            derive_pliron_conditional_prefix_coverage_v1(&context, &f.base.function).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn native_ssa_rejects_a_value_that_does_not_dominate_its_use() {
    let mut context = Context::new();
    let f = production_values(&mut context);
    let exit = f
        .base
        .function
        .get_region(&context)
        .deref(&context)
        .iter(&context)
        .last()
        .unwrap();
    let value = f.base.value.result(&context);
    let root = SemanticTypedExpressionRootOp::new(
        &mut context,
        value,
        SemanticNumericalPolicyAttr::ExactIeeeNearestTiesToEvenPreserveBits,
        SemanticIeeeRoundingAttr::NearestTiesToEven,
        SemanticExceptionalValueAttr::PreserveExactBits,
        [1, 2, 3, 4],
    );
    root.get_operation().insert_at_front(exit, &context);
    // The write block is visited first, but early exits skip its definition.
    collect_model(&context, &f.base.function, &mut 0).unwrap();
    assert_eq!(
        derive_pliron_conditional_prefix_coverage_v1(&context, &f.base.function),
        Err(ConditionalPrefixDerivationErrorV1::StructuralIdentityUnavailable)
    );
}

#[test]
fn malformed_metadata_and_wrong_bitvector_root_policy_reject() {
    for mutation in 0..4 {
        let mut context = Context::new();
        let f = production_values(&mut context);
        match mutation {
            0 => {
                Operation::remove_operand(f.metadata.get_operation(), &context, 9);
            }
            1 => {
                Operation::insert_operand(
                    f.metadata.get_operation(),
                    &context,
                    10,
                    f.predicate.result(&context),
                );
            }
            2 => {
                Operation::replace_operand(
                    f.metadata.get_operation(),
                    &context,
                    1,
                    f.coordinate.result(&context),
                );
            }
            3 => f.predicate.set_attr_kernel_semantic_numerical_policy(
                &context,
                SemanticNumericalPolicyAttr::ExactIeeeNearestTiesToEvenPreserveBits,
            ),
            _ => unreachable!(),
        }
        assert!(
            derive_pliron_conditional_prefix_coverage_v1(&context, &f.base.function).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn stored_read_rhs_mutation_invalidates_the_exact_record() {
    let mut context = Context::new();
    let f = production_values(&mut context);
    let before = derive_pliron_conditional_prefix_coverage_v1(&context, &f.base.function).unwrap();
    Operation::replace_operand(
        f.base.value.get_operation(),
        &context,
        1,
        f.reads[0].result(&context),
    );
    let after = derive_pliron_conditional_prefix_coverage_v1(&context, &f.base.function).unwrap();
    assert_eq!(before.conditions(), after.conditions());
    assert_ne!(before.canonical_bytes(), after.canonical_bytes());
    assert_eq!(
        revalidate_pliron_conditional_prefix_coverage_v1(&context, &f.base.function, &before),
        Err(ConditionalPrefixDerivationErrorV1::StaleGraph)
    );
    assert!(!after.grants_launch_authority());
}

#[test]
fn dominating_read_in_a_physically_later_block_is_supported() {
    let mut context = Context::new();
    let f = production_values(&mut context);
    let block = f.reads[0]
        .get_operation()
        .deref(&context)
        .get_parent_block()
        .unwrap();
    block.unlink(&context);
    block.insert_at_back(f.base.function.get_region(&context), &context);
    let report = derive_pliron_conditional_prefix_coverage_v1(&context, &f.base.function).unwrap();
    assert_eq!(report.conditions().len(), 3);
    assert!(!report.grants_launch_authority());
}

#[test]
fn foreign_reverse_users_and_predecessors_reject_before_native_verification() {
    for predecessor in [false, true] {
        let mut context = Context::new();
        let f = production_values(&mut context);
        if predecessor {
            let exit = f
                .base
                .function
                .get_region(&context)
                .deref(&context)
                .iter(&context)
                .last()
                .unwrap();
            let _foreign = BranchOp::new(&mut context, exit);
        } else {
            let value = f.reads[0].result(&context);
            let scalar = f.reads[0].scalar(&context).unwrap();
            let _foreign = SemanticTypedUnaryOp::new(
                &mut context,
                SemanticTypedUnaryKindAttr::Negate,
                scalar,
                value,
            );
        }
        assert_eq!(
            derive_pliron_conditional_prefix_coverage_v1(&context, &f.base.function),
            Err(ConditionalPrefixDerivationErrorV1::Malformed(
                "SSA custody or order"
            ))
        );
    }
}

#[test]
fn native_preflight_and_value_recognition_share_the_work_budget() {
    let mut context = Context::new();
    let f = production_values(&mut context);
    let mut used = 0;
    collect_model(&context, &f.base.function, &mut used).unwrap();
    assert!(used > 0 && used < MAX_CONDITIONAL_PREFIX_WORK_V1);
    let mut exact = MAX_CONDITIONAL_PREFIX_WORK_V1 - used;
    collect_model(&context, &f.base.function, &mut exact).unwrap();
    assert_eq!(exact, MAX_CONDITIONAL_PREFIX_WORK_V1);
    let mut short = MAX_CONDITIONAL_PREFIX_WORK_V1 - used + 1;
    assert!(matches!(
        collect_model(&context, &f.base.function, &mut short),
        Err(ConditionalPrefixDerivationErrorV1::Limit("work"))
    ));
}

#[test]
fn unsigned_semantic_values_never_become_invocation_coordinates() {
    let mut context = Context::new();
    let f = production_values(&mut context);
    let entry = f.base.function.get_entry_block(&context);
    let branch = entry.deref(&context).iter(&context).last().unwrap();
    Operation::replace_operand(branch, &context, 0, f.coordinate.result(&context));
    assert!(derive_pliron_conditional_prefix_coverage_v1(&context, &f.base.function).is_err());
}

#[test]
fn unsigned_partial_operations_reject_even_with_wrapping_overflow() {
    use SemanticTypedBinaryKindAttr as Kind;
    for kind in [
        Kind::Add,
        Kind::Subtract,
        Kind::Multiply,
        Kind::BitAnd,
        Kind::BitOr,
        Kind::BitXor,
        Kind::Divide,
        Kind::Remainder,
        Kind::ShiftLeft,
        Kind::ShiftRight,
    ] {
        let mut context = Context::new();
        let f = production_values(&mut context);
        let value = f.coordinate.result(&context);
        let scalar =
            SemanticTypedScalarV1::new(SemanticScalarKindAttr::UnsignedInteger, 64).unwrap();
        let binary = SemanticTypedBinaryOp::new(
            &mut context,
            kind,
            SemanticOverflowAttr::Wrapping,
            scalar,
            value,
            value,
        );
        binary
            .get_operation()
            .insert_before(&context, f.metadata.get_operation());
        let partial = matches!(
            kind,
            Kind::Divide | Kind::Remainder | Kind::ShiftLeft | Kind::ShiftRight
        );
        let result = derive_pliron_conditional_prefix_coverage_v1(&context, &f.base.function);
        assert_eq!(result.is_err(), partial, "{kind:?}: {result:?}");
    }
}

#[test]
fn value_selection_requires_bool_condition_and_matching_arms() {
    for wrong_condition in [false, true] {
        let mut context = Context::new();
        let f = production_values(&mut context);
        let select = f
            .base
            .value
            .get_operation()
            .deref(&context)
            .get_operand(0)
            .defining_op()
            .unwrap();
        Operation::replace_operand(
            select,
            &context,
            if wrong_condition { 0 } else { 2 },
            if wrong_condition {
                f.coordinate.result(&context)
            } else {
                f.predicate.result(&context)
            },
        );
        assert!(derive_pliron_conditional_prefix_coverage_v1(&context, &f.base.function).is_err());
    }
}

#[test]
fn volatile_read_events_preserve_coverage_but_not_memory_equivalence() {
    let mut context = Context::new();
    let f = production_values(&mut context);
    let ordinary =
        derive_pliron_conditional_prefix_coverage_v1(&context, &f.base.function).unwrap();
    for read in &f.reads {
        read.set_attr_kernel_semantic_read_volatility(
            &context,
            SemanticReadVolatilityAttr::Volatile,
        );
    }
    let volatile =
        derive_pliron_conditional_prefix_coverage_v1(&context, &f.base.function).unwrap();
    assert_eq!(ordinary.conditions(), volatile.conditions());
    assert_ne!(ordinary.canonical_bytes(), volatile.canonical_bytes());
    assert!(!volatile.grants_launch_authority());
    assert!(!volatile.proves_unconditional_total_view());
    assert_eq!(
        collect_model(&context, &f.base.function, &mut 0)
            .unwrap()
            .blocks
            .iter()
            .map(|b| b.accesses.len())
            .sum::<usize>(),
        3
    );
}

#[test]
fn mixed_volatile_inputs_reject_memory_proofs_before_dynamic_trace() {
    use crate::pliron_semantic_memory_v1::{
        LivePlironInitialReadInputsV1, PlironSemanticMemoryErrorV1, PlironSemanticMemorySiteV1,
        prove_live_pliron_semantic_memory_v1,
    };
    for mask in 1..4 {
        let mut context = Context::new();
        fe2o3_pliron_owner_core::ensure_context_identity(&mut context).unwrap();
        let f = production_values(&mut context);
        let before =
            derive_pliron_conditional_prefix_coverage_v1(&context, &f.base.function).unwrap();
        for (i, read) in f.reads.iter().enumerate() {
            if mask & (1 << i) != 0 {
                read.set_attr_kernel_semantic_read_volatility(
                    &context,
                    SemanticReadVolatilityAttr::Volatile,
                );
            }
        }
        let after =
            derive_pliron_conditional_prefix_coverage_v1(&context, &f.base.function).unwrap();
        assert_eq!(before.conditions(), after.conditions());
        assert_eq!(
            revalidate_pliron_conditional_prefix_coverage_v1(&context, &f.base.function, &before),
            Err(ConditionalPrefixDerivationErrorV1::StaleGraph)
        );
        let first = f
            .reads
            .iter()
            .find(|read| read.volatility(&context) == Some(SemanticReadVolatilityAttr::Volatile))
            .unwrap()
            .get_operation();
        let site = f
            .base
            .function
            .get_region(&context)
            .deref(&context)
            .iter(&context)
            .enumerate()
            .find_map(|(b, block)| {
                block
                    .deref(&context)
                    .iter(&context)
                    .position(|op| op == first)
                    .map(|op| PlironSemanticMemorySiteV1::new(b, op))
            })
            .unwrap();
        let expected = PlironSemanticMemoryErrorV1::UnsupportedRead { site };
        assert_eq!(
            prove_live_pliron_semantic_memory_v1(&context, &f.base.function).err(),
            Some(expected.clone())
        );
        assert_eq!(
            LivePlironInitialReadInputsV1::prove(&context, &f.base.function).err(),
            Some(expected)
        );
    }
}
