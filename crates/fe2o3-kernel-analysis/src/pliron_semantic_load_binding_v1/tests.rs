use super::*;
use dialect_kernel::{
    AllocationOriginAttr, IndexValueAttr, RankedViewType, SemanticOverflowAttr,
    SemanticScalarKindAttr, SemanticTypedBinaryKindAttr,
};
use fe2o3_kernel_ir::{BasicBlock as KirBlock, BlockId, Function, Module, Signature, Terminator};
use fe2o3_pliron_owner_core::ensure_context_identity;
use pliron::{
    builtin::{op_interfaces::OneRegionInterface, types::FunctionType},
    dialect::DialectName,
};

type E = PlironSemanticLoadBindingErrorV1;

fn f32_type() -> SemanticTypedScalarV1 {
    SemanticTypedScalarV1::new(SemanticScalarKindAttr::Float, 32).unwrap()
}

fn canonical(name: &str) -> VerifiedCanonicalKernelIrV13 {
    let mut module = Module::new(name);
    let mut block = KirBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::definition(
        "subject",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    VerifiedCanonicalKernelIrV13::from_module(module).unwrap()
}

fn setup() -> Context {
    let mut context = Context::new();
    dialect_kernel::register_dialect(
        &mut context,
        &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    ensure_context_identity(&mut context).unwrap();
    context
}

fn append(context: &Context, function: &FuncOp, op: &impl Op) {
    op.get_operation()
        .insert_at_back(function.get_entry_block(context), context);
}

struct Fixture {
    function: FuncOp,
    view: RankedViewOp,
    index: IndexConstantOp,
    read: SemanticTypedReadOp,
    guard: SemanticTypedConstantOp,
    fallback: SemanticTypedConstantOp,
    sum: Option<SemanticTypedBinaryOp>,
}

fn fixture(
    context: &mut Context,
    name: &str,
    writable: bool,
    guarded: bool,
    consumed: bool,
) -> Fixture {
    let function = FuncOp::new(
        context,
        name.try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    );
    let view_type = RankedViewType::new(context, 32, writable, vec![8]).unwrap();
    let view = RankedViewOp::new_in_space_with_allocation_contract(
        context,
        view_type,
        vec![],
        MemorySpaceAttr::Global,
        1,
        0,
    )
    .unwrap();
    append(context, &function, &view);
    let index = IndexConstantOp::new(context, 2);
    append(context, &function, &index);
    let guard = SemanticTypedConstantOp::new(
        context,
        1,
        SemanticTypedScalarV1::new(SemanticScalarKindAttr::Bool, 1).unwrap(),
    );
    append(context, &function, &guard);
    let fallback = SemanticTypedConstantOp::new(context, 0, f32_type());
    append(context, &function, &fallback);
    let view_value = view.result(context);
    let index_value = index.result(context);
    let guarded = guarded.then(|| (guard.result(context), fallback.result(context)));
    let read = SemanticTypedReadOp::new(
        context,
        SEMANTIC_TYPED_READ_SYMBOL_BASE_V1,
        f32_type(),
        MemorySpaceAttr::Global,
        SemanticReadVolatilityAttr::NonVolatile,
        SemanticReadOrderingAttr::Unordered,
        view_value,
        vec![index_value],
        guarded,
    )
    .unwrap();
    append(context, &function, &read);
    let sum = consumed.then(|| {
        let value = read.result(context);
        let sum = SemanticTypedBinaryOp::new(
            context,
            SemanticTypedBinaryKindAttr::Add,
            SemanticOverflowAttr::Wrapping,
            f32_type(),
            value,
            value,
        );
        append(context, &function, &sum);
        sum
    });
    let ret = ReturnOp::new(context);
    append(context, &function, &ret);
    Fixture {
        function,
        view,
        index,
        read,
        guard,
        fallback,
        sum,
    }
}

fn bind(
    context: &Context,
    f: &Fixture,
    canonical: &VerifiedCanonicalKernelIrV13,
) -> Result<LivePlironSemanticLoadBindingsV1, E> {
    validate_live_pliron_semantic_load_bindings_v1(context, &f.function, canonical, 7)
}

#[test]
fn binds_actual_read_operands_without_equating_it_to_a_symbol() {
    for guarded in [false, true] {
        let mut context = setup();
        let f = fixture(&mut context, "read", false, guarded, true);
        let canonical = canonical("subject");
        let binding = bind(&context, &f, &canonical).unwrap();
        binding
            .with_live_reads(&context, &f.function, &canonical, 7, |reads| {
                assert_eq!(reads.len(), 1);
                let read = &reads[0];
                assert_eq!((read.block(), read.operation()), (0, 4));
                assert_eq!(read.producer(), f.read.get_operation());
                assert_eq!(read.result(), f.read.result(&context));
                assert_eq!(read.symbol(), SEMANTIC_TYPED_READ_SYMBOL_BASE_V1);
                assert_eq!(read.view(), f.view.result(&context));
                assert_eq!(read.indices(), &[f.index.result(&context)]);
                assert_eq!(read.allocation_origin(), 1);
                assert_eq!(read.scalar(), f32_type());
                assert_eq!(read.kind(), AccessKindAttr::Read);
                assert_eq!(read.memory_space(), MemorySpaceAttr::Global);
                assert_eq!(read.volatility(), SemanticReadVolatilityAttr::NonVolatile);
                assert_eq!(read.ordering(), SemanticReadOrderingAttr::Unordered);
                assert_eq!(
                    read.guarded(),
                    guarded.then(|| (f.guard.result(&context), f.fallback.result(&context)))
                );
            })
            .unwrap();
    }
}

#[test]
fn canonical_epoch_context_and_function_substitution_reject() {
    let mut context = setup();
    let f = fixture(&mut context, "first", false, false, true);
    let other = fixture(&mut context, "second", false, false, true);
    let canonical = canonical("first");
    let changed = self::canonical("second");
    let binding = bind(&context, &f, &canonical).unwrap();
    assert_eq!(
        binding.revalidate(&context, &f.function, &changed, 7),
        Err(E::CanonicalSubjectChanged)
    );
    assert_eq!(
        binding.revalidate(&context, &f.function, &canonical, 8),
        Err(E::CanonicalSubjectChanged)
    );
    assert_eq!(
        binding.revalidate(&context, &other.function, &canonical, 7),
        Err(E::FunctionChanged)
    );
    let mut other_context = setup();
    let other = fixture(&mut other_context, "first", false, false, true);
    assert_eq!(
        binding.revalidate(&other_context, &other.function, &canonical, 7),
        Err(E::ContextChanged)
    );
}

#[test]
fn every_mutation_attempt_invalidates_even_when_value_is_restored() {
    for case in 0..5 {
        let mut context = setup();
        let f = fixture(&mut context, "read", false, true, true);
        let canonical = canonical("subject");
        let binding = bind(&context, &f, &canonical).unwrap();
        match case {
            0 => drop(f.read.get_operation().deref_mut(&context)),
            1 => {
                f.read.set_attr_kernel_semantic_read_volatility(
                    &mut context,
                    SemanticReadVolatilityAttr::Volatile,
                );
                f.read.set_attr_kernel_semantic_read_volatility(
                    &mut context,
                    SemanticReadVolatilityAttr::NonVolatile,
                );
            }
            2 => {
                f.view
                    .set_attr_kernel_allocation_origin(&mut context, AllocationOriginAttr(2));
                f.view
                    .set_attr_kernel_allocation_origin(&mut context, AllocationOriginAttr(1));
            }
            3 => f
                .index
                .set_attr_kernel_index_value(&mut context, IndexValueAttr(2)),
            _ => drop(f.fallback.get_operation().deref_mut(&context)),
        }
        assert_eq!(
            binding.revalidate(&context, &f.function, &canonical, 7),
            Err(E::MutationEpochChanged)
        );
    }
}

#[test]
fn observation_callback_cannot_return_a_live_binding_after_mutation() {
    let mut context = setup();
    let f = fixture(&mut context, "read", false, false, true);
    let canonical = canonical("subject");
    let binding = bind(&context, &f, &canonical).unwrap();
    assert_eq!(
        binding.with_live_reads(&context, &f.function, &canonical, 7, |_| {
            drop(f.read.get_operation().deref_mut(&context));
        }),
        Err(E::MutationEpochChanged)
    );
}

#[test]
fn volatile_writable_unknown_origin_and_unused_reads_reject() {
    for case in 0..4 {
        let mut context = setup();
        let f = fixture(&mut context, "read", case == 1, false, case != 3);
        let expected = match case {
            0 => {
                f.read.set_attr_kernel_semantic_read_volatility(
                    &mut context,
                    SemanticReadVolatilityAttr::Volatile,
                );
                E::UnsupportedVolatilityOrOrdering
            }
            1 => E::UnknownOrWritableAllocation,
            2 => {
                f.view
                    .set_attr_kernel_allocation_origin(&mut context, AllocationOriginAttr(0));
                E::UnknownOrWritableAllocation
            }
            _ => E::UnconsumedRead,
        };
        assert_eq!(
            bind(&context, &f, &canonical("subject")).err(),
            Some(expected)
        );
    }
}

#[test]
fn zero_operand_load_symbol_cannot_stand_in_for_a_read() {
    let mut context = setup();
    let f = fixture(&mut context, "read", false, false, true);
    let symbol =
        SemanticTypedSymbolOp::new(&mut context, SEMANTIC_TYPED_READ_SYMBOL_BASE_V1, f32_type());
    symbol
        .get_operation()
        .insert_before(&context, f.read.get_operation());
    assert_eq!(
        bind(&context, &f, &canonical("subject")).err(),
        Some(E::UnboundLoadSymbol)
    );
}

#[test]
fn duplicate_load_labels_do_not_merge_two_memory_observations() {
    let mut context = setup();
    let f = fixture(&mut context, "read", false, false, true);
    let view = f.view.result(&context);
    let index = f.index.result(&context);
    let duplicate = SemanticTypedReadOp::new(
        &mut context,
        SEMANTIC_TYPED_READ_SYMBOL_BASE_V1,
        f32_type(),
        MemorySpaceAttr::Global,
        SemanticReadVolatilityAttr::NonVolatile,
        SemanticReadOrderingAttr::Unordered,
        view,
        vec![index],
        None,
    )
    .unwrap();
    duplicate
        .get_operation()
        .insert_before(&context, f.sum.as_ref().unwrap().get_operation());
    assert_eq!(
        bind(&context, &f, &canonical("subject")).err(),
        Some(E::DuplicateLoadIdentity)
    );
}

#[test]
fn non_dominating_guard_or_index_is_not_a_binding() {
    for guard in [false, true] {
        let mut context = setup();
        let f = fixture(&mut context, "read", false, true, true);
        let producer = if guard {
            f.guard.get_operation()
        } else {
            f.index.get_operation()
        };
        producer.unlink(&context);
        producer.insert_before(&context, f.sum.as_ref().unwrap().get_operation());
        assert!(bind(&context, &f, &canonical("subject")).is_err());
    }
}

#[test]
fn cross_function_operands_and_multiple_blocks_reject() {
    let mut context = setup();
    let f = fixture(&mut context, "read", false, false, true);
    let second = BasicBlock::new(&mut context, Some("second".try_into().unwrap()), vec![]);
    second.insert_at_back(f.function.get_region(&context), &context);
    let ret = ReturnOp::new(&mut context);
    ret.get_operation().insert_at_back(second, &context);
    assert!(bind(&context, &f, &canonical("subject")).is_err());

    let mut context = setup();
    let f = fixture(&mut context, "read", false, false, true);
    let other = fixture(&mut context, "other", false, false, true);
    f.index.get_operation().unlink(&context);
    f.index
        .get_operation()
        .insert_before(&context, other.read.get_operation());
    assert!(bind(&context, &f, &canonical("subject")).is_err());
}

#[test]
fn typed_read_never_disappears_or_becomes_a_free_expression_variable() {
    for consumed in [false, true] {
        let mut context = setup();
        let f = fixture(&mut context, "read", false, false, consumed);
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &f.function).unwrap();
        assert!(matches!(crate::pliron_semantic_refinement::SemanticExpressionTableV1::from_inventory(&context, &f.function, &inventory),
            Err(crate::pliron_semantic_refinement::SemanticExpressionBuildErrorV1::InvalidTypedExpression(_))));
        assert!(!crate::run_pliron_semantic_refinement_check_v1(&context, &f.function).is_clean());
        assert!(!crate::run_pliron_ranked_bounds_check_v1(&context, &f.function).is_clean());
    }
}

#[test]
fn reserved_symbol_alone_is_not_memory_evidence_but_scalar_parameters_remain_supported() {
    for reserved in [false, true] {
        let mut context = setup();
        let function_type = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "symbol_only".try_into().unwrap(),
            function_type,
        );
        let symbol = SemanticTypedSymbolOp::new(
            &mut context,
            if reserved {
                SEMANTIC_TYPED_READ_SYMBOL_BASE_V1
            } else {
                0
            },
            f32_type(),
        );
        append(&context, &function, &symbol);
        let ret = ReturnOp::new(&mut context);
        append(&context, &function, &ret);
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        let table = crate::pliron_semantic_refinement::SemanticExpressionTableV1::from_inventory(
            &context, &function, &inventory,
        );
        assert_eq!(table.is_ok(), !reserved);
        assert_eq!(
            validate_live_pliron_semantic_load_bindings_v1(
                &context,
                &function,
                &canonical("subject"),
                7
            )
            .err(),
            Some(if reserved {
                E::UnboundLoadSymbol
            } else {
                E::EmptyReadRoster
            })
        );
    }
}

