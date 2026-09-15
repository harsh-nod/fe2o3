//! Component memory proofs, not production output-refinement qualification.
use super::*;
use dialect_kernel::*;
use fe2o3_pliron_owner_core::ensure_context_identity;
use pliron::{builtin::types::FunctionType, dialect::DialectName};

type E = PlironSemanticMemoryErrorV1;

#[path = "pipeline_tests.rs"]
mod pipeline_tests;
#[path = "control_flow_tests.rs"]
mod control_flow_tests;

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

fn scalar() -> SemanticTypedScalarV1 {
    SemanticTypedScalarV1::new(SemanticScalarKindAttr::Float, 32).unwrap()
}

struct Fixture {
    function: FuncOp,
    view: RankedViewOp,
    index: InvocationIndexOp,
    access: RankedAccessOp,
    read: SemanticTypedReadOp,
    write: RankedAccessOp,
    sum: SemanticTypedBinaryOp,
    ret: ReturnOp,
}

fn fixture(context: &mut Context) -> Fixture {
    let function = FuncOp::new(
        context,
        "memory_component".try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    );
    let ty = RankedViewType::new(context, 32, true, vec![64]).unwrap();
    let view = RankedViewOp::new_in_space_with_allocation_contract(
        context,
        ty,
        vec![],
        MemorySpaceAttr::Global,
        1,
        1,
    )
    .unwrap();
    append(context, &function, &view);
    let index = InvocationIndexOp::new(context, 0, 64);
    append(context, &function, &index);
    let view_value = view.result(context);
    let index_value = index.result(context);
    let access =
        RankedAccessOp::new(context, AccessKindAttr::Read, view_value, vec![index_value]).unwrap();
    append(context, &function, &access);
    let read = SemanticTypedReadOp::new(
        context,
        SEMANTIC_TYPED_READ_SYMBOL_BASE_V1,
        scalar(),
        MemorySpaceAttr::Global,
        SemanticReadVolatilityAttr::NonVolatile,
        SemanticReadOrderingAttr::Unordered,
        view_value,
        vec![index_value],
        None,
    )
    .unwrap();
    append(context, &function, &read);
    let one = SemanticTypedConstantOp::new(context, u64::from(1f32.to_bits()), scalar());
    append(context, &function, &one);
    let lhs = read.result(context);
    let rhs = one.result(context);
    let sum = SemanticTypedBinaryOp::new(
        context,
        SemanticTypedBinaryKindAttr::Add,
        SemanticOverflowAttr::Wrapping,
        scalar(),
        lhs,
        rhs,
    );
    append(context, &function, &sum);
    let write = RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        view_value,
        vec![index_value],
    )
    .unwrap();
    append(context, &function, &write);
    let ret = ReturnOp::new(context);
    append(context, &function, &ret);
    Fixture {
        function,
        view,
        index,
        access,
        read,
        write,
        sum,
        ret,
    }
}

#[test]
fn live_pointwise_rmw_proves_original_read_snapshot_without_a_kir_seal() {
    let mut context = setup();
    let f = fixture(&mut context);
    let proof = prove_live_pliron_semantic_memory_v1(&context, &f.function).unwrap();
    proof
        .with_live_reads(&context, &f.function, |reads| {
            assert_eq!(reads.len(), 1);
            let read = &reads[0];
            assert_eq!(read.access(), f.access.get_operation());
            assert_eq!(read.producer(), f.read.get_operation());
            assert_eq!(read.result(), f.read.result(&context));
            assert_eq!(read.allocation_origin(), 1);
            assert_eq!(read.indices(), [f.index.result(&context)]);
            assert_eq!(read.scalar(), scalar());
            assert!(read.reads_initial_memory());
            assert_eq!(read.instances().len(), 64);
            for (index, instance) in read.instances().iter().enumerate() {
                assert_eq!(instance.coordinates(), [index as u64]);
                assert_eq!(instance.indices(), [index as u64]);
                assert_eq!(instance.version(), PlironSemanticMemoryVersionV1::Initial);
            }
        })
        .unwrap();
}

#[test]
fn live_prior_write_cannot_claim_initial_memory() {
    let mut context = setup();
    let f = fixture(&mut context);
    f.write.get_operation().unlink(&context);
    f.write
        .get_operation()
        .insert_before(&context, f.access.get_operation());
    let proof = prove_live_pliron_semantic_memory_v1(&context, &f.function).unwrap();
    proof
        .with_live_reads(&context, &f.function, |reads| {
            assert!(!reads[0].reads_initial_memory());
            for (invocation, instance) in reads[0].instances().iter().enumerate() {
                assert_eq!(
                    instance.version(),
                    PlironSemanticMemoryVersionV1::AfterWrite {
                        invocation,
                        event: 0,
                        block: 0,
                        operation: 2,
                    }
                );
            }
        })
        .unwrap();
    assert!(matches!(LivePlironInitialReadInputsV1::prove(&context, &f.function),
        Err(E::NonInitialRead { site: PlironSemanticMemorySiteV1 { block: 0, operation: 4 } })));
}