#[test]
fn every_represented_read_component_changes_the_structural_identity() {
    for case in 0..7 {
        let mut context = setup();
        let f = fixture(&mut context, "read", false, true, true);
        let before = derive_pliron_ir_structural_identity_v1(&context, &f.function).unwrap();
        match case {
            0 => f.read.set_attr_kernel_semantic_read_id(
                &mut context,
                dialect_kernel::SemanticSymbolAttr(SEMANTIC_TYPED_READ_SYMBOL_BASE_V1 + 1),
            ),
            1 => f
                .view
                .set_attr_kernel_allocation_origin(&mut context, AllocationOriginAttr(2)),
            2 => f
                .index
                .set_attr_kernel_index_value(&mut context, IndexValueAttr(3)),
            3 => f.guard.set_attr_kernel_semantic_typed_constant_bits(
                &mut context,
                dialect_kernel::SemanticConstantAttr(0),
            ),
            4 => f.fallback.set_attr_kernel_semantic_typed_constant_bits(
                &mut context,
                dialect_kernel::SemanticConstantAttr(0x8000_0000),
            ),
            5 => f.read.set_attr_kernel_semantic_read_volatility(
                &mut context,
                SemanticReadVolatilityAttr::Volatile,
            ),
            _ => {
                f.view
                    .set_attr_kernel_memory_space(&mut context, MemorySpaceAttr::Workgroup);
                f.read
                    .set_attr_kernel_semantic_read_space(&mut context, MemorySpaceAttr::Workgroup);
            }
        }
        let after = derive_pliron_ir_structural_identity_v1(&context, &f.function).unwrap();
        assert!(!before.exactly_matches(&after), "substitution {case}");
    }
}

#[test]
fn unsupported_ordering_and_forged_guard_type_fail_live_validation() {
    for ordering in [false, true] {
        let mut context = setup();
        let f = fixture(&mut context, "read", false, true, true);
        if ordering {
            f.read.set_attr_kernel_semantic_read_ordering(
                &mut context,
                SemanticReadOrderingAttr::Acquire,
            );
        } else {
            f.guard.set_attr_kernel_semantic_typed_constant_scalar_kind(
                &mut context,
                SemanticScalarKindAttr::Float,
            );
            f.guard.set_attr_kernel_semantic_typed_constant_bit_width(
                &mut context,
                dialect_kernel::DimensionAttr(32),
            );
        }
        assert!(bind(&context, &f, &canonical("subject")).is_err());
    }
}

#[test]
fn read_against_itself_is_not_a_clean_refinement() {
    let mut context = setup();
    let f = fixture(&mut context, "read", false, true, true);
    bind(&context, &f, &canonical("subject")).unwrap();
    let result = f.read.result(&context);
    let equality = dialect_kernel::RequireEquivalentOp::new(&mut context, result, result);
    equality
        .get_operation()
        .insert_before(&context, f.sum.as_ref().unwrap().get_operation());
    let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &f.function).unwrap();
    assert!(
        crate::pliron_semantic_refinement::SemanticExpressionTableV1::from_inventory(
            &context, &f.function, &inventory
        )
        .is_err()
    );
    assert!(!crate::run_pliron_semantic_refinement_check_v1(&context, &f.function).is_clean());
}