#[test]
fn initial_input_commitment_requires_the_exact_live_producer() {
    let mut context = setup();
    let f = fixture(&mut context);
    let inputs = LivePlironInitialReadInputsV1::prove(&context, &f.function).unwrap();
    assert_eq!(inputs.commitment_leaf(f.read.get_operation()).unwrap(),
        Some((SEMANTIC_TYPED_READ_SYMBOL_BASE_V1, scalar())));
    assert_eq!(inputs.commitment_leaf(f.sum.get_operation()).unwrap(), None);
    assert_eq!(inputs.commitment_leaf(f.access.get_operation()).unwrap(), None);
    f.sum.get_operation().unlink(&context);
    assert_eq!(inputs.commitment_leaf(f.read.get_operation()), Err(E::MutationEpochChanged));
}

#[test]
fn changed_owner_function_epoch_and_pointer_graph_reject() {
    let mut context = setup();
    let f = fixture(&mut context);
    let other = fixture(&mut context);
    let proof = prove_live_pliron_semantic_memory_v1(&context, &f.function).unwrap();
    assert_eq!(
        proof.revalidate(&context, &other.function),
        Err(E::FunctionChanged)
    );
    let mut foreign = setup();
    let other = fixture(&mut foreign);
    assert_eq!(
        proof.revalidate(&foreign, &other.function),
        Err(E::ContextChanged)
    );
    f.view
        .set_attr_kernel_allocation_origin(&mut context, AllocationOriginAttr(2));
    assert_eq!(
        proof.revalidate(&context, &f.function),
        Err(E::MutationEpochChanged)
    );
}

#[test]
fn missing_access_pair_and_non_dominating_index_reject_exactly() {
    let mut context = setup();
    let f = fixture(&mut context);
    Operation::erase(f.access.get_operation(), &mut context);
    assert_eq!(
        prove_live_pliron_semantic_memory_v1(&context, &f.function).err(),
        Some(E::UnpairedRead {
            site: PlironSemanticMemorySiteV1 {
                block: 0,
                operation: 2
            }
        })
    );
    let mut context = setup();
    let f = fixture(&mut context);
    f.index.get_operation().unlink(&context);
    f.index
        .get_operation()
        .insert_before(&context, f.ret.get_operation());
    assert_eq!(
        prove_live_pliron_semantic_memory_v1(&context, &f.function).err(),
        Some(E::NonDominatingOperand {
            site: PlironSemanticMemorySiteV1 {
                block: 0,
                operation: 1
            }
        })
    );
}

#[test]
fn live_bounds_and_cross_invocation_interference_are_not_erased() {
    let mut context = setup();
    let f = fixture(&mut context);
    f.index
        .set_attr_kernel_launch_extent(&mut context, LaunchExtentAttr(65));
    assert_eq!(
        prove_live_pliron_semantic_memory_v1(&context, &f.function).err(),
        Some(E::UnprovedBounds {
            site: PlironSemanticMemorySiteV1 {
                block: 0,
                operation: 2
            }
        })
    );
    let mut context = setup();
    let f = fixture(&mut context);
    let zero = IndexConstantOp::new(&mut context, 0);
    zero.get_operation()
        .insert_before(&context, f.write.get_operation());
    Operation::replace_operand(f.write.get_operation(), &context, 1, zero.result(&context));
    assert!(matches!(
        prove_live_pliron_semantic_memory_v1(&context, &f.function).err(),
        Some(E::Interference {
            first_invocation: 0,
            second_invocation: 1,
            ..
        })
    ));
}

#[test]
fn volatile_ordered_free_symbol_and_unused_read_are_rejected() {
    for mutation in 0..4 {
        let mut context = setup();
        let f = fixture(&mut context);
        let expected = match mutation {
            0 => {
                f.read.set_attr_kernel_semantic_read_volatility(
                    &mut context,
                    SemanticReadVolatilityAttr::Volatile,
                );
                E::UnsupportedRead {
                    site: PlironSemanticMemorySiteV1 {
                        block: 0,
                        operation: 3,
                    },
                }
            }
            1 => {
                f.read.set_attr_kernel_semantic_read_ordering(
                    &mut context,
                    SemanticReadOrderingAttr::Acquire,
                );
                E::InvalidGraph
            }
            2 => {
                let symbol = SemanticTypedSymbolOp::new(
                    &mut context,
                    SEMANTIC_TYPED_READ_SYMBOL_BASE_V1,
                    scalar(),
                );
                symbol
                    .get_operation()
                    .insert_before(&context, f.ret.get_operation());
                E::UnboundLoadSymbol {
                    site: PlironSemanticMemorySiteV1 {
                        block: 0,
                        operation: 7,
                    },
                }
            }
            _ => {
                Operation::erase(f.sum.get_operation(), &mut context);
                E::UnconsumedRead
            }
        };
        assert_eq!(
            prove_live_pliron_semantic_memory_v1(&context, &f.function).err(),
            Some(expected)
        );
    }
}