#[test]
fn replacing_each_read_operand_breaks_both_identity_and_live_binding() {
    for slot in 0..4 {
        let mut context = setup();
        let f = fixture(&mut context, "read", false, true, true);
        let ty = RankedViewType::new(&context, 32, false, vec![8]).unwrap();
        let view = RankedViewOp::new_in_space_with_allocation_contract(
            &mut context,
            ty,
            vec![],
            MemorySpaceAttr::Global,
            2,
            0,
        )
        .unwrap();
        let index = IndexConstantOp::new(&mut context, 3);
        let guard = SemanticTypedConstantOp::new(
            &mut context,
            0,
            SemanticTypedScalarV1::new(SemanticScalarKindAttr::Bool, 1).unwrap(),
        );
        let fallback = SemanticTypedConstantOp::new(&mut context, 0x8000_0000, f32_type());
        for operation in [
            view.get_operation(),
            index.get_operation(),
            guard.get_operation(),
            fallback.get_operation(),
        ] {
            operation.insert_before(&context, f.read.get_operation());
        }
        let replacement = [
            view.result(&context),
            index.result(&context),
            guard.result(&context),
            fallback.result(&context),
        ][slot];
        let canonical = canonical("subject");
        let binding = bind(&context, &f, &canonical).unwrap();
        let before = derive_pliron_ir_structural_identity_v1(&context, &f.function).unwrap();
        Operation::replace_operand(f.read.get_operation(), &context, slot, replacement);
        let after = derive_pliron_ir_structural_identity_v1(&context, &f.function).unwrap();
        assert!(!before.exactly_matches(&after));
        assert_eq!(
            binding.revalidate(&context, &f.function, &canonical, 7),
            Err(E::MutationEpochChanged)
        );
    }
}

#[test]
fn same_width_integer_is_not_the_same_observation_type_as_f32() {
    let mut context = setup();
    let f = fixture(&mut context, "read", false, true, true);
    let before = derive_pliron_ir_structural_identity_v1(&context, &f.function).unwrap();
    f.read.set_attr_kernel_semantic_read_scalar_kind(
        &mut context,
        SemanticScalarKindAttr::SignedInteger,
    );
    f.fallback
        .set_attr_kernel_semantic_typed_constant_scalar_kind(
            &mut context,
            SemanticScalarKindAttr::SignedInteger,
        );
    f.sum
        .as_ref()
        .unwrap()
        .set_attr_kernel_semantic_typed_binary_scalar_kind(
            &mut context,
            SemanticScalarKindAttr::SignedInteger,
        );
    let after = derive_pliron_ir_structural_identity_v1(&context, &f.function).unwrap();
    assert!(!before.exactly_matches(&after));
    let canonical = canonical("subject");
    bind(&context, &f, &canonical)
        .unwrap()
        .with_live_reads(&context, &f.function, &canonical, 7, |reads| {
            assert_eq!(
                reads[0].scalar(),
                SemanticTypedScalarV1::new(SemanticScalarKindAttr::SignedInteger, 32).unwrap()
            );
        })
        .unwrap();
}

#[test]
fn readonly_binding_cannot_ignore_an_additional_write_effect() {
    let mut context = setup();
    let f = fixture(&mut context, "read", false, false, true);
    let ty = RankedViewType::new(&context, 32, true, vec![8]).unwrap();
    let output = RankedViewOp::new_in_space_with_allocation_contract(
        &mut context,
        ty,
        vec![],
        MemorySpaceAttr::Global,
        2,
        0,
    )
    .unwrap();
    output
        .get_operation()
        .insert_before(&context, f.read.get_operation());
    let view = output.result(&context);
    let index = f.index.result(&context);
    let write =
        dialect_kernel::RankedAccessOp::new(&mut context, AccessKindAttr::Write, view, vec![index])
            .unwrap();
    write
        .get_operation()
        .insert_before(&context, f.read.get_operation());
    assert_eq!(
        bind(&context, &f, &canonical("subject")).err(),
        Some(E::UnsupportedOperation)
    );
}
